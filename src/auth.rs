//! Google authentication flow for NotebookLM.
//!
//! Opens Chrome with the persistent profile so user can log in manually.
//! Auth cookies persist in the user-data-dir across sessions.
//! Grounding: ∂(Boundary) + π(Persistence) — auth boundary with persistent credentials.

use crate::browser;
use crate::error::NotebookLmError;
use crate::persistence;
use tracing::info;

/// NotebookLM sign-in URL.
const NOTEBOOKLM_URL: &str = "https://notebooklm.google.com";

/// Run the interactive setup auth flow.
///
/// 1. Launches Chrome with persistent profile + stealth
/// 2. Navigates to NotebookLM
/// 3. Returns — user completes login manually in the opened browser
///
/// After the user logs in, cookies are saved to the persistent Chrome profile.
/// Subsequent launches will auto-authenticate.
pub async fn setup_auth() -> Result<AuthResult, NotebookLmError> {
    // Launch browser (will reuse if already running)
    browser::launch().await?;

    // Navigate to NotebookLM — triggers Google sign-in if needed
    browser::navigate_to(NOTEBOOKLM_URL).await?;

    info!("NLM auth: browser opened at {NOTEBOOKLM_URL} — waiting for user login");

    Ok(AuthResult {
        browser_opened: true,
        url: NOTEBOOKLM_URL.to_string(),
        message: "Browser opened at NotebookLM. Complete Google sign-in in the browser window. \
                  Cookies will be saved automatically to the persistent Chrome profile."
            .to_string(),
    })
}

/// Re-authenticate: close browser, wipe auth data, launch fresh.
pub async fn re_auth() -> Result<AuthResult, NotebookLmError> {
    // Close existing browser if running
    if browser::is_running() {
        browser::close().await?;
    }

    // Wipe the Chrome profile to force fresh login
    let profile_dir = persistence::chrome_profile_path()?;
    if profile_dir.exists() {
        std::fs::remove_dir_all(&profile_dir)
            .map_err(|e| NotebookLmError::Other(format!("failed to remove profile: {e}")))?;
        info!(
            "NLM auth: wiped Chrome profile at {}",
            profile_dir.display()
        );
    }

    // Now run setup_auth with clean state
    setup_auth().await
}

/// Result of an auth operation.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AuthResult {
    /// Whether the browser was successfully opened.
    pub browser_opened: bool,
    /// URL the browser was navigated to.
    pub url: String,
    /// Human-readable status message.
    pub message: String,
}
