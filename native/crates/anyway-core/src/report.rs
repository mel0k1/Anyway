use printpdf::{
    Color, Mm, Op, ParsedFont, PdfDocument, PdfPage, PdfSaveOptions, Point, Pt, Rgb, TextItem,
};
use serde_json::Value;

/// A4 report renderer: target summary, per-module findings, correlation
/// clusters and the timeline, laid out with absolute text positioning and
/// manual pagination. Uses an embedded DejaVu font so Cyrillic works on any
/// platform.
const FONT: &[u8] = include_bytes!("../../../assets/fonts/DejaVuSans.ttf");

const PAGE_W_MM: f32 = 210.0;
const PAGE_H_MM: f32 = 297.0;
const MARGIN_MM: f32 = 22.0;
const BOTTOM_MM: f32 = 20.0;

struct Line {
    size: f32, // pt
    bold_gap: bool,
    text: String,
}

fn wrap(text: &str, max_chars: usize) -> Vec<String> {
    let mut out = Vec::new();
    for paragraph in text.lines() {
        if paragraph.chars().count() <= max_chars {
            out.push(paragraph.to_string());
            continue;
        }
        let mut current = String::new();
        for word in paragraph.split(' ') {
            if current.chars().count() + word.chars().count() + 1 > max_chars && !current.is_empty()
            {
                out.push(current.clone());
                current.clear();
            }
            if word.chars().count() > max_chars {
                if !current.is_empty() {
                    out.push(current.clone());
                    current.clear();
                }
                out.push(word.chars().take(max_chars).collect());
                continue;
            }
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
        if !current.is_empty() {
            out.push(current);
        }
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

fn sanitize(text: &str) -> String {
    // control characters break PDF text streams
    text.chars().filter(|c| !c.is_control()).collect::<String>()
}

fn header_color() -> Color {
    Color::Rgb(Rgb {
        r: 0.35,
        g: 0.18,
        b: 0.62,
        icc_profile: None,
    })
}

fn body_color() -> Color {
    Color::Rgb(Rgb {
        r: 0.12,
        g: 0.12,
        b: 0.14,
        icc_profile: None,
    })
}

pub fn generate(snapshot: &Value) -> Result<Vec<u8>, String> {
    let font =
        ParsedFont::from_bytes(FONT, 0, &mut Vec::new()).ok_or("embedded font failed to parse")?;
    let mut doc = PdfDocument::new("Anyway report");
    let font_id = doc.add_font(&font);

    let mut lines: Vec<Line> = Vec::new();
    let target = snapshot["target"]["raw"].as_str().unwrap_or("unknown");
    let kind = snapshot["target"]["kind"].as_str().unwrap_or("unknown");
    let status = snapshot["status"].as_str().unwrap_or("?");
    let started = snapshot["started_at"].as_i64().unwrap_or(0);
    let started_str = chrono_date(started);

    lines.push(Line {
        size: 20.0,
        bold_gap: true,
        text: format!("Anyway report: {target}"),
    });
    lines.push(Line {
        size: 10.0,
        bold_gap: false,
        text: format!("Type: {kind} · status: {status} · started {started_str}"),
    });
    lines.push(Line {
        size: 10.0,
        bold_gap: false,
        text: format!("Scan id: {}", snapshot["scan_id"].as_str().unwrap_or("-")),
    });
    lines.push(Line {
        size: 10.0,
        bold_gap: true,
        text: String::new(),
    });

    // findings grouped by module
    let mut modules: Vec<(String, Vec<&Value>)> = Vec::new();
    if let Some(findings) = snapshot["findings"].as_array() {
        for f in findings {
            let module = f["module_id"].as_str().unwrap_or("?").to_string();
            match modules.iter_mut().find(|(m, _)| *m == module) {
                Some((_, v)) => v.push(f),
                None => modules.push((module, vec![f])),
            }
        }
    }
    lines.push(Line {
        size: 14.0,
        bold_gap: true,
        text: format!(
            "Findings ({})",
            snapshot["findings"]
                .as_array()
                .map(|a| a.len())
                .unwrap_or(0)
        ),
    });
    for (module, items) in &modules {
        lines.push(Line {
            size: 11.5,
            bold_gap: true,
            text: format!(
                "{module} — {} finding{}",
                items.len(),
                if items.len() == 1 { "" } else { "s" }
            ),
        });
        for f in items.iter().take(250) {
            let label = f["label"].as_str().unwrap_or("?");
            let value = f["value"].as_str().unwrap_or("");
            let conf = f["confidence"].as_str().unwrap_or("?");
            let text = sanitize(&format!("{label}: {value} [{conf}]"));
            for (i, piece) in wrap(&text, 100).into_iter().enumerate() {
                let prefix = if i == 0 { "" } else { "    " };
                lines.push(Line {
                    size: 9.0,
                    bold_gap: false,
                    text: format!("{prefix}{piece}"),
                });
            }
        }
    }

    // correlation clusters
    let report = crate::resolve::resolve(snapshot, 45);
    let clusters = report.to_json(45)["clusters"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let linked: Vec<&Value> = clusters
        .iter()
        .filter(|c| c["size"].as_u64().unwrap_or(0) > 1)
        .collect();
    if !linked.is_empty() {
        lines.push(Line {
            size: 14.0,
            bold_gap: true,
            text: String::new(),
        });
        lines.push(Line {
            size: 14.0,
            bold_gap: true,
            text: format!("Correlation clusters ({})", linked.len()),
        });
        for c in linked.iter().take(30) {
            let label = c["label"].as_str().unwrap_or("?");
            let size = c["size"].as_u64().unwrap_or(0);
            lines.push(Line {
                size: 10.5,
                bold_gap: true,
                text: format!("· {label} ({size} linked identities)"),
            });
            if let Some(members) = c["members"].as_array() {
                for m in members.iter().take(10) {
                    let platform = m["platform"].as_str().unwrap_or("?");
                    let handle = m["handle"].as_str().unwrap_or("");
                    let url = m["url"].as_str().unwrap_or("");
                    let text = if url.is_empty() {
                        format!("    {platform}: {handle}")
                    } else {
                        format!("    {platform}: {handle} — {url}")
                    };
                    for piece in wrap(&sanitize(&text), 96) {
                        lines.push(Line {
                            size: 9.0,
                            bold_gap: false,
                            text: piece,
                        });
                    }
                }
            }
        }
    }

    // timeline
    let timeline = crate::resolve::build_timeline(snapshot)["events"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if !timeline.is_empty() {
        lines.push(Line {
            size: 14.0,
            bold_gap: true,
            text: String::new(),
        });
        lines.push(Line {
            size: 14.0,
            bold_gap: true,
            text: format!("Timeline ({})", timeline.len()),
        });
        for e in timeline.iter().take(120) {
            let date = e["date"].as_str().unwrap_or("?");
            let label = e["label"].as_str().unwrap_or("");
            let kind = e["kind"].as_str().unwrap_or("");
            let text = sanitize(&format!("{date} — {label} [{kind}]"));
            for (i, piece) in wrap(&text, 100).into_iter().enumerate() {
                let prefix = if i == 0 { "" } else { "    " };
                lines.push(Line {
                    size: 9.0,
                    bold_gap: false,
                    text: format!("{prefix}{piece}"),
                });
            }
        }
    }

    // ---- paginate ----
    let mm_per_pt: f32 = 0.3528;
    let mut pages: Vec<PdfPage> = Vec::new();
    let mut ops: Vec<Op> = Vec::new();
    let mut y: f32 = PAGE_H_MM - MARGIN_MM;
    let mut page_first = true;

    macro_rules! flush_page {
        () => {{
            if !ops.is_empty() {
                pages.push(PdfPage::new(
                    Mm(PAGE_W_MM),
                    Mm(PAGE_H_MM),
                    std::mem::take(&mut ops),
                ));
                y = PAGE_H_MM - MARGIN_MM;
                page_first = true;
            }
        }};
        (last) => {{
            if !ops.is_empty() {
                pages.push(PdfPage::new(
                    Mm(PAGE_W_MM),
                    Mm(PAGE_H_MM),
                    std::mem::take(&mut ops),
                ));
            }
        }};
    }

    for line in &lines {
        let lh_mm = line.size * mm_per_pt * 1.45;
        if y < BOTTOM_MM + lh_mm {
            ops.push(Op::EndTextSection);
            flush_page!();
        }
        if page_first {
            ops.push(Op::StartTextSection);
            page_first = false;
        }
        ops.push(Op::SetTextCursor {
            pos: Point::new(Mm(MARGIN_MM), Mm(y)),
        });
        ops.push(Op::SetFontSize {
            font: font_id.clone(),
            size: Pt(line.size),
        });
        ops.push(Op::SetLineHeight {
            lh: Pt(line.size * 1.45),
        });
        ops.push(Op::SetFillColor {
            col: if line.size >= 14.0 {
                header_color()
            } else {
                body_color()
            },
        });
        ops.push(Op::WriteText {
            font: font_id.clone(),
            items: vec![TextItem::Text(line.text.clone())],
        });
        y -= lh_mm;
        if line.bold_gap {
            y -= lh_mm * 0.35;
        }
    }
    ops.push(Op::EndTextSection);
    flush_page!(last);

    if pages.is_empty() {
        pages.push(PdfPage::new(
            Mm(PAGE_W_MM),
            Mm(PAGE_H_MM),
            vec![Op::StartTextSection, Op::EndTextSection],
        ));
    }
    let warnings = &mut Vec::new();
    let bytes = doc
        .with_pages(pages)
        .save(&PdfSaveOptions::default(), warnings);
    Ok(bytes)
}

fn chrono_date(ts: i64) -> String {
    if ts <= 0 {
        return "-".into();
    }
    let secs = ts;
    let days = secs / 86400;
    // civil-from-days algorithm (Howard Hinnant)
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    let rem = secs % 86400;
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    format!("{y:04}-{m:02}-{d:02} {h:02}:{mi:02}:{s:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn generates_multi_page_pdf() {
        let snap = json!({
            "scan_id": "scan-1",
            "status": "done",
            "started_at": 1700000000,
            "target": {"raw": "mel0k1", "kind": "username", "normalized": "mel0k1"},
            "findings": [
                {"module_id": "username", "category": "social_profile", "label": "GitHub", "value": "https://github.com/mel0k1", "confidence": "confirmed"},
                {"module_id": "username", "category": "social_profile", "label": "GitLab", "value": "https://gitlab.com/mel0k1", "confidence": "confirmed"}
            ],
            "log": []
        });
        let bytes = generate(&snap).unwrap();
        assert!(bytes.len() > 2000);
        assert_eq!(&bytes[..5], b"%PDF-");
    }

    #[test]
    fn wraps_long_text() {
        let parts = wrap(&"a ".repeat(120), 100);
        assert!(parts.len() >= 2);
        assert!(parts.iter().all(|p| p.chars().count() <= 101));
        assert_eq!(wrap("short", 100), vec!["short"]);
        assert_eq!(wrap("", 100), vec![String::new()]);
    }

    #[test]
    fn formats_unix_dates() {
        assert_eq!(chrono_date(1700000000), "2023-11-14 22:13:20");
        assert_eq!(chrono_date(0), "-");
    }
}
