//! CDP stealth: JS injection and Chrome launch args.
//!
//! Prevents Google from detecting automated browser.
//! Grounding: ∂(Boundary) + ν(Frequency) — boundary evasion at detection frequency.

use crate::error::NotebookLmError;
use chromiumoxide::page::Page;

/// Chrome launch arguments for stealth operation.
///
/// NOTE: `--user-data-dir` is NOT included here — it must be set via
/// `BrowserConfig::builder().user_data_dir()` to avoid being overridden
/// by chromiumoxide's default temp directory.
pub fn stealth_args() -> Vec<String> {
    vec![
        "--disable-blink-features=AutomationControlled".to_string(),
        "--no-first-run".to_string(),
        "--disable-default-apps".to_string(),
        "--disable-infobars".to_string(),
        "--disable-extensions".to_string(),
        "--window-size=1920,1080".to_string(),
        "--disable-background-timer-throttling".to_string(),
        "--disable-renderer-backgrounding".to_string(),
        "--no-sandbox".to_string(),
        "--disable-dev-shm-usage".to_string(),
    ]
}

/// JavaScript to inject via `Page.addScriptToEvaluateOnNewDocument` before navigation.
///
/// Patches:
/// 1. `navigator.webdriver` → undefined
/// 2. `navigator.plugins` → non-empty array
/// 3. `navigator.languages` → `['en-US', 'en']`
/// 4. `Permissions.query` → patch notifications
/// 5. `chrome.runtime` → mock object
/// 6. WebGL vendor/renderer → Intel values
fn stealth_js() -> &'static str {
    r#"
    // 1. Remove webdriver flag
    Object.defineProperty(navigator, 'webdriver', { get: () => undefined });

    // 2. Mock plugins
    Object.defineProperty(navigator, 'plugins', {
        get: () => [
            { name: 'Chrome PDF Plugin', filename: 'internal-pdf-viewer' },
            { name: 'Chrome PDF Viewer', filename: 'mhjfbmdgcfjbbpaeojofohoefgiehjai' },
            { name: 'Native Client', filename: 'internal-nacl-plugin' }
        ]
    });

    // 3. Languages
    Object.defineProperty(navigator, 'languages', {
        get: () => ['en-US', 'en']
    });

    // 4. Permissions
    if (window.Permissions && window.Permissions.prototype) {
        const origQuery = window.Permissions.prototype.query;
        window.Permissions.prototype.query = (parameters) => (
            parameters.name === 'notifications'
                ? Promise.resolve({ state: Notification.permission })
                : origQuery(parameters)
        );
    }

    // 5. Chrome runtime mock
    window.chrome = window.chrome || {};
    window.chrome.runtime = window.chrome.runtime || {
        connect: () => {},
        sendMessage: () => {}
    };

    // 6. WebGL vendor/renderer
    const getParameter = WebGLRenderingContext.prototype.getParameter;
    WebGLRenderingContext.prototype.getParameter = function(parameter) {
        if (parameter === 37445) return 'Intel Inc.';
        if (parameter === 37446) return 'Intel Iris OpenGL Engine';
        return getParameter.call(this, parameter);
    };
    "#
}

/// Inject stealth scripts into a page so they run on every new document load.
pub async fn inject_stealth(page: &Page) -> Result<(), NotebookLmError> {
    page.evaluate_on_new_document(stealth_js())
        .await
        .map_err(|e| NotebookLmError::Other(format!("stealth injection failed: {e}")))?;
    tracing::debug!("stealth scripts injected");
    Ok(())
}
