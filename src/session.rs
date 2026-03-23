//! Chat session management.
//!
//! Sessions track conversation state with individual notebooks.
//! Grounding: ς(State) + σ(Sequence) — stateful sequential interactions.

use crate::error::NotebookLmError;
use crate::persistence;
use crate::types::Session;
use nexcore_chrono::DateTime;
use nexcore_id::NexId;

/// In-memory session store — persisted to JSON.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct SessionStore {
    pub sessions: Vec<Session>,
}

impl SessionStore {
    /// Load sessions from disk.
    pub fn load() -> Result<Self, NotebookLmError> {
        let path = persistence::sessions_path()?;
        persistence::read_json(&path)
    }

    /// Save sessions to disk (atomic).
    pub fn save(&self) -> Result<(), NotebookLmError> {
        let path = persistence::sessions_path()?;
        persistence::write_json(&path, self)
    }

    /// Create a new session for a notebook.
    pub fn create(&mut self, notebook_id: &str) -> Result<&Session, NotebookLmError> {
        let now = DateTime::now();
        let session = Session {
            id: NexId::v4().to_string(),
            notebook_id: notebook_id.to_string(),
            message_count: 0,
            created_at: now,
            last_activity: now,
        };
        self.sessions.push(session);
        self.save()?;

        // Return reference to the just-pushed session
        self.sessions
            .last()
            .ok_or_else(|| NotebookLmError::Other("session creation failed".to_string()))
    }

    /// Get a session by ID.
    pub fn get(&self, id: &str) -> Result<&Session, NotebookLmError> {
        self.sessions
            .iter()
            .find(|s| s.id == id)
            .ok_or_else(|| NotebookLmError::SessionNotFound(id.to_string()))
    }

    /// Get or create a session for a notebook.
    pub fn get_or_create(&mut self, notebook_id: &str) -> Result<String, NotebookLmError> {
        // Return existing session if one exists for this notebook
        if let Some(session) = self.sessions.iter().find(|s| s.notebook_id == notebook_id) {
            return Ok(session.id.clone());
        }
        // Create new
        let session = self.create(notebook_id)?;
        Ok(session.id.clone())
    }

    /// Record a message exchange in a session.
    pub fn record_message(&mut self, session_id: &str) -> Result<(), NotebookLmError> {
        let session = self
            .sessions
            .iter_mut()
            .find(|s| s.id == session_id)
            .ok_or_else(|| NotebookLmError::SessionNotFound(session_id.to_string()))?;

        session.message_count += 1;
        session.last_activity = DateTime::now();
        self.save()
    }

    /// Close (remove) a session.
    pub fn close(&mut self, id: &str) -> Result<Session, NotebookLmError> {
        let pos = self
            .sessions
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| NotebookLmError::SessionNotFound(id.to_string()))?;

        let removed = self.sessions.remove(pos);
        self.save()?;
        Ok(removed)
    }

    /// Reset a session (clear message count, keep ID).
    pub fn reset(&mut self, id: &str) -> Result<(), NotebookLmError> {
        let session = self
            .sessions
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| NotebookLmError::SessionNotFound(id.to_string()))?;

        session.message_count = 0;
        session.last_activity = DateTime::now();
        self.save()
    }

    /// List all sessions.
    pub fn list(&self) -> &[Session] {
        &self.sessions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_lifecycle() {
        let mut store = SessionStore::default();
        assert!(store.sessions.is_empty());

        // Create
        let sid = store.get_or_create("nb1");
        assert!(sid.is_ok());
        let sid = sid.unwrap_or_default();
        assert!(!sid.is_empty());

        // Get
        let session = store.get(&sid);
        assert!(session.is_ok());
        assert_eq!(session.map(|s| s.message_count).ok(), Some(0));

        // Get-or-create returns same session
        let sid2 = store.get_or_create("nb1").unwrap_or_default();
        assert_eq!(sid, sid2);
        assert_eq!(store.sessions.len(), 1);

        // Record messages
        assert!(store.record_message(&sid).is_ok());
        assert_eq!(store.get(&sid).map(|s| s.message_count).ok(), Some(1));

        // Reset
        assert!(store.reset(&sid).is_ok());
        assert_eq!(store.get(&sid).map(|s| s.message_count).ok(), Some(0));
    }
}
