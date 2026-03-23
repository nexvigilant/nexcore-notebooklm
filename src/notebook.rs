//! Core query engine: navigate to notebook, ask questions, extract responses.
//!
//! Implements the full ask flow:
//! 1. Navigate to notebook URL
//! 2. Wait for chat interface readiness
//! 3. Snapshot existing responses (to filter them out)
//! 4. Type question with human-like timing
//! 5. Submit via Enter key
//! 6. Poll for response with 3-poll stability check
//! 7. Extract and return new response text
//!
//! Grounding: →(Causality) + σ(Sequence) — causal query→response sequence.

use std::sync::Arc;
use std::time::{Duration, Instant};

use chromiumoxide::page::Page;
use tracing::{debug, info, warn};

use crate::browser;
use crate::error::NotebookLmError;
use crate::selectors;
use crate::session::SessionStore;

/// Result of asking a question to NotebookLM.
#[derive(Debug, Clone, serde::Serialize)]
pub struct QueryResult {
    /// The answer text from NotebookLM.
    pub answer: String,
    /// Session ID used for this conversation.
    pub session_id: String,
    /// Notebook ID queried.
    pub notebook_id: String,
    /// Whether a rate limit was detected after this query.
    pub rate_limited: bool,
    /// Time taken for the query (ms).
    pub duration_ms: u64,
}

/// Ask a question to a NotebookLM notebook.
///
/// Full flow: launch browser → navigate → wait ready → type → submit → extract.
pub async fn ask_question(
    notebook_url: &str,
    question: &str,
    session_id: Option<&str>,
    notebook_id: &str,
) -> Result<QueryResult, NotebookLmError> {
    let start = Instant::now();

    // Ensure browser is running
    browser::ensure_running().await?;

    // Navigate to the notebook
    let page = browser::navigate_to(notebook_url).await?;

    // Wait for page to settle after navigation
    let settle_ms = random_delay(selectors::NAV_SETTLE_MIN_MS, selectors::NAV_SETTLE_MAX_MS);
    tokio::time::sleep(Duration::from_millis(settle_ms)).await;

    // Wait for chat interface to be ready
    wait_for_chat_ready(&page).await?;

    // Snapshot existing responses to filter them out later
    let pre_count = count_responses(&page).await;
    debug!("pre-existing response count: {pre_count}");

    // Type the question with human-like delays
    type_question(&page, question).await?;

    // Brief pause before submitting (human behavior)
    let pre_submit = random_delay(selectors::PRE_SUBMIT_MIN_MS, selectors::PRE_SUBMIT_MAX_MS);
    tokio::time::sleep(Duration::from_millis(pre_submit)).await;

    // Submit via Enter key
    submit_question(&page).await?;

    // Poll for response with stability check
    let answer = wait_for_response(&page, pre_count, question).await?;

    // Check for rate limiting
    let rate_limited = check_rate_limit(&page).await;
    if rate_limited {
        warn!("NotebookLM rate limit detected after query");
    }

    // Track session
    let sid = match session_id {
        Some(id) => id.to_string(),
        None => {
            let mut store = SessionStore::load().unwrap_or_default();
            let id = store.get_or_create(notebook_id).unwrap_or_default();
            if let Err(e) = store.record_message(&id) {
                warn!("failed to record session message: {e}");
            }
            if let Err(e) = store.save() {
                warn!("failed to save session store: {e}");
            }
            id
        }
    };

    let duration_ms = start.elapsed().as_millis() as u64;

    info!(
        "NLM query complete in {duration_ms}ms — answer length: {} chars",
        answer.len()
    );

    Ok(QueryResult {
        answer,
        session_id: sid,
        notebook_id: notebook_id.to_string(),
        rate_limited,
        duration_ms,
    })
}

/// Wait for the chat input textarea to become visible.
async fn wait_for_chat_ready(page: &Arc<Page>) -> Result<(), NotebookLmError> {
    let deadline = Instant::now() + Duration::from_millis(selectors::READY_TIMEOUT_MS);

    // Try primary selector first
    while Instant::now() < deadline {
        let visible = element_visible(page, selectors::CHAT_INPUT).await;
        if visible {
            debug!("chat input ready (primary selector)");
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // Fallback selector (localization variants)
    let fallback_deadline = Instant::now() + Duration::from_millis(selectors::FALLBACK_TIMEOUT_MS);
    while Instant::now() < fallback_deadline {
        let visible = element_visible(page, selectors::CHAT_INPUT_FALLBACK).await;
        if visible {
            debug!("chat input ready (fallback selector)");
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    Err(NotebookLmError::Other(
        "chat interface did not become ready within timeout".to_string(),
    ))
}

/// Find the chat input element, trying primary then fallback selector.
async fn find_chat_input(
    page: &Arc<Page>,
) -> Result<chromiumoxide::element::Element, NotebookLmError> {
    // Try primary selector
    if let Ok(el) = page.find_element(selectors::CHAT_INPUT).await {
        return Ok(el);
    }
    // Try fallback
    page.find_element(selectors::CHAT_INPUT_FALLBACK)
        .await
        .map_err(|e| NotebookLmError::Other(format!("chat input not found: {e}")))
}

/// Type a question into the chat input using Element.type_str.
///
/// chromiumoxide's type_str dispatches individual CDP key events,
/// which provides natural typing behavior.
async fn type_question(page: &Arc<Page>, question: &str) -> Result<(), NotebookLmError> {
    let el = find_chat_input(page).await?;

    // Click to focus
    el.click()
        .await
        .map_err(|e| NotebookLmError::Other(format!("click chat input: {e}")))?;

    // type_str sends individual key events through CDP
    el.type_str(question)
        .await
        .map_err(|e| NotebookLmError::Other(format!("type question: {e}")))?;

    debug!("typed {} chars into chat input", question.len());
    Ok(())
}

/// Submit the question by pressing Enter on the chat input element.
async fn submit_question(page: &Arc<Page>) -> Result<(), NotebookLmError> {
    let el = find_chat_input(page).await?;
    el.press_key("Enter")
        .await
        .map_err(|e| NotebookLmError::Other(format!("press Enter: {e}")))?;
    debug!("submitted question via Enter key");
    Ok(())
}

/// Poll for a new response with 3-poll stability check.
///
/// Waits until:
/// 1. A new response appears (count > pre_count)
/// 2. Thinking indicator disappears
/// 3. Response text is stable for STABILITY_POLLS consecutive polls
async fn wait_for_response(
    page: &Arc<Page>,
    pre_count: usize,
    question: &str,
) -> Result<String, NotebookLmError> {
    let deadline = Instant::now() + Duration::from_millis(selectors::RESPONSE_TIMEOUT_MS);
    let poll_interval = Duration::from_millis(selectors::POLL_INTERVAL_MS);

    let mut last_text = String::new();
    let mut stable_count: usize = 0;
    let question_normalized = normalize_text(question);

    while Instant::now() < deadline {
        tokio::time::sleep(poll_interval).await;

        // Check if thinking indicator is visible — if so, keep waiting
        if element_visible(page, selectors::THINKING_INDICATOR).await {
            stable_count = 0;
            continue;
        }

        // Extract latest response text
        let current_text = extract_latest_response(page, pre_count).await;

        // Skip if empty or if it's just an echo of the question
        if current_text.is_empty() || normalize_text(&current_text) == question_normalized {
            stable_count = 0;
            continue;
        }

        // Stability check: text must be identical for N consecutive polls
        if current_text == last_text {
            stable_count += 1;
            if stable_count >= selectors::STABILITY_POLLS {
                debug!("response stable after {stable_count} polls");
                return Ok(current_text);
            }
        } else {
            // Text changed — reset stability counter
            stable_count = 1;
            last_text = current_text;
        }
    }

    // Timeout — return whatever we have
    if !last_text.is_empty() {
        warn!(
            "response timeout — returning partial text ({} chars)",
            last_text.len()
        );
        Ok(last_text)
    } else {
        Err(NotebookLmError::Other(
            "no response received within timeout".to_string(),
        ))
    }
}

/// Count the number of response containers currently on the page.
async fn count_responses(page: &Arc<Page>) -> usize {
    let js = format!(
        "document.querySelectorAll('{}').length",
        selectors::RESPONSE_CONTAINER
    );
    page.evaluate(js)
        .await
        .ok()
        .and_then(|v| v.into_value::<usize>().ok())
        .unwrap_or(0)
}

/// Extract the text of the latest response (after pre_count).
async fn extract_latest_response(page: &Arc<Page>, pre_count: usize) -> String {
    let js = format!(
        r#"
        (() => {{
            const containers = document.querySelectorAll('{container}');
            if (containers.length <= {pre_count}) return '';
            const latest = containers[containers.length - 1];
            const textEl = latest.querySelector('{text_sel}');
            return textEl ? textEl.textContent.trim() : '';
        }})()
        "#,
        container = selectors::RESPONSE_CONTAINER,
        pre_count = pre_count,
        text_sel = selectors::RESPONSE_TEXT,
    );

    page.evaluate(js)
        .await
        .ok()
        .and_then(|v| v.into_value::<String>().ok())
        .unwrap_or_default()
}

/// Check if a rate limit has been hit.
async fn check_rate_limit(page: &Arc<Page>) -> bool {
    let js = format!(
        r#"
        (() => {{
            // Check error-related elements
            const errorEls = document.querySelectorAll('{error}, {alert}, {toast}');
            for (const el of errorEls) {{
                const text = (el.textContent || '').toLowerCase();
                if (text.includes('rate limit') || text.includes('quota exhausted') || text.includes('daily limit')) {{
                    return true;
                }}
            }}
            // Check if textarea is disabled (signals quota depletion)
            const input = document.querySelector('{input}') || document.querySelector('{input_fb}');
            if (input && input.disabled) return true;
            return false;
        }})()
        "#,
        error = selectors::ERROR_MESSAGE,
        alert = selectors::ALERT_ROLE,
        toast = selectors::TOAST_ERROR,
        input = selectors::CHAT_INPUT,
        input_fb = selectors::CHAT_INPUT_FALLBACK,
    );

    page.evaluate(js)
        .await
        .ok()
        .and_then(|v| v.into_value::<bool>().ok())
        .unwrap_or(false)
}

/// Check if an element is visible on the page.
async fn element_visible(page: &Arc<Page>, selector: &str) -> bool {
    let js = format!(
        r#"
        (() => {{
            const el = document.querySelector('{selector}');
            if (!el) return false;
            const style = window.getComputedStyle(el);
            return style.display !== 'none' && style.visibility !== 'hidden' && el.offsetParent !== null;
        }})()
        "#,
        selector = selector,
    );

    page.evaluate(js)
        .await
        .ok()
        .and_then(|v| v.into_value::<bool>().ok())
        .unwrap_or(false)
}

/// Normalize text for comparison (trim, collapse whitespace, lowercase).
fn normalize_text(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Generate a random delay in the given range (inclusive).
/// Uses a simple deterministic approach based on system time nanos.
fn random_delay(min_ms: u64, max_ms: u64) -> u64 {
    if min_ms >= max_ms {
        return min_ms;
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(min_ms);
    min_ms + (nanos % (max_ms - min_ms + 1))
}
