//! NotebookLM domain types.
//!
//! Grounding: ∃(Existence) + κ(Comparison) — typed entities for library and sessions.

use nexcore_chrono::DateTime;
use serde::{Deserialize, Serialize};

/// A notebook in the library.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notebook {
    /// Unique identifier (user-provided or auto-generated).
    pub id: String,
    /// Display name.
    pub name: String,
    /// NotebookLM URL.
    pub url: String,
    /// Description of the notebook's content.
    pub description: String,
    /// Topics covered.
    #[serde(default)]
    pub topics: Vec<String>,
    /// Types of content (e.g., "documentation", "examples").
    #[serde(default)]
    pub content_types: Vec<String>,
    /// When to use this notebook.
    #[serde(default)]
    pub use_cases: Vec<String>,
    /// Organization tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// When added to library.
    pub created_at: DateTime,
    /// Last modified.
    pub updated_at: DateTime,
}

/// A chat session with a notebook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Session ID.
    pub id: String,
    /// Notebook ID this session is for.
    pub notebook_id: String,
    /// Number of messages exchanged.
    pub message_count: u32,
    /// When the session was created.
    pub created_at: DateTime,
    /// Last activity time.
    pub last_activity: DateTime,
}

/// Health status of the NotebookLM subsystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    /// Whether browser is running (Phase 2 — always false in Phase 1).
    pub browser_running: bool,
    /// Whether authenticated with Google (Phase 2 — always false in Phase 1).
    pub authenticated: bool,
    /// Number of notebooks in library.
    pub library_size: usize,
    /// Number of active sessions.
    pub active_sessions: usize,
    /// Data directory path.
    pub data_dir: String,
}

/// Persistent auth state — tracks when authentication last succeeded.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuthState {
    /// Whether the last auth attempt was successful.
    pub authenticated: bool,
    /// When auth last succeeded (ISO 8601).
    pub last_authenticated: Option<DateTime>,
    /// Google account email (if detected).
    pub account_email: Option<String>,
}

impl AuthState {
    /// Auth is considered valid if authenticated within the last 30 days.
    pub fn is_valid(&self) -> bool {
        if !self.authenticated {
            return false;
        }
        match self.last_authenticated {
            Some(ts) => {
                let age = DateTime::now().signed_duration_since(ts);
                age.num_days() < 30
            }
            None => false,
        }
    }
}

/// Library statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryStats {
    /// Total notebooks.
    pub total_notebooks: usize,
    /// Total unique topics across all notebooks.
    pub total_topics: usize,
    /// Total unique tags.
    pub total_tags: usize,
    /// Most recent notebook added.
    pub most_recent: Option<String>,
}
