//! Atomic JSON persistence layer.
//!
//! All writes go to `.tmp` then `fs::rename` for crash safety.
//! Grounding: π(Persistence) + ∂(Boundary) — durable storage with atomic boundaries.

use crate::error::NotebookLmError;
use nexcore_fs::dirs;
use std::fs;
use std::path::{Path, PathBuf};

/// Returns the data directory for NotebookLM, creating it if needed.
///
/// Location: `~/.claude/data/notebooklm/`
pub fn data_dir() -> Result<PathBuf, NotebookLmError> {
    let home = dirs::home_dir()
        .ok_or_else(|| NotebookLmError::Other("cannot determine home directory".to_string()))?;
    let dir = home.join(".claude").join("data").join("notebooklm");
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}

/// Read and deserialize JSON from a file. Returns default `T` if file doesn't exist.
pub fn read_json<T: serde::de::DeserializeOwned + Default>(
    path: &Path,
) -> Result<T, NotebookLmError> {
    if !path.exists() {
        return Ok(T::default());
    }
    let content = fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(T::default());
    }
    let value = serde_json::from_str(&content)?;
    Ok(value)
}

/// Serialize and write JSON to a file atomically (write .tmp, then rename).
pub fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), NotebookLmError> {
    let content = serde_json::to_string_pretty(value)?;
    let tmp_path = path.with_extension("tmp");

    // Ensure parent directory exists
    if let Some(parent) = path.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent)?;
    }

    fs::write(&tmp_path, &content)?;
    fs::rename(&tmp_path, path)?;
    Ok(())
}

/// Path to the library JSON file.
pub fn library_path() -> Result<PathBuf, NotebookLmError> {
    Ok(data_dir()?.join("library.json"))
}

/// Path to the sessions JSON file.
pub fn sessions_path() -> Result<PathBuf, NotebookLmError> {
    Ok(data_dir()?.join("sessions.json"))
}

/// Path to the auth state JSON file.
pub fn auth_state_path() -> Result<PathBuf, NotebookLmError> {
    Ok(data_dir()?.join("auth_state.json"))
}

/// Path to the Chrome profile directory.
pub fn chrome_profile_path() -> Result<PathBuf, NotebookLmError> {
    Ok(data_dir()?.join("chrome_profile"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_data_dir_creation() {
        let dir = data_dir();
        assert!(dir.is_ok());
        let dir = dir.unwrap_or_else(|_| PathBuf::from("/tmp"));
        assert!(dir.to_string_lossy().contains("notebooklm"));
    }

    #[test]
    fn test_read_missing_file_returns_default() {
        let result: Result<HashMap<String, String>, _> =
            read_json(Path::new("/tmp/nonexistent_nlm_test.json"));
        assert!(result.is_ok());
        assert!(result.unwrap_or_default().is_empty());
    }

    #[test]
    fn test_atomic_write_and_read() {
        let tmp = std::env::temp_dir().join("nlm_test_atomic.json");
        let data: HashMap<String, String> = [("key".to_string(), "value".to_string())]
            .into_iter()
            .collect();

        let write_result = write_json(&tmp, &data);
        assert!(write_result.is_ok());

        let read_back: HashMap<String, String> = read_json(&tmp).unwrap_or_default();
        assert_eq!(read_back.get("key").map(String::as_str), Some("value"));

        // Best-effort test cleanup — non-critical if it fails
        if let Err(e) = fs::remove_file(&tmp) {
            tracing::debug!("test cleanup failed: {e}");
        }
    }
}
