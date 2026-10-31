use crate::error::CoreError;
use std::path::{Path, PathBuf};
use std::time::Duration;

const BROWSER_ENV_VARS: &[&str] = &["ANYWAY_BROWSER", "ANYWAY_CHROME"];

#[cfg(target_os = "windows")]
const PATH_NAMES: &[&str] = &["chrome.exe", "msedge.exe", "chromium.exe", "brave.exe"];

#[cfg(target_os = "macos")]
const PATH_NAMES: &[&str] = &[
    "Google Chrome",
    "Chromium",
    "Microsoft Edge",
    "Brave Browser",
];

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const PATH_NAMES: &[&str] = &[
    "chromium",
    "chromium-browser",
    "google-chrome",
    "google-chrome-stable",
    "microsoft-edge",
    "microsoft-edge-stable",
    "brave-browser",
];

#[cfg(target_os = "macos")]
const ABSOLUTE_CANDIDATES: &[&str] = &[
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
];

#[cfg(target_os = "windows")]
const ABSOLUTE_CANDIDATES: &[&str] = &[
    r"C:\Program Files\Google\Chrome\Application\chrome.exe",
    r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
    r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
    r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
];

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const ABSOLUTE_CANDIDATES: &[&str] = &[
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
    "/usr/bin/google-chrome",
    "/usr/bin/google-chrome-stable",
    "/usr/bin/microsoft-edge",
    "/usr/bin/microsoft-edge-stable",
    "/usr/bin/brave-browser",
    "/snap/bin/chromium",
    "/opt/google/chrome/chrome",
];

pub fn find_browser() -> Option<PathBuf> {
    for var in BROWSER_ENV_VARS {
        if let Ok(path) = std::env::var(var) {
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                let candidate = PathBuf::from(trimmed);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    for name in PATH_NAMES {
        if let Some(path) = which(name) {
            return Some(path);
        }
    }
    for candidate in ABSOLUTE_CANDIDATES {
        let path = PathBuf::from(candidate);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

fn which(name: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    let exe_suffix = if cfg!(target_os = "windows") {
        ".exe"
    } else {
        ""
    };
    for dir in std::env::split_paths(&paths) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        if !exe_suffix.is_empty() {
            let with_suffix = dir.join(format!("{name}{exe_suffix}"));
            if with_suffix.is_file() {
                return Some(with_suffix);
            }
        }
    }
    None
}

pub fn browser_args(url: &str, timeout_ms: u64, profile_dir: &Path) -> Vec<String> {
    vec![
        "--headless=new".to_string(),
        "--disable-gpu".to_string(),
        "--disable-extensions".to_string(),
        "--disable-sync".to_string(),
        "--no-first-run".to_string(),
        "--no-default-browser-check".to_string(),
        "--disable-crash-reporter".to_string(),
        "--disable-background-networking".to_string(),
        format!("--user-data-dir={}", profile_dir.display()),
        "--virtual-time-budget=10000".to_string(),
        format!("--timeout={timeout_ms}"),
        "--dump-dom".to_string(),
        url.to_string(),
    ]
}

pub fn profile_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("anyway-browser-{}-{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub async fn render_dom(browser: &Path, url: &str, timeout: Duration) -> Result<String, CoreError> {
    let profile = profile_dir("dom");
    let args = browser_args(url, timeout.as_millis() as u64, &profile);
    let mut output = spawn_browser(browser, &args, timeout).await;
    if !output
        .as_ref()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false)
    {
        let legacy: Vec<String> = args
            .iter()
            .map(|a| {
                if a == "--headless=new" {
                    "--headless".to_string()
                } else {
                    a.clone()
                }
            })
            .collect();
        output = spawn_browser(browser, &legacy, timeout).await;
    }
    let _ = std::fs::remove_dir_all(&profile);
    let output = output?;
    if output.status.success() && !output.stdout.is_empty() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    Err(CoreError::Parse(format!(
        "browser exited with {} and empty dom",
        output.status
    )))
}

async fn spawn_browser(
    browser: &Path,
    args: &[String],
    timeout: Duration,
) -> Result<std::process::Output, CoreError> {
    let mut command = tokio::process::Command::new(browser);
    command
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    #[cfg(target_os = "windows")]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let future = command.output();
    match tokio::time::timeout(timeout + Duration::from_secs(10), future).await {
        Ok(result) => result.map_err(|e| CoreError::Parse(format!("browser spawn failed: {e}"))),
        Err(_) => Err(CoreError::Parse("browser render timed out".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_cover_headless_keys() {
        let profile = std::env::temp_dir().join("anyway-test-profile");
        let args = browser_args("https://example.org/u/x", 15000, &profile);
        assert!(args.contains(&"--headless=new".to_string()));
        assert!(args.contains(&"--dump-dom".to_string()));
        assert!(args.iter().any(|a| a == "--virtual-time-budget=10000"));
        assert!(args.iter().any(|a| a == "--timeout=15000"));
        assert!(args.iter().any(|a| a.starts_with("--user-data-dir=")));
        assert_eq!(
            args.last().map(|s| s.as_str()),
            Some("https://example.org/u/x")
        );
    }

    #[test]
    fn find_browser_never_panics() {
        let found = find_browser();
        if let Some(path) = found {
            assert!(path.is_file(), "reported browser must exist: {path:?}");
        }
    }

    #[test]
    fn profile_dir_is_unique_per_tag() {
        let a = profile_dir("t1");
        let b = profile_dir("t2");
        assert!(a.starts_with(std::env::temp_dir()));
        assert!(b.starts_with(std::env::temp_dir()));
        assert_ne!(a, b);
        let _ = std::fs::remove_dir_all(a);
        let _ = std::fs::remove_dir_all(b);
    }
}
