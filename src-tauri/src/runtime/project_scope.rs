//! Host-internal opened-project scopes; no IPC/model/extension mint entry point.
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

const MAX_SCOPES: usize = 128;
const MAX_WINDOWS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrustedOpenSource {
    NativePicker,
    RestoredWorkspace,
    TrustedGuiOpen,
}
struct OpenedScope {
    window: String,
    generation: u64,
    project: PathBuf,
    source: TrustedOpenSource,
}
/// Fields cannot be assembled from renderer parameters. Lookup anew per operation;
/// this snapshot is not an execution grant or a live revocation guard.
pub struct VerifiedProjectScope {
    handle: String,
    window: String,
    generation: u64,
    project: PathBuf,
    source: TrustedOpenSource,
}
impl VerifiedProjectScope {
    pub fn handle(&self) -> &str {
        &self.handle
    }
    pub fn window(&self) -> &str {
        &self.window
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn project(&self) -> &Path {
        &self.project
    }
    pub fn source(&self) -> TrustedOpenSource {
        self.source
    }
}
pub struct ProjectScopeRegistry {
    scopes: HashMap<String, OpenedScope>,
    windows: HashMap<String, (u64, bool)>,
    limit: usize,
}
impl Default for ProjectScopeRegistry {
    fn default() -> Self {
        Self {
            scopes: HashMap::new(),
            windows: HashMap::new(),
            limit: MAX_SCOPES,
        }
    }
}
impl ProjectScopeRegistry {
    pub fn with_limit(limit: usize) -> Result<Self, String> {
        if limit == 0 || limit > MAX_SCOPES {
            return Err("invalid project scope budget".into());
        }
        Ok(Self {
            limit,
            ..Self::default()
        })
    }
    /// Host lifecycle only. A strictly newer generation revokes this window's old handles.
    pub fn begin_window(&mut self, window: &str, generation: u64) -> Result<(), String> {
        if window.trim().is_empty()
            || window.len() > 128
            || window.chars().any(char::is_control)
            || generation == 0
        {
            return Err("invalid host window identity".into());
        }
        if self
            .windows
            .get(window)
            .is_some_and(|(old, _)| generation <= *old)
        {
            return Err("stale host window generation".into());
        }
        if !self.windows.contains_key(window) && self.windows.len() >= MAX_WINDOWS {
            return Err("opened window scope budget exceeded".into());
        }
        self.scopes.retain(|_, scope| scope.window != window);
        self.windows.insert(window.into(), (generation, true));
        Ok(())
    }
    fn require_window(&self, window: &str, generation: u64) -> Result<(), String> {
        if self.windows.get(window) != Some(&(generation, true)) {
            return Err("unknown or stale host window scope".into());
        }
        Ok(())
    }
    /// Only trusted GUI/native picker/host restore lifecycle may call this. Source is
    /// provenance metadata, not proof of filesystem ownership or permission.
    pub fn mint(
        &mut self,
        window: &str,
        generation: u64,
        project: &Path,
        source: TrustedOpenSource,
    ) -> Result<VerifiedProjectScope, String> {
        self.require_window(window, generation)?;
        let canonical = std::fs::canonicalize(project)
            .map_err(|_| "project scope root cannot be resolved".to_string())?;
        if !canonical.is_dir() {
            return Err("project scope root is not a directory".into());
        }
        if let Some((handle, scope)) = self.scopes.iter().find(|(_, scope)| {
            scope.window == window && scope.generation == generation && scope.project == canonical
        }) {
            return Ok(verified(handle, scope));
        }
        if self.scopes.len() >= self.limit {
            return Err(
                "opened project scope budget exceeded; close an existing project first".into(),
            );
        }
        let handle = uuid::Uuid::new_v4().to_string();
        let scope = OpenedScope {
            window: window.into(),
            generation,
            project: canonical,
            source,
        };
        let result = verified(&handle, &scope);
        self.scopes.insert(handle, scope);
        Ok(result)
    }
    /// Must precede opening a project/session database. Caller identity comes from host,
    /// not user-supplied window/generation fields. Unknown and revoked handles fail alike.
    pub fn lookup(
        &self,
        handle: &str,
        window: &str,
        generation: u64,
    ) -> Result<VerifiedProjectScope, String> {
        self.require_window(window, generation)?;
        let scope = self
            .scopes
            .get(handle)
            .ok_or("unknown or revoked project scope")?;
        if scope.window != window || scope.generation != generation {
            return Err("project scope does not belong to this host window".into());
        }
        Ok(verified(handle, scope))
    }
    pub fn revoke(&mut self, handle: &str, window: &str, generation: u64) -> Result<(), String> {
        self.lookup(handle, window, generation)?;
        self.scopes.remove(handle);
        Ok(())
    }
    /// Retains generation watermark, so a reused label cannot resurrect old identity.
    pub fn close_window(&mut self, window: &str, generation: u64) -> Result<(), String> {
        self.require_window(window, generation)?;
        self.scopes.retain(|_, scope| scope.window != window);
        self.windows.insert(window.into(), (generation, false));
        Ok(())
    }
}
fn verified(handle: &str, scope: &OpenedScope) -> VerifiedProjectScope {
    VerifiedProjectScope {
        handle: handle.into(),
        window: scope.window.clone(),
        generation: scope.generation,
        project: scope.project.clone(),
        source: scope.source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Directory(PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            assert!(self.0.starts_with(std::env::temp_dir()));
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn fixture() -> Directory {
        let d =
            Directory(std::env::temp_dir().join(format!("agentz-scope-{}", uuid::Uuid::new_v4())));
        std::fs::create_dir_all(d.0.join("first")).unwrap();
        std::fs::create_dir_all(d.0.join("second")).unwrap();
        d
    }
    #[test]
    fn unknown_wrong_window_stale_generation_and_revoked_handles_fail() {
        let d = fixture();
        let mut r = ProjectScopeRegistry::default();
        assert!(r
            .mint("main", 1, &d.0, TrustedOpenSource::NativePicker)
            .is_err());
        r.begin_window("main", 1).unwrap();
        r.begin_window("other", 1).unwrap();
        let scope = r
            .mint("main", 1, &d.0, TrustedOpenSource::NativePicker)
            .unwrap();
        assert!(r.lookup("unknown", "main", 1).is_err());
        assert!(r.lookup(scope.handle(), "other", 1).is_err());
        assert!(r.lookup(scope.handle(), "main", 2).is_err());
        r.revoke(scope.handle(), "main", 1).unwrap();
        assert!(r.lookup(scope.handle(), "main", 1).is_err());
    }
    #[test]
    fn canonical_multiproject_scope_and_window_restart_are_isolated() {
        let d = fixture();
        let mut r = ProjectScopeRegistry::default();
        r.begin_window("main", 1).unwrap();
        let first = r
            .mint(
                "main",
                1,
                &d.0.join("first"),
                TrustedOpenSource::TrustedGuiOpen,
            )
            .unwrap();
        let alias = r
            .mint(
                "main",
                1,
                &d.0.join("first/../first"),
                TrustedOpenSource::RestoredWorkspace,
            )
            .unwrap();
        assert_eq!(first.handle(), alias.handle());
        assert_eq!(
            first.project(),
            std::fs::canonicalize(d.0.join("first")).unwrap()
        );
        let second = r
            .mint(
                "main",
                1,
                &d.0.join("second"),
                TrustedOpenSource::NativePicker,
            )
            .unwrap();
        assert_ne!(first.handle(), second.handle());
        r.revoke(first.handle(), "main", 1).unwrap();
        assert!(r.lookup(second.handle(), "main", 1).is_ok());
        r.begin_window("main", 2).unwrap();
        assert!(r.lookup(second.handle(), "main", 2).is_err());
        assert!(r.begin_window("main", 1).is_err());
    }
    #[test]
    fn budget_errors_do_not_evict_scopes_and_invalid_roots_do_not_mint() {
        let d = fixture();
        let mut r = ProjectScopeRegistry::with_limit(1).unwrap();
        r.begin_window("main", 1).unwrap();
        assert!(r
            .mint(
                "main",
                1,
                &d.0.join("missing"),
                TrustedOpenSource::NativePicker
            )
            .is_err());
        let first = r
            .mint(
                "main",
                1,
                &d.0.join("first"),
                TrustedOpenSource::NativePicker,
            )
            .unwrap();
        assert!(r
            .mint(
                "main",
                1,
                &d.0.join("second"),
                TrustedOpenSource::NativePicker
            )
            .is_err());
        assert!(r.lookup(first.handle(), "main", 1).is_ok());
        r.close_window("main", 1).unwrap();
        assert!(r.lookup(first.handle(), "main", 1).is_err());
        assert!(r
            .mint("main", 1, &d.0, TrustedOpenSource::NativePicker)
            .is_err());
        assert!(ProjectScopeRegistry::with_limit(0).is_err());
    }
}
