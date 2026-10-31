use crate::models::{Target, TargetKind};
use std::net::IpAddr;
use std::path::Path;

pub fn detect_target(query: &str) -> Target {
    let raw = query.trim().to_string();
    let normalized = raw.trim_start_matches('@').to_lowercase();
    let kind = classify(&raw, &normalized);
    Target {
        kind,
        raw,
        normalized,
    }
}

fn classify(raw: &str, normalized: &str) -> TargetKind {
    if Path::new(raw).extension().is_some_and(|e| FILE_EXTS.contains(&e.to_string_lossy().to_lowercase().as_str())) && Path::new(raw).is_file() {
        return TargetKind::File;
    }
    if let Ok(ip) = normalized.parse::<IpAddr>() {
        let _ = ip;
        return TargetKind::IpAddress;
    }
    if is_email(normalized) {
        return TargetKind::Email;
    }
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    if (raw.starts_with('+') || digits.len() >= 7) && digits.len() <= 15 && raw.chars().all(|c| c.is_ascii_digit() || "+ ()-.()-".contains(c)) && digits.len() >= 7 {
        return TargetKind::PhoneNumber;
    }
    if is_domain_like(normalized) {
        return TargetKind::Domain;
    }
    if is_username_like(normalized) {
        return TargetKind::Username;
    }
    TargetKind::Unknown
}

fn is_email(s: &str) -> bool {
    let Some((local, domain)) = s.split_once('@') else {
        return false;
    };
    !local.is_empty() && is_domain_like(domain) && s.chars().all(|c| c.is_ascii_alphanumeric() || ".@_-+".contains(c))
}

fn is_domain_like(s: &str) -> bool {
    let Some((_, tld)) = s.rsplit_once('.') else {
        return false;
    };
    tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic()) && !s.contains(' ') && s.len() >= 4 && s.chars().all(|c| c.is_ascii_alphanumeric() || ".-".contains(c))
}

fn is_username_like(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.chars().all(|c| c.is_alphanumeric() || "._-".contains(c)) && s.chars().next().is_some_and(|c| c.is_alphanumeric())
}

const FILE_EXTS: &[&str] = &["jpg", "jpeg", "png", "heic", "webp", "tiff", "tif", "bmp"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_kinds() {
        assert_eq!(detect_target("mel0k1").kind, TargetKind::Username);
        assert_eq!(detect_target("@mel0k1").kind, TargetKind::Username);
        assert_eq!(detect_target("a@b.co").kind, TargetKind::Email);
        assert_eq!(detect_target("8.8.8.8").kind, TargetKind::IpAddress);
        assert_eq!(detect_target("example.org").kind, TargetKind::Domain);
        assert_eq!(detect_target("+79261234567").kind, TargetKind::PhoneNumber);
        assert_eq!(detect_target("sub.deep.example.co.uk").kind, TargetKind::Domain);
    }
}
