//! Pluggable project-context discovery and injection.
//!
//! This module formalises the *strategy* by which the agent loop discovers
//! project-specific instructions (PISCIS.md, `.piscis/instructions.md`, …) and
//! renders them into the system prompt.
//!
//! # Backward compatibility
//!
//! The free functions in [`crate::project_context`] remain the canonical
//! implementation and are **unchanged**; hosts such as openpiscis call them
//! directly. [`ProjectContextManager`] simply wraps those functions behind the
//! [`ContextManager`] trait so that callers who want to *swap* the discovery
//! strategy (workbench, headless, future remote sources) can do so through a
//! single [`HarnessConfig`](crate::agent::harness::config::HarnessConfig) slot
//! without changing loop code. When no manager is wired the loop behaves
//! exactly as before.

use std::path::Path;

pub use crate::project_context::ProjectInstructionFile;

/// Default character budget used when a caller does not specify one.
pub const DEFAULT_CONTEXT_BUDGET_CHARS: usize = 8_000;

/// Strategy for discovering and rendering project context.
///
/// Implementations must be cheap to share (`Send + Sync`) because the harness
/// stores them behind an `Arc`.
pub trait ContextManager: Send + Sync {
    /// Stable identifier for diagnostics / config round-tripping.
    fn name(&self) -> &str;

    /// Discover instruction files reachable from `root`.
    fn discover(&self, root: &Path) -> std::io::Result<Vec<ProjectInstructionFile>>;

    /// Render discovered context into a prompt-ready string, bounded by
    /// `budget_chars`. Returning an empty string means "no context".
    ///
    /// The default implementation delegates to the canonical renderer in
    /// [`crate::project_context`], guaranteeing identical formatting and
    /// budgeting semantics to the legacy code path.
    fn render(&self, root: &Path, budget_chars: usize) -> std::io::Result<String> {
        crate::project_context::render_project_instruction_context(root, budget_chars)
    }
}

/// Ancestor-chain scanner — the default, behaviour-preserving manager.
///
/// Equivalent to calling [`crate::project_context`] directly.
#[derive(Debug, Default, Clone, Copy)]
pub struct ProjectContextManager;

impl ContextManager for ProjectContextManager {
    fn name(&self) -> &str {
        "ProjectContextManager"
    }

    fn discover(&self, root: &Path) -> std::io::Result<Vec<ProjectInstructionFile>> {
        crate::project_context::discover_project_instruction_files(root)
    }
}

/// Manager that performs no discovery — used by ephemeral loops (fish) or when
/// a host injects context by other means.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoOpContextManager;

impl ContextManager for NoOpContextManager {
    fn name(&self) -> &str {
        "NoOpContextManager"
    }

    fn discover(&self, _root: &Path) -> std::io::Result<Vec<ProjectInstructionFile>> {
        Ok(Vec::new())
    }

    fn render(&self, _root: &Path, _budget_chars: usize) -> std::io::Result<String> {
        Ok(String::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("piscis-ctx-mgr-{nanos}"))
    }

    #[test]
    fn project_manager_matches_free_functions() {
        let root = temp_dir();
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("PISCIS.md"), "rules here").unwrap();

        let mgr = ProjectContextManager;
        assert_eq!(mgr.name(), "ProjectContextManager");

        let files = mgr.discover(&root).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].content, "rules here");

        let rendered = mgr.render(&root, 4_000).unwrap();
        assert!(rendered.contains("## Project Instructions"));
        assert!(rendered.contains("rules here"));

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn noop_manager_discovers_nothing() {
        let mgr = NoOpContextManager;
        assert_eq!(mgr.name(), "NoOpContextManager");
        assert!(mgr.discover(Path::new("/nonexistent")).unwrap().is_empty());
        assert!(mgr
            .render(Path::new("/nonexistent"), 4_000)
            .unwrap()
            .is_empty());
    }
}
