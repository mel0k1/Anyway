use rusqlite::Connection;
use serde_json::{json, Value};

/// Persistent cross-scan entity index: every finished scan feeds its findings
/// into one SQLite store, so identities can be tracked over time ("first
/// seen", "last seen", "changed"). Observations keep the per-scan history that
/// powers diffs between any two scans.
pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS entities (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    value TEXT NOT NULL,
    label TEXT NOT NULL DEFAULT '',
    first_seen INTEGER NOT NULL,
    last_seen INTEGER NOT NULL,
    times_seen INTEGER NOT NULL DEFAULT 1,
    UNIQUE(kind, value)
);
CREATE INDEX IF NOT EXISTS idx_entities_kind ON entities(kind);
CREATE INDEX IF NOT EXISTS idx_entities_last_seen ON entities(last_seen);
CREATE TABLE IF NOT EXISTS observations (
    entity_id INTEGER NOT NULL,
    scan_id TEXT NOT NULL,
    seen_at INTEGER NOT NULL,
    PRIMARY KEY (entity_id, scan_id)
);
";

fn open(db_path: &str) -> Result<Connection, String> {
    let conn = Connection::open(db_path).map_err(|e| format!("cannot open db: {e}"))?;
    conn.execute_batch(SCHEMA)
        .map_err(|e| format!("schema init failed: {e}"))?;
    Ok(conn)
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Ingest one scan snapshot into the store. Empty values and findings without
/// a value are skipped; a scan ingested twice is a no-op for observations but
/// still refreshes `last_seen`/`times_seen`.
pub fn ingest(db_path: &str, snapshot: &Value, scan_id: &str) -> Result<Value, String> {
    let scan = {
        let s = scan_id.trim();
        if s.is_empty() {
            format!("manual-{}", now_unix())
        } else {
            s.to_string()
        }
    };
    let now = now_unix() as i64;
    let mut conn = open(db_path)?;
    let tx = conn.transaction().map_err(|e| format!("tx failed: {e}"))?;
    let mut new = 0u64;
    let mut updated = 0u64;
    let mut seen = 0u64;
    if let Some(findings) = snapshot["findings"].as_array() {
        for f in findings {
            let kind = f["category"].as_str().unwrap_or("").trim().to_string();
            let value = f["value"].as_str().unwrap_or("").trim().to_string();
            if kind.is_empty() || value.is_empty() {
                continue;
            }
            let label = f["label"].as_str().unwrap_or("").trim().to_string();
            seen += 1;
            let before: Option<i64> = tx
                .query_row(
                    "SELECT times_seen FROM entities WHERE kind = ?1 AND value = ?2",
                    rusqlite::params![kind, value],
                    |r| r.get(0),
                )
                .ok();
            tx.execute(
                "INSERT INTO entities (kind, value, label, first_seen, last_seen, times_seen)
                 VALUES (?1, ?2, ?3, ?4, ?4, 1)
                 ON CONFLICT(kind, value) DO UPDATE SET
                     last_seen = excluded.last_seen,
                     times_seen = times_seen + 1,
                     label = CASE WHEN excluded.label != '' THEN excluded.label ELSE entities.label END",
                rusqlite::params![kind, value, label, now],
            )
            .map_err(|e| format!("upsert failed: {e}"))?;
            if before.is_some() {
                updated += 1;
            } else {
                new += 1;
            }
            let entity_id: i64 = tx
                .query_row(
                    "SELECT id FROM entities WHERE kind = ?1 AND value = ?2",
                    rusqlite::params![kind, value],
                    |r| r.get(0),
                )
                .map_err(|e| format!("lookup failed: {e}"))?;
            tx.execute(
                "INSERT OR IGNORE INTO observations (entity_id, scan_id, seen_at) VALUES (?1, ?2, ?3)",
                rusqlite::params![entity_id, scan, now],
            )
            .map_err(|e| format!("observation failed: {e}"))?;
        }
    }
    tx.commit().map_err(|e| format!("commit failed: {e}"))?;
    Ok(json!({
        "ok": true,
        "scan_id": scan,
        "findings_seen": seen,
        "new_entities": new,
        "updated_entities": updated,
    }))
}

pub fn stats(db_path: &str) -> Result<Value, String> {
    let conn = open(db_path)?;
    let entities: i64 = conn
        .query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))
        .map_err(|e| format!("query failed: {e}"))?;
    let observations: i64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT scan_id) FROM observations",
            [],
            |r| r.get(0),
        )
        .map_err(|e| format!("query failed: {e}"))?;
    let mut kinds = Vec::new();
    {
        let mut stmt = conn
            .prepare("SELECT kind, COUNT(*) FROM entities GROUP BY kind ORDER BY 2 DESC LIMIT 40")
            .map_err(|e| format!("query failed: {e}"))?;
        let rows = stmt
            .query_map([], |r| {
                Ok(json!({"kind": r.get::<_, String>(0)?, "count": r.get::<_, i64>(1)?}))
            })
            .map_err(|e| format!("query failed: {e}"))?;
        for row in rows {
            kinds.push(row.map_err(|e| format!("row failed: {e}"))?);
        }
    }
    Ok(json!({
        "ok": true,
        "entities": entities,
        "scans": observations,
        "kinds": kinds,
    }))
}

fn like_escaped(q: &str) -> String {
    q.to_lowercase()
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Search entities by value/label substring. `kind` filters to one category or
/// is empty / "all" for everything.
pub fn search(
    db_path: &str,
    query: &str,
    kind: &str,
    limit: usize,
    offset: usize,
) -> Result<Value, String> {
    let conn = open(db_path)?;
    let limit = limit.clamp(1, 1000) as i64;
    let offset = offset as i64;
    let kind_filter = if kind.is_empty() || kind == "all" {
        String::new()
    } else {
        format!("AND kind = '{}'", kind.replace('\'', "''"))
    };
    let q = query.trim();
    let rows = if q.is_empty() {
        let sql = format!(
            "SELECT id, kind, value, label, first_seen, last_seen, times_seen
             FROM entities WHERE 1=1 {kind_filter} ORDER BY last_seen DESC, id DESC LIMIT ?1 OFFSET ?2"
        );
        query_rows(&conn, &sql, rusqlite::params![limit, offset])?
    } else {
        let like = format!("%{}%", like_escaped(q));
        let sql = format!(
            "SELECT id, kind, value, label, first_seen, last_seen, times_seen
             FROM entities WHERE (value LIKE ?1 ESCAPE '\\' OR lower(label) LIKE ?1 ESCAPE '\\') {kind_filter}
             ORDER BY last_seen DESC, id DESC LIMIT ?2 OFFSET ?3"
        );
        query_rows(&conn, &sql, rusqlite::params![like, limit, offset])?
    };
    Ok(json!({"ok": true, "total_returned": rows.len(), "entities": rows}))
}

fn query_rows(
    conn: &Connection,
    sql: &str,
    params: impl rusqlite::Params,
) -> Result<Vec<Value>, String> {
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| format!("query failed: {e}"))?;
    let rows = stmt
        .query_map(params, |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "kind": r.get::<_, String>(1)?,
                "value": r.get::<_, String>(2)?,
                "label": r.get::<_, String>(3)?,
                "first_seen": r.get::<_, i64>(4)?,
                "last_seen": r.get::<_, i64>(5)?,
                "times_seen": r.get::<_, i64>(6)?,
            }))
        })
        .map_err(|e| format!("query failed: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("row failed: {e}"))?);
    }
    Ok(out)
}

/// Full record for one entity, including every scan that observed it.
pub fn entity(db_path: &str, id: i64) -> Result<Value, String> {
    let conn = open(db_path)?;
    let base = query_rows(
        &conn,
        "SELECT id, kind, value, label, first_seen, last_seen, times_seen FROM entities WHERE id = ?1",
        rusqlite::params![id],
    )?;
    let Some(e) = base.into_iter().next() else {
        return Err("unknown entity".into());
    };
    let mut observations = Vec::new();
    {
        let mut stmt = conn
            .prepare("SELECT scan_id, seen_at FROM observations WHERE entity_id = ?1 ORDER BY seen_at DESC LIMIT 200")
            .map_err(|e| format!("query failed: {e}"))?;
        let rows = stmt
            .query_map(rusqlite::params![id], |r| {
                Ok(json!({"scan_id": r.get::<_, String>(0)?, "seen_at": r.get::<_, i64>(1)?}))
            })
            .map_err(|e| format!("query failed: {e}"))?;
        for row in rows {
            observations.push(row.map_err(|e| format!("row failed: {e}"))?);
        }
    }
    Ok(json!({"ok": true, "entity": e, "observations": observations}))
}

/// Compare two scans by their observation sets: entities seen in one scan but
/// not the other appeared or disappeared between the runs.
pub fn diff(db_path: &str, scan_id_a: &str, scan_id_b: &str) -> Result<Value, String> {
    let conn = open(db_path)?;
    let only_in = |a: &str, b: &str| -> Result<Vec<Value>, String> {
        query_rows(
            &conn,
            "SELECT e.id, e.kind, e.value, e.label, e.first_seen, e.last_seen, e.times_seen
             FROM entities e
             WHERE EXISTS (SELECT 1 FROM observations o WHERE o.entity_id = e.id AND o.scan_id = ?1)
               AND NOT EXISTS (SELECT 1 FROM observations o WHERE o.entity_id = e.id AND o.scan_id = ?2)
             ORDER BY e.kind, e.value LIMIT 200",
            rusqlite::params![a, b],
        )
    };
    let gone = only_in(scan_id_a, scan_id_b)?;
    let appeared = only_in(scan_id_b, scan_id_a)?;
    let common: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM entities e
             WHERE EXISTS (SELECT 1 FROM observations o WHERE o.entity_id = e.id AND o.scan_id = ?1)
               AND EXISTS (SELECT 1 FROM observations o WHERE o.entity_id = e.id AND o.scan_id = ?2)",
            rusqlite::params![scan_id_a, scan_id_b],
            |r| r.get(0),
        )
        .map_err(|e| format!("query failed: {e}"))?;
    Ok(json!({
        "ok": true,
        "scan_a": scan_id_a,
        "scan_b": scan_id_b,
        "appeared": appeared,
        "disappeared": gone,
        "common": common,
    }))
}

pub fn clear(db_path: &str) -> Result<Value, String> {
    let conn = open(db_path)?;
    conn.execute_batch("DELETE FROM observations; DELETE FROM entities;")
        .map_err(|e| format!("clear failed: {e}"))?;
    Ok(json!({"ok": true}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn snap() -> Value {
        json!({"findings": [
            {"module_id": "u", "category": "social_profile", "label": "GitHub", "value": "https://github.com/mel0k1"},
            {"module_id": "b", "category": "breach", "label": "Adobe", "value": "mel0k1@example.org"},
            {"module_id": "b", "category": "breach", "label": "Adobe", "value": "mel0k1@example.org"},
            {"module_id": "m", "category": "email", "label": "Mail", "value": ""},
        ]})
    }

    fn temp_db(name: &str) -> String {
        let p = std::env::temp_dir().join(format!(
            "estore-{name}-{}-{}.sqlite",
            std::process::id(),
            now_unix()
        ));
        p.to_string_lossy().to_string()
    }

    #[test]
    fn ingest_dedups_and_tracks() {
        let db = temp_db("ingest");
        let r = ingest(&db, &snap(), "scan-a").unwrap();
        // 4 findings -> 2 distinct non-empty entities; the duplicate Adobe entry updates
        assert_eq!(r["findings_seen"], 3);
        assert_eq!(r["new_entities"], 2);
        assert_eq!(r["updated_entities"], 1);
        // second ingest of the same scan: nothing new, counts move to updated
        let r2 = ingest(&db, &snap(), "scan-a").unwrap();
        assert_eq!(r2["new_entities"], 0);
        assert_eq!(r2["updated_entities"], 3);
        let s = stats(&db).unwrap();
        assert_eq!(s["entities"], 2);
        assert_eq!(s["scans"], 1);
        let _ = std::fs::remove_file(&db);
    }

    #[test]
    fn search_filters_by_kind_and_query() {
        let db = temp_db("search");
        ingest(&db, &snap(), "scan-a").unwrap();
        let all = search(&db, "", "all", 100, 0).unwrap();
        assert_eq!(all["total_returned"], 2);
        let only_breach = search(&db, "", "breach", 100, 0).unwrap();
        assert_eq!(only_breach["total_returned"], 1);
        assert_eq!(only_breach["entities"][0]["kind"], "breach");
        let by_query = search(&db, "github", "all", 100, 0).unwrap();
        assert_eq!(by_query["total_returned"], 1);
        assert_eq!(
            by_query["entities"][0]["value"],
            "https://github.com/mel0k1"
        );
        // LIKE wildcards stay literal
        assert_eq!(
            search(&db, "%", "all", 100, 0).unwrap()["total_returned"],
            0
        );
        let _ = std::fs::remove_file(&db);
    }

    #[test]
    fn entity_detail_lists_observations() {
        let db = temp_db("detail");
        ingest(&db, &snap(), "scan-a").unwrap();
        ingest(&db, &snap(), "scan-b").unwrap();
        let e = entity(&db, 1).unwrap();
        assert_eq!(e["entity"]["times_seen"], 2);
        let obs = e["observations"].as_array().unwrap();
        assert_eq!(obs.len(), 2);
        let ids: Vec<&str> = obs.iter().map(|o| o["scan_id"].as_str().unwrap()).collect();
        assert!(ids.contains(&"scan-a") && ids.contains(&"scan-b"));
        assert!(entity(&db, 999).is_err());
        let _ = std::fs::remove_file(&db);
    }

    #[test]
    fn diff_between_scans() {
        let db = temp_db("diff");
        let snap_b = json!({"findings": [
            {"module_id": "u", "category": "social_profile", "label": "GitHub", "value": "https://github.com/mel0k1"},
            {"module_id": "m", "category": "email", "label": "Mail", "value": "fresh@example.org"},
        ]});
        ingest(&db, &snap(), "scan-a").unwrap();
        ingest(&db, &snap_b, "scan-b").unwrap();
        let d = diff(&db, "scan-a", "scan-b").unwrap();
        assert_eq!(d["common"], 1);
        let appeared = d["appeared"].as_array().unwrap();
        assert_eq!(appeared.len(), 1);
        assert_eq!(appeared[0]["value"], "fresh@example.org");
        let gone = d["disappeared"].as_array().unwrap();
        assert_eq!(gone.len(), 1);
        assert_eq!(gone[0]["value"], "mel0k1@example.org");
        let _ = std::fs::remove_file(&db);
    }

    #[test]
    fn clear_wipes_everything() {
        let db = temp_db("clear");
        ingest(&db, &snap(), "scan-a").unwrap();
        clear(&db).unwrap();
        assert_eq!(stats(&db).unwrap()["entities"], 0);
        let _ = std::fs::remove_file(&db);
    }
}
