#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("http client error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("cancelled")]
    Cancelled,
}
