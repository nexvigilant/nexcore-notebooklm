//! Chrome lifecycle management for NotebookLM.
//!
//! Independent browser instance — NOT sharing nexcore-browser's global state.
//! Uses persistent user-data-dir for auth cookie survival across sessions.
//! Grounding: ∃(Existence) + ς(State) — browser process lifecycle.

use std::sync::{Arc, OnceLock};

use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::page::Page;
use futures::StreamExt;
use parking_lot::Mutex;
use tracing::{debug, info, warn};

use crate::error::NotebookLmError;
use crate::persistence;
use crate::stealth;

/// Global NLM browser state (separate from nexcore-browser).
static NLM_BROWSER: OnceLock<Arc<Mutex<NlmBrowserState>>> = OnceLock::new();

/// Internal browser state for NotebookLM.
pub struct NlmBrowserState {
    /// The browser instance.
    browser: Option<Arc<Browser>>,
    /// Handler task (keeps CDP connection alive).
    handler_task: Option<tokio::task::JoinHandle<()>>,
    /// Active page for NotebookLM interaction.
    page: Option<Arc<Page>>,
    /// Whether stealth scripts have been injected on the current page.
    stealth_injected: bool,
}

impl NlmBrowserState {
    fn new() -> Self {
        Self {
            browser: None,
            handler_task: None,
            page: None,
            stealth_injected: false,
        }
    }
}

/// Get or initialize the global NLM browser context.
fn get_state() -> Arc<Mutex<NlmBrowserState>> {
    NLM_BROWSER
        .get_or_init(|| Arc::new(Mutex::new(NlmBrowserState::new())))
        .clone()
}

/// Check if the browser is currently running.
pub fn is_running() -> bool {
    let state = get_state();
    let s = state.lock();
    s.browser.is_some()
}

/// Launch a new Chrome instance with stealth args and persistent profile.
///
/// If browser is already running, returns Ok without relaunching.
pub async fn launch() -> Result<(), NotebookLmError> {
    {
        let state = get_state();
        let s = state.lock();
        if s.browser.is_some() {
            debug!("NLM browser already running");
            return Ok(());
        }
    }

    let profile_dir = persistence::chrome_profile_path()?;

    // Ensure profile directory exists
    if !profile_dir.exists() {
        std::fs::create_dir_all(&profile_dir)?;
    }

    // Build config with persistent profile and stealth args.
    // CRITICAL: use .user_data_dir() — not --user-data-dir arg —
    // because chromiumoxide sets its own default that would override a raw arg.
    let mut builder = BrowserConfig::builder()
        .with_head() // Must be headed for Google auth
        .user_data_dir(&profile_dir)
        .window_size(1920, 1080);

    for arg in stealth::stealth_args() {
        builder = builder.arg(&arg);
    }

    let config = builder
        .build()
        .map_err(|e| NotebookLmError::Other(format!("browser config error: {e}")))?;

    let (browser, mut handler) = Browser::launch(config)
        .await
        .map_err(|e| NotebookLmError::Other(format!("browser launch failed: {e}")))?;

    // Spawn handler task to keep CDP connection alive
    let handle = tokio::spawn(async move {
        while (handler.next().await).is_some() {
            // Process CDP messages — keeps connection alive
        }
        warn!("NLM browser handler exited — browser disconnected");
    });

    let state = get_state();
    let mut s = state.lock();
    s.browser = Some(Arc::new(browser));
    s.handler_task = Some(handle);
    s.page = None;
    s.stealth_injected = false;

    info!(
        "NLM browser launched with persistent profile at {}",
        profile_dir.display()
    );
    Ok(())
}

/// Ensure browser is running, launching if needed.
pub async fn ensure_running() -> Result<(), NotebookLmError> {
    if !is_running() {
        launch().await?;
    }
    Ok(())
}

/// Get or create the active page, injecting stealth scripts.
pub async fn get_or_create_page(url: &str) -> Result<Arc<Page>, NotebookLmError> {
    ensure_running().await?;

    let state = get_state();

    // Check if we already have a page
    {
        let s = state.lock();
        if let Some(ref page) = s.page {
            return Ok(page.clone());
        }
    }

    // Create new page
    let browser = {
        let s = state.lock();
        s.browser
            .clone()
            .ok_or(NotebookLmError::BrowserNotRunning)?
    };

    let page = browser
        .new_page(url)
        .await
        .map_err(|e| NotebookLmError::Other(format!("failed to create page: {e}")))?;

    // Inject stealth scripts before any navigation
    stealth::inject_stealth(&page).await?;

    let page = Arc::new(page);

    {
        let mut s = state.lock();
        s.page = Some(page.clone());
        s.stealth_injected = true;
    }

    debug!("NLM page created at {url}");
    Ok(page)
}

/// Navigate the active page to a URL.
pub async fn navigate_to(url: &str) -> Result<Arc<Page>, NotebookLmError> {
    let page = get_or_create_page("about:blank").await?;

    page.goto(url)
        .await
        .map_err(|e| NotebookLmError::Other(format!("navigation failed: {e}")))?;

    debug!("NLM page navigated to {url}");
    Ok(page)
}

/// Close the browser and clean up.
pub async fn close() -> Result<(), NotebookLmError> {
    let state = get_state();
    let mut s = state.lock();

    // Drop page first
    s.page = None;
    s.stealth_injected = false;

    // Abort handler task
    if let Some(handle) = s.handler_task.take() {
        handle.abort();
    }

    // Drop browser (closes Chrome process)
    s.browser = None;

    info!("NLM browser closed");
    Ok(())
}

/// Check if we have an authenticated session.
///
/// Two-layer check:
/// 1. AuthState persisted after successful login (primary — has 30-day TTL)
/// 2. Chrome cookie files exist (fallback — file presence only)
///
/// Returns true only if auth state is valid AND cookie files exist.
pub fn has_auth_cookies() -> bool {
    // Layer 1: Check persisted auth state (written after confirmed login)
    let auth_state = match persistence::auth_state_path() {
        Ok(path) => persistence::read_json::<crate::AuthState>(&path).unwrap_or_default(),
        Err(_) => return false,
    };

    if !auth_state.is_valid() {
        return false;
    }

    // Layer 2: Cookie files still exist (haven't been wiped)
    let profile = match persistence::chrome_profile_path() {
        Ok(p) => p,
        Err(_) => return false,
    };

    let cookies_path = profile.join("Default").join("Cookies");
    let network_cookies = profile.join("Default").join("Network").join("Cookies");

    cookies_path.exists() || network_cookies.exists()
}

/// Record a successful authentication.
///
/// Call this after confirming the user has logged in.
/// Persists auth state with timestamp for TTL-based validation.
pub fn record_auth_success(email: Option<String>) {
    let state = crate::AuthState {
        authenticated: true,
        last_authenticated: Some(nexcore_chrono::DateTime::now()),
        account_email: email,
    };

    if let Ok(path) = persistence::auth_state_path() {
        if let Err(e) = persistence::write_json(&path, &state) {
            tracing::warn!("failed to persist auth state: {e}");
        } else {
            tracing::info!("NLM auth state recorded");
        }
    }
}
