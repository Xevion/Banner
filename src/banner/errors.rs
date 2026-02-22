//! Error types for the Banner API client.

#[derive(Debug, thiserror::Error)]
pub enum BannerApiError {
    #[error("Banner session is invalid or expired: {0}")]
    InvalidSession(String),
    #[error("Failed to parse response")]
    ParseFailed {
        status: u16,
        url: String,
        #[source]
        source: anyhow::Error,
    },
    #[error(transparent)]
    RequestFailed(#[from] anyhow::Error),
}
