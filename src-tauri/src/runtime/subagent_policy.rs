//! Capability resolution for delegated sub-agents.
//!
//! Effective capability = agent manifest ceiling ∩ call-time narrowing ∩ parent
//! session policy. No layer may widen another: the result is never broader than
//! any single input.

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};

/// Tools that let a sub-agent delegate further or reach the team pool. Always
/// stripped from temporary sub-agents unless the manifest allowlist names them
/// explicitly (delegation tools are never re-enabled, to keep depth at 1).
pub const DELEGATION_TOOLS: &[&str] = &["delegate", "call_fish", "task"];
pub const TEAM_POOL_TOOLS: &[&str] = &["pool_chat", "pool_org"];

/// Tools that talk to the user or mutate the parent's plan/memory state. A
/// sub-agent has no user to ask and must not rewrite the parent's plan, so these
/// are never available, even when named in an allowlist.
pub const SUBAGENT_NEVER_TOOLS: &[&str] = &[
    "chat_ui",
    "chat_ui_listen",
    "chat_ui_patch",
    "plan_mode_ui",
    "plan_write",
    "plan_todo",
    "memory_store",
];

/// Powerful side-effect tools that a writable sub-agent only gets when its
/// manifest allowlist names them explicitly.
pub const SENSITIVE_UNLESS_NAMED: &[&str] =
    &["elevate", "email", "ssh", "app_control", "skill_manage", "browser"];

/// Tools that can mutate state; removed whenever the effective mode is read-only.
pub const MUTATING_TOOLS: &[&str] = &[
    "file_write",
    "file_edit",
    "shell",
    "code_run",
    "process_control",
    "elevate",
    "email",
    "ssh",
    "memory_store",
    "plan_todo",
    "browser",
    "app_control",
    "skill_manage",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PermissionMode {
    /// Read and search only.
    #[default]
    ReadOnly,
    /// Mutating tools allowed but still routed through approval.
    Ask,
    /// Follow the parent session's policy.
    Inherit,
}

impl PermissionMode {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "ask" => Self::Ask,
            "inherit" => Self::Inherit,
            _ => Self::ReadOnly,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ParentPolicy {
    /// Parent is in plan mode or a read-only policy.
    pub read_only: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CallNarrowing {
    /// Caller forces read-only regardless of manifest.
    pub force_read_only: bool,
}

#[derive(Debug, Clone)]
pub struct ManifestCaps {
    /// Real allowlist; empty means "no restriction from the manifest".
    pub tool_allowlist: Vec<String>,
    pub mode: PermissionMode,
}

impl Default for ManifestCaps {
    fn default() -> Self {
        Self {
            tool_allowlist: Vec::new(),
            mode: PermissionMode::ReadOnly,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveCaps {
    pub read_only: bool,
    /// Tools that must be removed from the sub-agent registry.
    pub denied: HashSet<String>,
    /// If set, only these tools may remain.
    pub allowed: Option<HashSet<String>>,
}

impl EffectiveCaps {
    pub fn permits(&self, tool: &str) -> bool {
        if self.denied.contains(tool) {
            return false;
        }
        self.allowed.as_ref().map_or(true, |a| a.contains(tool))
    }
}

pub fn resolve(manifest: &ManifestCaps, narrow: &CallNarrowing, parent: &ParentPolicy) -> EffectiveCaps {
    let read_only = match manifest.mode {
        PermissionMode::ReadOnly => true,
        PermissionMode::Ask => false,
        PermissionMode::Inherit => parent.read_only,
    } || narrow.force_read_only
        || parent.read_only;

    let mut denied: HashSet<String> = DELEGATION_TOOLS
        .iter()
        .chain(TEAM_POOL_TOOLS.iter())
        .chain(SUBAGENT_NEVER_TOOLS.iter())
        .map(|s| s.to_string())
        .collect();
    denied.extend(SENSITIVE_UNLESS_NAMED.iter().map(|s| s.to_string()));
    if read_only {
        denied.extend(MUTATING_TOOLS.iter().map(|s| s.to_string()));
    }
    // Team-pool and sensitive tools may be re-enabled only by explicit manifest
    // naming (and sensitive ones never in read-only mode); delegation and
    // user-interaction tools never are.
    for t in &manifest.tool_allowlist {
        let name = t.as_str();
        if TEAM_POOL_TOOLS.contains(&name)
            || (!read_only && SENSITIVE_UNLESS_NAMED.contains(&name))
        {
            denied.remove(t);
        }
    }

    let allowed = if manifest.tool_allowlist.is_empty() {
        None
    } else {
        Some(manifest.tool_allowlist.iter().cloned().collect())
    };
    EffectiveCaps {
        read_only,
        denied,
        allowed,
    }
}

// ── Delegation metrics (process-wide, read by diagnostics) ──────────────────

static CALLS: AtomicU64 = AtomicU64::new(0);
static FAILURES: AtomicU64 = AtomicU64::new(0);
static SUB_INPUT_TOKENS: AtomicU64 = AtomicU64::new(0);
static SUB_OUTPUT_TOKENS: AtomicU64 = AtomicU64::new(0);
static TOOL_CALLS: AtomicU64 = AtomicU64::new(0);
static FLASH_FALLBACKS: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct DelegationMetrics {
    pub calls: u64,
    pub failures: u64,
    pub sub_input_tokens: u64,
    pub sub_output_tokens: u64,
    pub sub_tool_calls: u64,
    /// Runs that fell back to the main model because no valid flash/agent model was set.
    pub model_fallbacks: u64,
}

pub fn record_run(
    ok: bool,
    input_tokens: u32,
    output_tokens: u32,
    tool_calls: u32,
    model_fallback: bool,
) {
    CALLS.fetch_add(1, Ordering::Relaxed);
    if !ok {
        FAILURES.fetch_add(1, Ordering::Relaxed);
    }
    SUB_INPUT_TOKENS.fetch_add(input_tokens as u64, Ordering::Relaxed);
    SUB_OUTPUT_TOKENS.fetch_add(output_tokens as u64, Ordering::Relaxed);
    TOOL_CALLS.fetch_add(tool_calls as u64, Ordering::Relaxed);
    if model_fallback {
        FLASH_FALLBACKS.fetch_add(1, Ordering::Relaxed);
    }
}

pub fn metrics_snapshot() -> DelegationMetrics {
    DelegationMetrics {
        calls: CALLS.load(Ordering::Relaxed),
        failures: FAILURES.load(Ordering::Relaxed),
        sub_input_tokens: SUB_INPUT_TOKENS.load(Ordering::Relaxed),
        sub_output_tokens: SUB_OUTPUT_TOKENS.load(Ordering::Relaxed),
        sub_tool_calls: TOOL_CALLS.load(Ordering::Relaxed),
        model_fallbacks: FLASH_FALLBACKS.load(Ordering::Relaxed),
    }
}

/// Cap a sub-agent report so it cannot flood the parent context.
pub fn cap_report(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let head: String = text.chars().take(max_chars).collect();
    format!("{head}\n\n[report truncated at {max_chars} chars; ask a narrower follow-up if you need more]")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps(mode: PermissionMode, allow: &[&str]) -> ManifestCaps {
        ManifestCaps {
            tool_allowlist: allow.iter().map(|s| s.to_string()).collect(),
            mode,
        }
    }

    #[test]
    fn default_is_read_only_without_delegation_or_pool() {
        let e = resolve(&ManifestCaps::default(), &CallNarrowing::default(), &ParentPolicy::default());
        assert!(e.read_only);
        for t in ["file_write", "shell", "delegate", "call_fish", "task", "pool_chat"] {
            assert!(!e.permits(t), "{t} must be denied");
        }
        assert!(e.permits("file_read"));
    }

    #[test]
    fn parent_read_only_clamps_writable_manifest() {
        let e = resolve(
            &caps(PermissionMode::Ask, &[]),
            &CallNarrowing::default(),
            &ParentPolicy { read_only: true },
        );
        assert!(e.read_only);
        assert!(!e.permits("file_write"));
    }

    #[test]
    fn writable_manifest_with_permissive_parent_allows_writes() {
        let e = resolve(&caps(PermissionMode::Ask, &[]), &CallNarrowing::default(), &ParentPolicy::default());
        assert!(!e.read_only);
        assert!(e.permits("file_write"));
        assert!(!e.permits("delegate"));
    }

    #[test]
    fn call_narrowing_can_only_restrict() {
        let e = resolve(
            &caps(PermissionMode::Ask, &[]),
            &CallNarrowing { force_read_only: true },
            &ParentPolicy::default(),
        );
        assert!(e.read_only);
        assert!(!e.permits("shell"));
    }

    #[test]
    fn allowlist_restricts_and_cannot_reenable_delegation() {
        let e = resolve(
            &caps(PermissionMode::ReadOnly, &["file_read", "delegate", "file_write"]),
            &CallNarrowing::default(),
            &ParentPolicy::default(),
        );
        assert!(e.permits("file_read"));
        assert!(!e.permits("file_search"), "not in allowlist");
        assert!(!e.permits("delegate"), "delegation never re-enabled");
        assert!(!e.permits("file_write"), "read-only beats allowlist");
    }

    #[test]
    fn manifest_may_explicitly_enable_pool_tools() {
        let e = resolve(
            &caps(PermissionMode::ReadOnly, &["pool_chat"]),
            &CallNarrowing::default(),
            &ParentPolicy::default(),
        );
        assert!(e.permits("pool_chat"));
    }

    #[test]
    fn writable_agent_keeps_dev_tools_but_not_sensitive_ones() {
        let e = resolve(&caps(PermissionMode::Ask, &[]), &CallNarrowing::default(), &ParentPolicy::default());
        for t in ["file_write", "file_edit", "shell", "code_run", "file_read"] {
            assert!(e.permits(t), "{t} should be available to a writable agent");
        }
        for t in ["elevate", "ssh", "email", "app_control", "chat_ui", "plan_todo", "memory_store", "task"] {
            assert!(!e.permits(t), "{t} must stay denied");
        }
    }

    #[test]
    fn sensitive_tools_need_explicit_naming_and_never_in_read_only() {
        let named = resolve(
            &caps(PermissionMode::Ask, &["file_write", "ssh"]),
            &CallNarrowing::default(),
            &ParentPolicy::default(),
        );
        assert!(named.permits("ssh"));
        let ro = resolve(
            &caps(PermissionMode::ReadOnly, &["ssh", "chat_ui"]),
            &CallNarrowing::default(),
            &ParentPolicy::default(),
        );
        assert!(!ro.permits("ssh"));
        assert!(!ro.permits("chat_ui"), "user-interaction tools are never re-enabled");
    }

    #[test]
    fn inherit_follows_parent() {
        let open = resolve(&caps(PermissionMode::Inherit, &[]), &CallNarrowing::default(), &ParentPolicy::default());
        assert!(!open.read_only);
        let closed = resolve(
            &caps(PermissionMode::Inherit, &[]),
            &CallNarrowing::default(),
            &ParentPolicy { read_only: true },
        );
        assert!(closed.read_only);
    }

    #[test]
    fn parse_defaults_to_read_only() {
        assert_eq!(PermissionMode::parse(""), PermissionMode::ReadOnly);
        assert_eq!(PermissionMode::parse("ASK"), PermissionMode::Ask);
        assert_eq!(PermissionMode::parse("inherit"), PermissionMode::Inherit);
        assert_eq!(PermissionMode::parse("bogus"), PermissionMode::ReadOnly);
    }

    #[test]
    fn cap_report_truncates() {
        assert_eq!(cap_report("abc", 10), "abc");
        let out = cap_report(&"x".repeat(50), 10);
        assert!(out.starts_with("xxxxxxxxxx\n"));
        assert!(out.contains("truncated"));
    }
}
