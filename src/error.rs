//! NotebookLM error types.
//!
//! Grounding: ∂(Boundary) — error classification at domain boundary.

use nexcore_error::Error;

/// Errors from NotebookLM operations.
#[derive(Debug, Error)]
pub enum NotebookLmError {
    /// Notebook not found in library.
    #[error("notebook not found: {0}")]
    NotebookNotFound(String),

    /// Session not found.
    #[error("session not found: {0}")]
    SessionNotFound(String),

    /// No active notebook selected.
    #[error("no active notebook selected — use nlm_select_notebook first")]
    NoActiveNotebook,

    /// JSON serialization/deserialization error.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// File I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Browser not running (Phase 2).
    #[error("browser not running — use nlm_setup_auth first")]
    BrowserNotRunning,

    /// Authentication required.
    #[error("not authenticated — use nlm_setup_auth to log in")]
    NotAuthenticated,

    /// Query timeout.
    #[error("query timed out after {0}ms")]
    Timeout(u64),

    /// DOM selector not found (Phase 2).
    #[error("selector not found: {0}")]
    SelectorNotFound(String),

    /// Generic operational error.
    #[error("{0}")]
    Other(String),
}
