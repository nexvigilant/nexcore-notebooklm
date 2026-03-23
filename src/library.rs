//! Notebook library CRUD operations.
//!
//! JSON-backed library at `~/.claude/data/notebooklm/library.json`.
//! Grounding: π(Persistence) + μ(Mapping) — persistent key-value notebook store.

use crate::error::NotebookLmError;
use crate::persistence;
use crate::types::{LibraryStats, Notebook};
use nexcore_chrono::DateTime;
use std::collections::HashSet;

/// In-memory library state — Vec of notebooks serialized to JSON.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Library {
    /// All notebooks.
    pub notebooks: Vec<Notebook>,
    /// Currently active notebook ID.
    #[serde(default)]
    pub active_id: Option<String>,
}

impl Library {
    /// Load library from disk.
    pub fn load() -> Result<Self, NotebookLmError> {
        let path = persistence::library_path()?;
        persistence::read_json(&path)
    }

    /// Save library to disk (atomic).
    pub fn save(&self) -> Result<(), NotebookLmError> {
        let path = persistence::library_path()?;
        persistence::write_json(&path, self)
    }

    /// Add a notebook to the library.
    pub fn add(&mut self, notebook: Notebook) -> Result<(), NotebookLmError> {
        // Check for duplicate ID
        if self.notebooks.iter().any(|n| n.id == notebook.id) {
            return Err(NotebookLmError::Other(format!(
                "notebook with id '{}' already exists",
                notebook.id
            )));
        }
        self.notebooks.push(notebook);
        self.save()
    }

    /// Get a notebook by ID.
    pub fn get(&self, id: &str) -> Result<&Notebook, NotebookLmError> {
        self.notebooks
            .iter()
            .find(|n| n.id == id)
            .ok_or_else(|| NotebookLmError::NotebookNotFound(id.to_string()))
    }

    /// Get the active notebook.
    pub fn active(&self) -> Result<&Notebook, NotebookLmError> {
        let id = self
            .active_id
            .as_deref()
            .ok_or(NotebookLmError::NoActiveNotebook)?;
        self.get(id)
    }

    /// Set the active notebook by ID.
    pub fn select(&mut self, id: &str) -> Result<(), NotebookLmError> {
        // Verify it exists
        if !self.notebooks.iter().any(|n| n.id == id) {
            return Err(NotebookLmError::NotebookNotFound(id.to_string()));
        }
        self.active_id = Some(id.to_string());
        self.save()
    }

    /// Update a notebook's metadata. Returns the updated notebook.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        id: &str,
        name: Option<String>,
        description: Option<String>,
        url: Option<String>,
        topics: Option<Vec<String>>,
        content_types: Option<Vec<String>>,
        use_cases: Option<Vec<String>>,
        tags: Option<Vec<String>>,
    ) -> Result<&Notebook, NotebookLmError> {
        let notebook = self
            .notebooks
            .iter_mut()
            .find(|n| n.id == id)
            .ok_or_else(|| NotebookLmError::NotebookNotFound(id.to_string()))?;

        if let Some(v) = name {
            notebook.name = v;
        }
        if let Some(v) = description {
            notebook.description = v;
        }
        if let Some(v) = url {
            notebook.url = v;
        }
        if let Some(v) = topics {
            notebook.topics = v;
        }
        if let Some(v) = content_types {
            notebook.content_types = v;
        }
        if let Some(v) = use_cases {
            notebook.use_cases = v;
        }
        if let Some(v) = tags {
            notebook.tags = v;
        }
        notebook.updated_at = DateTime::now();

        self.save()?;

        // Re-borrow after save
        self.notebooks
            .iter()
            .find(|n| n.id == id)
            .ok_or_else(|| NotebookLmError::NotebookNotFound(id.to_string()))
    }

    /// Remove a notebook from the library.
    pub fn remove(&mut self, id: &str) -> Result<Notebook, NotebookLmError> {
        let pos = self
            .notebooks
            .iter()
            .position(|n| n.id == id)
            .ok_or_else(|| NotebookLmError::NotebookNotFound(id.to_string()))?;

        let removed = self.notebooks.remove(pos);

        // Clear active if it was the removed one
        if self.active_id.as_deref() == Some(id) {
            self.active_id = None;
        }

        self.save()?;
        Ok(removed)
    }

    /// Search notebooks by keyword across name, description, topics, tags.
    pub fn search(&self, query: &str) -> Vec<&Notebook> {
        let q = query.to_lowercase();
        self.notebooks
            .iter()
            .filter(|n| {
                n.name.to_lowercase().contains(&q)
                    || n.description.to_lowercase().contains(&q)
                    || n.topics.iter().any(|t| t.to_lowercase().contains(&q))
                    || n.tags.iter().any(|t| t.to_lowercase().contains(&q))
                    || n.use_cases.iter().any(|u| u.to_lowercase().contains(&q))
            })
            .collect()
    }

    /// Compute library statistics.
    pub fn stats(&self) -> LibraryStats {
        let mut all_topics = HashSet::new();
        let mut all_tags = HashSet::new();

        for nb in &self.notebooks {
            for t in &nb.topics {
                all_topics.insert(t.clone());
            }
            for t in &nb.tags {
                all_tags.insert(t.clone());
            }
        }

        let most_recent = self
            .notebooks
            .iter()
            .max_by_key(|n| n.created_at)
            .map(|n| n.name.clone());

        LibraryStats {
            total_notebooks: self.notebooks.len(),
            total_topics: all_topics.len(),
            total_tags: all_tags.len(),
            most_recent,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_notebook(id: &str, name: &str) -> Notebook {
        let now = DateTime::now();
        Notebook {
            id: id.to_string(),
            name: name.to_string(),
            url: format!("https://notebooklm.google.com/notebook/{id}"),
            description: format!("Test notebook {name}"),
            topics: vec!["testing".to_string()],
            content_types: vec![],
            use_cases: vec![],
            tags: vec!["test".to_string()],
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn test_library_crud() {
        let mut lib = Library::default();
        assert!(lib.notebooks.is_empty());

        let nb = make_notebook("nb1", "First");
        lib.notebooks.push(nb);
        assert_eq!(lib.notebooks.len(), 1);

        let found = lib.get("nb1");
        assert!(found.is_ok());
        assert_eq!(found.map(|n| n.name.as_str()).ok(), Some("First"));

        let missing = lib.get("nb99");
        assert!(missing.is_err());
    }

    #[test]
    fn test_library_search() {
        let mut lib = Library::default();
        lib.notebooks.push(make_notebook("nb1", "Rust Guide"));
        lib.notebooks.push(make_notebook("nb2", "Python Basics"));

        let results = lib.search("rust");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "nb1");

        let results = lib.search("test");
        assert_eq!(results.len(), 2); // both have "test" tag
    }

    #[test]
    fn test_library_select() {
        let mut lib = Library::default();
        lib.notebooks.push(make_notebook("nb1", "First"));

        assert!(lib.active().is_err());

        lib.active_id = Some("nb1".to_string());
        assert!(lib.active().is_ok());
        assert_eq!(lib.active().map(|n| n.name.as_str()).ok(), Some("First"));
    }

    #[test]
    fn test_library_stats() {
        let mut lib = Library::default();
        lib.notebooks.push(make_notebook("nb1", "First"));
        lib.notebooks.push(make_notebook("nb2", "Second"));

        let stats = lib.stats();
        assert_eq!(stats.total_notebooks, 2);
        assert_eq!(stats.total_topics, 1); // both have "testing"
        assert_eq!(stats.total_tags, 1); // both have "test"
    }
}
