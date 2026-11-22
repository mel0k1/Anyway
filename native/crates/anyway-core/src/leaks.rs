use rusqlite::Connection;
use serde_json::{json, Value};

/// Local breach-dump indexing: parse text/JSONL/CSV dumps into a SQLite
/// database owned by the app and search it by email / username / phone /
/// domain. Passwords are masked in search results unless explicitly revealed.
pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS records (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source TEXT NOT NULL,
    email TEXT,
    username TEXT,
    phone TEXT,
    password TEXT,
    ip TEXT,
    domain TEXT,
    hash TEXT,
    extra TEXT,
    line_no INTEGER
);
CREATE INDEX IF NOT EXISTS idx_records_email ON records(email);
CREATE INDEX IF NOT EXISTS idx_records_username ON records(username);
CREATE INDEX IF NOT EXISTS idx_records_phone ON records(phone);
CREATE INDEX IF NOT EXISTS idx_records_domain ON records(domain);
CREATE INDEX IF NOT EXISTS idx_records_source ON records(source);
CREATE TABLE IF NOT EXISTS sources (name TEXT PRIMARY KEY, records INTEGER NOT NULL DEFAULT 0, imported_at INTEGER NOT NULL);
";

fn open(db_path: &str) -> Result<Connection, String> {
    let conn = Connection::open(db_path).map_err(|e| format!("cannot open db: {e}"))?;
    conn.execute_batch(SCHEMA)
        .map_err(|e| format!("schema init failed: {e}"))?;
    Ok(conn)
}

fn is_email(s: &str) -> bool {
    let s = s.trim().to_lowercase();
    s.split_once('@').is_some_and(|(l, d)| {
        !l.is_empty()
            && d.contains('.')
            && !d.contains('@')
            && !s.contains(|c: char| c.is_whitespace())
    })
}

fn is_phone_token(s: &str) -> bool {
    let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    (s.starts_with('+') || digits.len() == s.len()) && digits.len() >= 10 && digits.len() <= 15
}

fn is_ip(s: &str) -> bool {
    s.parse::<std::net::IpAddr>().is_ok()
}

fn is_hashlike(s: &str) -> bool {
    let n = s.len();
    n >= 16 && s.chars().all(|c| c.is_ascii_hexdigit())
}

fn is_domain_like(s: &str) -> bool {
    let s = s.trim().to_lowercase();
    !s.is_empty()
        && !s.contains(['@', ':', '/', ' '])
        && s.contains('.')
        && s.split('.').all(|p| !p.is_empty())
        && s.split('.')
            .next_back()
            .is_some_and(|tld| tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphanumeric()))
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
}

#[derive(Debug, Default, Clone)]
pub struct ParsedRecord {
    pub email: String,
    pub username: String,
    pub phone: String,
    pub password: String,
    pub ip: String,
    pub domain: String,
    pub hash: String,
    pub extra: String,
}

fn pick_password(fields: &[&str], used: usize) -> String {
    // first non-credential, non-identifier field after the credential
    for f in fields.iter().skip(used + 1) {
        let f = f.trim();
        if f.is_empty() || is_email(f) || is_ip(f) || is_phone_token(f) || is_domain_like(f) {
            continue;
        }
        if is_hashlike(f) {
            continue;
        }
        return f.to_string();
    }
    String::new()
}

fn classify_fields(fields: &[&str]) -> ParsedRecord {
    let mut rec = ParsedRecord::default();
    let mut credential_used = usize::MAX;
    for (i, raw) in fields.iter().enumerate() {
        let f = raw.trim();
        if f.is_empty() {
            continue;
        }
        if rec.email.is_empty() && is_email(f) {
            rec.email = f.to_lowercase();
            if credential_used == usize::MAX {
                credential_used = i;
            }
            continue;
        }
        if credential_used == usize::MAX && rec.phone.is_empty() && is_phone_token(f) {
            rec.phone = f.replace(['(', ')', '-', ' '], "");
            credential_used = i;
            continue;
        }
        if credential_used == usize::MAX && rec.username.is_empty() && !is_ip(f) {
            rec.username = f.to_string();
            credential_used = i;
            continue;
        }
        if rec.ip.is_empty() && is_ip(f) {
            rec.ip = f.to_string();
            continue;
        }
        if rec.hash.is_empty() && is_hashlike(f) {
            rec.hash = f.to_lowercase();
            continue;
        }
        if rec.domain.is_empty() && is_domain_like(f) && !is_email(f) {
            rec.domain = f.to_lowercase();
            continue;
        }
        if rec.extra.is_empty() {
            rec.extra = f.to_string();
        }
    }
    rec.password = pick_password(fields, credential_used);
    rec
}

fn parse_jsonl(obj: &Value) -> ParsedRecord {
    let g = |keys: &[&str]| -> String {
        for k in keys {
            let v = obj
                .get(*k)
                .or_else(|| obj.get(k.to_ascii_lowercase()))
                .cloned()
                .unwrap_or(Value::Null);
            if let Some(s) = v.as_str() {
                if !s.is_empty() {
                    return s.to_string();
                }
            }
        }
        String::new()
    };
    let mut rec = ParsedRecord {
        email: g(&["email", "Email", "e-mail", "mail"]).to_lowercase(),
        username: g(&["username", "user", "login", "nick", "user_name"]),
        phone: g(&["phone", "tel", "telephone", "mobile"]).replace(['(', ')', '-', ' '], ""),
        password: g(&["password", "pass", "passwd", "pwd"]),
        ip: g(&["ip", "ip_address", "lastip", "reg_ip"]),
        domain: g(&["domain", "host"]),
        hash: g(&["hash", "password_hash"]),
        extra: g(&["extra", "misc", "note"]),
    };
    if rec.email.is_empty() {
        rec.email = g(&["username", "user", "login"]).to_lowercase();
        if !is_email(&rec.email) {
            rec.email = String::new();
        }
    }
    rec
}

/// Parse one dump line. Returns None for blank lines and comments.
pub fn parse_line(line: &str) -> Option<ParsedRecord> {
    let line = line.trim_end_matches(['\r', '\n']);
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
        return None;
    }
    if let Ok(obj) = serde_json::from_str::<Value>(trimmed) {
        if obj.is_object() {
            let rec = parse_jsonl(&obj);
            if !rec.email.is_empty() || !rec.username.is_empty() || !rec.phone.is_empty() {
                return Some(rec);
            }
            return None;
        }
    }
    // CSV with a recognizable header?
    if trimmed.contains(',') && !trimmed.contains(':') {
        return None; // handled at import level with headers
    }
    let separators = if trimmed.contains(';') && !trimmed.contains(':') {
        ';'
    } else {
        ':'
    };
    let fields: Vec<&str> = trimmed.split(separators).collect();
    if fields.len() < 2 || fields.len() > 8 {
        return None;
    }
    let rec = classify_fields(&fields);
    if rec.email.is_empty() && rec.username.is_empty() && rec.phone.is_empty() {
        return None;
    }
    Some(rec)
}

#[derive(Debug)]
struct CsvTable {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

fn parse_csv(text: &str) -> Option<CsvTable> {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let header: Vec<String> = lines
        .next()?
        .split(',')
        .map(|h| h.trim().trim_matches('"').to_lowercase())
        .collect();
    const KNOWN: &[&str] = &[
        "email", "e-mail", "mail", "username", "user", "login", "password", "pass", "passwd",
        "phone", "ip", "domain", "hash",
    ];
    if !header.iter().any(|h| KNOWN.contains(&h.as_str())) {
        return None;
    }
    let rows: Vec<Vec<String>> = lines
        .map(|l| {
            l.split(',')
                .map(|c| c.trim().trim_matches('"').to_string())
                .collect()
        })
        .collect();
    Some(CsvTable {
        headers: header,
        rows,
    })
}

fn csv_record(row: &[String], headers: &[String]) -> ParsedRecord {
    let mut rec = ParsedRecord::default();
    for (i, h) in headers.iter().enumerate() {
        let val = row.get(i).cloned().unwrap_or_default();
        match h.as_str() {
            "email" | "e-mail" | "mail" => rec.email = val.to_lowercase(),
            "username" | "user" | "login" => rec.username = val,
            "password" | "pass" | "passwd" => rec.password = val,
            "phone" => rec.phone = val.replace(['(', ')', '-', ' '], ""),
            "ip" => rec.ip = val,
            "domain" => rec.domain = val.to_lowercase(),
            "hash" => rec.hash = val.to_lowercase(),
            _ => {
                if rec.extra.is_empty() && !val.is_empty() {
                    rec.extra = val;
                }
            }
        }
    }
    rec
}

pub fn import_text(db_path: &str, source: &str, text: &str) -> Result<Value, String> {
    if source.trim().is_empty() {
        return Err("source name is required".into());
    }
    if text.len() > 512 * 1024 * 1024 {
        return Err("dump too large for one import (>512MB), split the file".into());
    }
    let mut conn = open(db_path)?;
    let tx = conn.transaction().map_err(|e| format!("tx failed: {e}"))?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut lines_total = 0u64;
    let mut imported = 0u64;
    let mut skipped = 0u64;
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    let maybe_csv = parse_csv(text);
    if let Some(csv) = &maybe_csv {
        for (idx, row) in csv.rows.iter().enumerate() {
            lines_total += 1;
            let rec = csv_record(row, &csv.headers);
            if rec.email.is_empty() && rec.username.is_empty() && rec.phone.is_empty() {
                skipped += 1;
                continue;
            }
            let key = format!(
                "{}|{}|{}|{}",
                rec.email, rec.username, rec.phone, rec.password
            );
            if !seen.insert(key) {
                skipped += 1;
                continue;
            }
            tx.execute(
                "INSERT INTO records (source, email, username, phone, password, ip, domain, hash, extra, line_no)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                rusqlite::params![
                    source,
                    rec.email,
                    rec.username,
                    rec.phone,
                    rec.password,
                    rec.ip,
                    rec.domain,
                    rec.hash,
                    rec.extra,
                    idx as i64
                ],
            )
            .map_err(|e| format!("insert failed: {e}"))?;
            imported += 1;
        }
    } else {
        for (idx, line) in text.lines().enumerate() {
            lines_total += 1;
            let Some(rec) = parse_line(line) else {
                skipped += 1;
                continue;
            };
            let key = format!(
                "{}|{}|{}|{}",
                rec.email, rec.username, rec.phone, rec.password
            );
            if !seen.insert(key) {
                skipped += 1;
                continue;
            }
            tx.execute(
                "INSERT INTO records (source, email, username, phone, password, ip, domain, hash, extra, line_no)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                rusqlite::params![
                    source,
                    rec.email,
                    rec.username,
                    rec.phone,
                    rec.password,
                    rec.ip,
                    rec.domain,
                    rec.hash,
                    rec.extra,
                    idx as i64
                ],
            )
            .map_err(|e| format!("insert failed: {e}"))?;
            imported += 1;
        }
    }

    tx.execute(
        "INSERT INTO sources (name, records, imported_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(name) DO UPDATE SET records = records + ?2",
        rusqlite::params![source, imported as i64, now as i64],
    )
    .map_err(|e| format!("source update failed: {e}"))?;
    tx.commit().map_err(|e| format!("commit failed: {e}"))?;
    Ok(json!({
        "ok": true,
        "source": source,
        "lines": lines_total,
        "imported": imported,
        "skipped": skipped,
    }))
}

pub fn search(db_path: &str, query: &str, limit: usize, reveal: bool) -> Result<Value, String> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Err("empty query".into());
    }
    let conn = open(db_path)?;
    // escape LIKE wildcards so a query like "100%" matches literally
    let escaped = q
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    let like = format!("%{escaped}%");
    let field = if is_email(&q) {
        "email"
    } else if is_phone_token(&q) {
        "phone"
    } else if is_domain_like(&q) {
        "domain"
    } else {
        "username"
    };
    let limit = limit.clamp(1, 1000) as i64;
    let mut stmt = conn
        .prepare(&format!(
            "SELECT source, email, username, phone, password, ip, domain, hash, extra, line_no
             FROM records WHERE {field} LIKE ?1 ESCAPE '\\' ORDER BY source, id LIMIT ?2"
        ))
        .map_err(|e| format!("query failed: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params![like, limit], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, String>(8)?,
                r.get::<_, i64>(9)?,
            ))
        })
        .map_err(|e| format!("query failed: {e}"))?;
    let mut matches = Vec::new();
    for row in rows {
        let (source, email, username, phone, password, ip, domain, hash, extra, line_no) =
            row.map_err(|e| format!("row: {e}"))?;
        matches.push(json!({
            "source": source,
            "email": email,
            "username": username,
            "phone": phone,
            "password": if reveal { password.clone() } else { mask(&password) },
            "ip": ip,
            "domain": domain,
            "hash": hash,
            "extra": extra,
            "line": line_no,
        }));
    }
    let total: u64 = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM records WHERE {field} LIKE ?1 ESCAPE '\\'"),
            rusqlite::params![like],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n.max(0) as u64)
        .unwrap_or(0);
    Ok(json!({
        "ok": true,
        "queryField": field,
        "total": total,
        "returned": matches.len(),
        "truncated": total as usize > matches.len(),
        "matches": matches,
    }))
}

fn mask(password: &str) -> String {
    if password.is_empty() {
        return String::new();
    }
    let visible = password.chars().take(1).collect::<String>();
    format!("{visible}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}")
}

pub fn stats(db_path: &str) -> Result<Value, String> {
    let conn = open(db_path)?;
    let total: u64 = conn
        .query_row("SELECT COUNT(*) FROM records", [], |r| r.get::<_, i64>(0))
        .map(|n| n.max(0) as u64)
        .unwrap_or(0);
    let with_password: u64 = conn
        .query_row(
            "SELECT COUNT(*) FROM records WHERE password != ''",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n.max(0) as u64)
        .unwrap_or(0);
    let mut stmt = conn
        .prepare("SELECT name, records FROM sources ORDER BY records DESC LIMIT 100")
        .map_err(|e| format!("stats failed: {e}"))?;
    let sources: Vec<Value> = stmt
        .query_map([], |r| {
            Ok(json!({
                "name": r.get::<_, String>(0)?,
                "records": r.get::<_, i64>(1)?.max(0),
            }))
        })
        .map_err(|e| format!("stats failed: {e}"))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(json!({
        "ok": true,
        "records": total,
        "withPassword": with_password,
        "sources": sources,
    }))
}

pub fn clear(db_path: &str) -> Result<Value, String> {
    let conn = open(db_path)?;
    conn.execute("DELETE FROM records", [])
        .map_err(|e| format!("clear failed: {e}"))?;
    conn.execute("DELETE FROM sources", [])
        .map_err(|e| format!("clear failed: {e}"))?;
    Ok(json!({"ok": true}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db(tag: &str) -> String {
        let dir = std::env::temp_dir();
        format!(
            "{}/anyway_leaks_{}_{}.db",
            dir.display(),
            tag,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        )
    }

    #[test]
    fn token_classifiers() {
        assert!(is_email("User@Example.ORG"));
        assert!(!is_email("nope"));
        assert!(!is_email("a@b"));
        assert!(is_phone_token("+79261234567"));
        assert!(is_phone_token("79261234567"));
        assert!(!is_phone_token("123"));
        assert!(is_ip("192.168.1.5"));
        assert!(!is_ip("999.1.1.1"));
        assert!(is_hashlike("5f4dcc3b5aa765d61d8327deb882cf99"));
        assert!(!is_hashlike("hunter2"));
        assert!(is_domain_like("example.org"));
        assert!(!is_domain_like("not_a_domain"));
    }

    #[test]
    fn parses_colon_layouts() {
        let r = parse_line("victim@example.org:Secret123").unwrap();
        assert_eq!(r.email, "victim@example.org");
        assert_eq!(r.password, "Secret123");

        let r = parse_line("victim@example.org:Secret123:192.168.0.1").unwrap();
        assert_eq!(r.password, "Secret123");
        assert_eq!(r.ip, "192.168.0.1");

        let r = parse_line("someuser:me@corp.io:Passw0rd").unwrap();
        assert_eq!(r.username, "someuser");
        assert_eq!(r.email, "me@corp.io");
        assert_eq!(r.password, "Passw0rd");

        let r = parse_line("user@example.org:5f4dcc3b5aa765d61d8327deb882cf99:Pass1").unwrap();
        assert_eq!(r.email, "user@example.org");
        assert_eq!(r.hash, "5f4dcc3b5aa765d61d8327deb882cf99");
        assert_eq!(r.password, "Pass1");

        let r = parse_line("john:doe:pass1:extra").unwrap();
        assert_eq!(r.username, "john");
        assert_eq!(r.password, "doe");

        assert!(parse_line("# comment").is_none());
        assert!(parse_line("").is_none());
        assert!(parse_line("justoneword").is_none());
    }

    #[test]
    fn parses_jsonl() {
        let r = parse_line(r#"{"email": "a@b.co", "password": "pw", "ip": "1.2.3.4"}"#).unwrap();
        assert_eq!(r.email, "a@b.co");
        assert_eq!(r.password, "pw");
        assert_eq!(r.ip, "1.2.3.4");
        assert!(parse_line(r#"{"foo": 1}"#).is_none());
    }

    #[test]
    fn search_escapes_like_wildcards() {
        let db = temp_db("escape");
        let text = "aXb:pass1\na%b:pass2\na_b:pass3\n";
        import_text(&db, "wildcards", text).unwrap();
        // % and _ match literally, not as SQL wildcards
        assert_eq!(search(&db, "a%b", 10, false).unwrap()["total"], 1);
        assert_eq!(search(&db, "a_b", 10, false).unwrap()["total"], 1);
        assert_eq!(
            search(&db, "a_b", 10, false).unwrap()["matches"][0]["username"],
            "a_b"
        );
        // a bare wildcard query finds only records containing that literal char
        assert_eq!(search(&db, "%", 10, false).unwrap()["total"], 1);
        assert_eq!(search(&db, "_", 10, false).unwrap()["total"], 1);
        // plain substring search matches all three
        assert_eq!(search(&db, "a", 10, false).unwrap()["total"], 3);
        let _ = std::fs::remove_file(&db);
    }

    #[test]
    fn import_search_cycle() {
        let db = temp_db("cycle");
        let text = "\
# leak 2023\n\
victim@example.org:Secret123:192.168.0.1\n\
victim@example.org:Secret123\n\
john:doe:pass1\n\
+79261234567:pw123456\n\
{\"email\": \"json@corp.io\", \"password\": \"jsonpw\"}\n";
        let res = import_text(&db, "combo-2023", text).unwrap();
        assert_eq!(res["lines"], 6);
        assert_eq!(res["imported"], 4); // email dup skipped
        assert_eq!(res["skipped"], 2);

        let s = search(&db, "victim@example.org", 100, false).unwrap();
        assert_eq!(s["queryField"], "email");
        assert_eq!(s["total"], 1);
        let m = &s["matches"][0];
        assert!(m["password"].as_str().unwrap().contains('\u{2022}'));

        let revealed = search(&db, "victim@example.org", 100, true).unwrap();
        assert_eq!(revealed["matches"][0]["password"], "Secret123");
        assert_eq!(revealed["matches"][0]["ip"], "192.168.0.1");

        let by_user = search(&db, "john", 10, false).unwrap();
        assert_eq!(by_user["queryField"], "username");
        assert_eq!(by_user["total"], 1);

        let by_phone = search(&db, "+79261234567", 10, false).unwrap();
        assert_eq!(by_phone["queryField"], "phone");
        assert_eq!(by_phone["total"], 1);

        let st = stats(&db).unwrap();
        assert_eq!(st["records"], 4);
        assert_eq!(st["sources"][0]["name"], "combo-2023");

        // second import into same db accumulates
        import_text(&db, "other", "second@corp.io:pw2").unwrap();
        let st2 = stats(&db).unwrap();
        assert_eq!(st2["records"], 5);

        clear(&db).unwrap();
        let st3 = stats(&db).unwrap();
        assert_eq!(st3["records"], 0);
        let _ = std::fs::remove_file(&db);
    }

    #[test]
    fn csv_import() {
        let db = temp_db("csv");
        let text = "email,password,ip\na@x.io,p1,1.1.1.1\nb@x.io,p2,2.2.2.2\n";
        let res = import_text(&db, "csv-dump", text).unwrap();
        assert_eq!(res["imported"], 2);
        let s = search(&db, "a@x.io", 10, true).unwrap();
        assert_eq!(s["matches"][0]["ip"], "1.1.1.1");
        let _ = std::fs::remove_file(&db);
    }

    #[test]
    fn search_errors() {
        let db = temp_db("err");
        assert!(search(&db, "", 10, false).is_err());
        assert!(import_text(&db, "", "a:b").is_err());
        let _ = std::fs::remove_file(&db);
    }
}
