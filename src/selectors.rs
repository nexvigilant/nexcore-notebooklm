//! DOM selector constants for NotebookLM.
//!
//! Single-file update point when Google changes their UI.
//! Grounding: λ(Location) — fixed addresses in DOM space.
//!
//! Sourced from npm notebooklm-mcp server analysis (2026-02).

// ── Chat Interface ──────────────────────────────────────────────────────────

/// Chat input textarea (primary).
pub const CHAT_INPUT: &str = "textarea.query-box-input";

/// Chat input textarea (fallback — German localization).
pub const CHAT_INPUT_FALLBACK: &str = "textarea[aria-label=\"Feld für Anfragen\"]";

// ── Response Extraction ─────────────────────────────────────────────────────

/// Container for assistant (bot) responses.
pub const RESPONSE_CONTAINER: &str = ".to-user-container";

/// Text content within a response container.
pub const RESPONSE_TEXT: &str = ".message-text-content";

/// Full selector: response text within response container.
pub const RESPONSE_FULL: &str = ".to-user-container .message-text-content";

// ── Loading / Thinking ──────────────────────────────────────────────────────

/// Thinking indicator (visible while NotebookLM processes a query).
pub const THINKING_INDICATOR: &str = "div.thinking-message";

// ── Error Detection ─────────────────────────────────────────────────────────

/// Error message container.
pub const ERROR_MESSAGE: &str = ".error-message";

/// Alert role element.
pub const ALERT_ROLE: &str = "[role='alert']";

/// Toast error notification.
pub const TOAST_ERROR: &str = ".toast-error";

// ── Auth Flow ───────────────────────────────────────────────────────────────

/// Google account email input.
pub const GOOGLE_EMAIL_INPUT: &str = "input[type='email']";

/// Google password input.
pub const GOOGLE_PASSWORD_INPUT: &str = "input[type='password']";

/// Google "Next" button (multiple contexts).
pub const GOOGLE_NEXT_BUTTON: &str = "#identifierNext button, #passwordNext button";

// ── Timing Constants ────────────────────────────────────────────────────────

/// Max wait for chat interface to become ready (ms).
pub const READY_TIMEOUT_MS: u64 = 10_000;

/// Fallback timeout for alternative selectors (ms).
pub const FALLBACK_TIMEOUT_MS: u64 = 5_000;

/// Response polling interval (ms).
pub const POLL_INTERVAL_MS: u64 = 1_000;

/// Max wait for response (ms). 120 seconds matches npm server.
pub const RESPONSE_TIMEOUT_MS: u64 = 120_000;

/// Consecutive identical polls needed to confirm response is complete.
pub const STABILITY_POLLS: usize = 3;

/// Post-navigation stabilization delay range (ms).
pub const NAV_SETTLE_MIN_MS: u64 = 2_000;
pub const NAV_SETTLE_MAX_MS: u64 = 3_000;

/// Delay between typing completion and submit (ms).
pub const PRE_SUBMIT_MIN_MS: u64 = 500;
pub const PRE_SUBMIT_MAX_MS: u64 = 1_000;
