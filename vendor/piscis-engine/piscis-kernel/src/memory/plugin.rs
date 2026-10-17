//! Pluggable long-term memory backend.
//!
//! Abstracts how agent memories are stored and retrieved so a host can choose
//! between ephemeral (`NoOpMemory`), in-process (`InMemoryVectorStore`) or, in
//! the future, persistent (SQLite / external vector DB) backends through a
//! single [`HarnessConfig`](crate::agent::harness::config::HarnessConfig) slot.
//!
//! # Backward compatibility
//!
//! This is a new, additive module. The existing [`crate::memory::vector`]
//! helpers are unchanged and reused here for similarity scoring, so no current
//! caller is affected.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::memory::vector::cosine_similarity;

/// A single stored memory.
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryEntry {
    /// Stable identifier (caller-provided; used for upsert / delete).
    pub id: String,
    /// Human-readable content.
    pub content: String,
    /// Optional embedding for vector search.
    pub embedding: Option<Vec<f32>>,
    /// Arbitrary key/value metadata for filtering.
    pub metadata: HashMap<String, String>,
}

impl MemoryEntry {
    /// Convenience constructor for a text-only entry.
    pub fn text(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            content: content.into(),
            embedding: None,
            metadata: HashMap::new(),
        }
    }
}

/// Query parameters for [`MemoryPlugin::search`].
#[derive(Debug, Clone, Default)]
pub struct MemoryQuery {
    /// Free-text query (used by keyword fallback search).
    pub text: Option<String>,
    /// Embedding for vector search (preferred when present).
    pub embedding: Option<Vec<f32>>,
    /// Maximum number of results to return (0 = unbounded).
    pub limit: usize,
    /// Metadata equality filters; all must match.
    pub metadata_filter: HashMap<String, String>,
}

/// A scored search hit.
#[derive(Debug, Clone)]
pub struct MemorySearchResult {
    pub entry: MemoryEntry,
    /// Similarity / relevance score in `[0, 1]` (higher is better).
    pub score: f32,
}

/// Errors surfaced by memory backends.
#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error("embedding dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },
    #[error("memory backend error: {0}")]
    Backend(String),
}

/// Storage + retrieval interface for agent memory.
///
/// Implementations must be `Send + Sync`; the harness shares them behind an
/// `Arc`. Methods take `&self` and use interior mutability so a single shared
/// instance can serve concurrent turns.
pub trait MemoryPlugin: Send + Sync {
    /// Stable identifier for diagnostics / config round-tripping.
    fn name(&self) -> &str;

    /// Insert or replace an entry (keyed by `entry.id`).
    fn store(&self, entry: MemoryEntry) -> Result<(), MemoryError>;

    /// Fetch a single entry by id.
    fn get(&self, id: &str) -> Result<Option<MemoryEntry>, MemoryError>;

    /// Remove an entry by id. Returns whether it existed.
    fn delete(&self, id: &str) -> Result<bool, MemoryError>;

    /// Search according to `query`.
    fn search(&self, query: &MemoryQuery) -> Result<Vec<MemorySearchResult>, MemoryError>;

    /// Whether vector search is meaningful for this backend.
    fn supports_vector_search(&self) -> bool {
        false
    }
}

/// Ephemeral memory that discards everything — for fish / short tasks.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoOpMemory;

impl MemoryPlugin for NoOpMemory {
    fn name(&self) -> &str {
        "NoOpMemory"
    }
    fn store(&self, _entry: MemoryEntry) -> Result<(), MemoryError> {
        Ok(())
    }
    fn get(&self, _id: &str) -> Result<Option<MemoryEntry>, MemoryError> {
        Ok(None)
    }
    fn delete(&self, _id: &str) -> Result<bool, MemoryError> {
        Ok(false)
    }
    fn search(&self, _query: &MemoryQuery) -> Result<Vec<MemorySearchResult>, MemoryError> {
        Ok(Vec::new())
    }
}

/// In-process store with cosine-similarity vector search and a keyword
/// fallback. Suitable for tests and session-scoped memory.
#[derive(Default)]
pub struct InMemoryVectorStore {
    entries: Mutex<Vec<MemoryEntry>>,
}

impl InMemoryVectorStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn matches_filter(entry: &MemoryEntry, filter: &HashMap<String, String>) -> bool {
        filter
            .iter()
            .all(|(k, v)| entry.metadata.get(k).map(|x| x == v).unwrap_or(false))
    }
}

impl MemoryPlugin for InMemoryVectorStore {
    fn name(&self) -> &str {
        "InMemoryVectorStore"
    }

    fn store(&self, entry: MemoryEntry) -> Result<(), MemoryError> {
        let mut entries = self.entries.lock().unwrap();
        if let Some(slot) = entries.iter_mut().find(|e| e.id == entry.id) {
            *slot = entry;
        } else {
            entries.push(entry);
        }
        Ok(())
    }

    fn get(&self, id: &str) -> Result<Option<MemoryEntry>, MemoryError> {
        Ok(self
            .entries
            .lock()
            .unwrap()
            .iter()
            .find(|e| e.id == id)
            .cloned())
    }

    fn delete(&self, id: &str) -> Result<bool, MemoryError> {
        let mut entries = self.entries.lock().unwrap();
        let before = entries.len();
        entries.retain(|e| e.id != id);
        Ok(entries.len() != before)
    }

    fn search(&self, query: &MemoryQuery) -> Result<Vec<MemorySearchResult>, MemoryError> {
        let entries = self.entries.lock().unwrap();
        let mut scored: Vec<MemorySearchResult> = Vec::new();

        for entry in entries.iter() {
            if !Self::matches_filter(entry, &query.metadata_filter) {
                continue;
            }

            let score = match (&query.embedding, &entry.embedding) {
                (Some(q), Some(e)) => {
                    if q.len() != e.len() {
                        return Err(MemoryError::DimensionMismatch {
                            expected: q.len(),
                            got: e.len(),
                        });
                    }
                    // Map cosine [-1, 1] into [0, 1].
                    (cosine_similarity(q, e) + 1.0) / 2.0
                }
                _ => match &query.text {
                    // Keyword fallback: fraction of query terms present.
                    Some(text) if !text.trim().is_empty() => keyword_score(text, &entry.content),
                    _ => 0.0,
                },
            };

            if score > 0.0 {
                scored.push(MemorySearchResult {
                    entry: entry.clone(),
                    score,
                });
            }
        }

        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        if query.limit > 0 {
            scored.truncate(query.limit);
        }
        Ok(scored)
    }

    fn supports_vector_search(&self) -> bool {
        true
    }
}

fn keyword_score(query: &str, content: &str) -> f32 {
    let content_lc = content.to_lowercase();
    let terms: Vec<&str> = query.split_whitespace().collect();
    if terms.is_empty() {
        return 0.0;
    }
    let hits = terms
        .iter()
        .filter(|t| content_lc.contains(&t.to_lowercase()))
        .count();
    hits as f32 / terms.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_discards_everything() {
        let m = NoOpMemory;
        m.store(MemoryEntry::text("a", "hello")).unwrap();
        assert!(m.get("a").unwrap().is_none());
        assert!(m.search(&MemoryQuery::default()).unwrap().is_empty());
    }

    #[test]
    fn in_memory_crud() {
        let m = InMemoryVectorStore::new();
        m.store(MemoryEntry::text("a", "first")).unwrap();
        m.store(MemoryEntry::text("a", "updated")).unwrap(); // upsert
        assert_eq!(m.get("a").unwrap().unwrap().content, "updated");
        assert!(m.delete("a").unwrap());
        assert!(!m.delete("a").unwrap());
    }

    #[test]
    fn vector_search_ranks_by_cosine() {
        let m = InMemoryVectorStore::new();
        let mut near = MemoryEntry::text("near", "near");
        near.embedding = Some(vec![1.0, 0.0]);
        let mut far = MemoryEntry::text("far", "far");
        far.embedding = Some(vec![0.0, 1.0]);
        m.store(near).unwrap();
        m.store(far).unwrap();

        let q = MemoryQuery {
            embedding: Some(vec![1.0, 0.0]),
            limit: 2,
            ..Default::default()
        };
        let results = m.search(&q).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].entry.id, "near");
        assert!(results[0].score > results[1].score);
    }

    #[test]
    fn dimension_mismatch_errors() {
        let m = InMemoryVectorStore::new();
        let mut e = MemoryEntry::text("a", "x");
        e.embedding = Some(vec![1.0, 0.0, 0.0]);
        m.store(e).unwrap();
        let q = MemoryQuery {
            embedding: Some(vec![1.0, 0.0]),
            ..Default::default()
        };
        assert!(matches!(
            m.search(&q),
            Err(MemoryError::DimensionMismatch { .. })
        ));
    }

    #[test]
    fn keyword_fallback_and_metadata_filter() {
        let m = InMemoryVectorStore::new();
        let mut tagged = MemoryEntry::text("a", "the quick brown fox");
        tagged.metadata.insert("kind".into(), "note".into());
        m.store(tagged).unwrap();
        m.store(MemoryEntry::text("b", "lazy dog")).unwrap();

        let q = MemoryQuery {
            text: Some("quick fox".into()),
            metadata_filter: HashMap::from([("kind".to_string(), "note".to_string())]),
            limit: 5,
            ..Default::default()
        };
        let results = m.search(&q).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entry.id, "a");
    }
}
