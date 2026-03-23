//! # nexcore-notebooklm
//!
//! NotebookLM integration for NexVigilant — replaces the npm `notebooklm-mcp` server
//! with a native Rust implementation wired into the nexcore MCP dispatcher.
//!
//! ## Architecture
//!
//! - **Library** (`library.rs`): Notebook metadata CRUD, JSON-persisted
//! - **Sessions** (`session.rs`): Chat session lifecycle management
//! - **Persistence** (`persistence.rs`): Atomic JSON read/write
//! - **Browser** (`browser.rs`): Chrome lifecycle (Phase 2)
//! - **Auth** (`auth.rs`): Google authentication flow (Phase 2)
//! - **Notebook** (`notebook.rs`): DOM query interaction (Phase 3)
//! - **Stealth** (`stealth.rs`): CDP anti-detection (Phase 2)
//! - **Selectors** (`selectors.rs`): DOM selector constants
//!
//! ## MCP Tools (16)
//!
//! Phase 1 (sync): `nlm_add_notebook`, `nlm_list_notebooks`, `nlm_get_notebook`,
//! `nlm_select_notebook`, `nlm_update_notebook`, `nlm_remove_notebook`,
//! `nlm_search_notebooks`, `nlm_get_library_stats`, `nlm_list_sessions`,
//! `nlm_close_session`, `nlm_reset_session`
//!
//! Phase 2 (async): `nlm_setup_auth`, `nlm_re_auth`, `nlm_get_health`
//!
//! Phase 3 (async): `nlm_ask_question`, `nlm_cleanup_data`
//!
//! Grounding: μ(Mapping) + π(Persistence) — maps questions to notebook knowledge,
//! persists library and session state.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(missing_docs)]
pub mod auth;
pub mod browser;
pub mod error;
pub mod library;
pub mod notebook;
pub mod persistence;
pub mod selectors;
pub mod session;
pub mod stealth;
pub mod types;

// Re-export primary types
pub use auth::AuthResult;
pub use error::NotebookLmError;
pub use library::Library;
pub use notebook::QueryResult;
pub use session::SessionStore;
pub use types::{AuthState, HealthStatus, LibraryStats, Notebook, Session};
