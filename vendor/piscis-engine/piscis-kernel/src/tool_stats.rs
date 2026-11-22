//! Per-tool call-outcome statistics.
//!
//! Purpose: measure whether a tool's minimal description is good enough. A
//! high `correctable` share means callers get the call shape wrong on the
//! first try; `recovered` says how often the next call of the same tool in the
//! same session then succeeded. `def_chars` is the size of what the tool costs
//! in every request (minimal description + minimal schema).
//!
//! In-memory, optionally persisted to a JSON file (`set_persist_path`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Ok,
    /// Call-shape error the caller can fix (`usage_err` / `correctable_err` /
    /// schema correction).
    Correctable,
    /// Refused by the loop detector or read guard.
    Blocked,
    /// Any other failure (environment, permission, runtime).
    Environment,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolStat {
    pub calls: u64,
    pub ok: u64,
    pub correctable: u64,
    pub blocked: u64,
    pub environment: u64,
    /// A correctable failure that the same session's next call of the same tool
    /// turned into a success.
    pub recovered: u64,
    /// Characters of the minimal description plus minimal schema JSON.
    pub def_chars: u64,
}

#[derive(Default)]
struct State {
    stats: HashMap<String, ToolStat>,
    /// (session, tool) pairs whose latest call was a correctable failure.
    pending: std::collections::HashSet<(String, String)>,
    path: Option<PathBuf>,
    since_save: u32,
}

fn state() -> &'static Mutex<State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(State::default()))
}

/// Classify a finished tool call from its result text.
pub fn classify(content: &str, is_error: bool) -> Outcome {
    if !is_error {
        Outcome::Ok
    } else if content.contains("[\u{5FAA}\u{73AF}\u{68C0}\u{6D4B}]") {
        Outcome::Blocked
    } else if crate::agent::message_utils::is_correctable_failure(content) {
        Outcome::Correctable
    } else {
        Outcome::Environment
    }
}

fn apply(st: &mut State, session: &str, tool: &str, outcome: Outcome) {
    let key = (session.to_string(), tool.to_string());
    let entry = st.stats.entry(tool.to_string()).or_default();
    entry.calls += 1;
    match outcome {
        Outcome::Ok => {
            entry.ok += 1;
            if st.pending.remove(&key) {
                entry.recovered += 1;
            }
        }
        Outcome::Correctable => {
            entry.correctable += 1;
            st.pending.insert(key);
        }
        Outcome::Blocked => entry.blocked += 1,
        Outcome::Environment => {
            entry.environment += 1;
            st.pending.remove(&key);
        }
    }
}

/// Record one finished call.
pub fn record(session: &str, tool: &str, content: &str, is_error: bool) {
    let outcome = classify(content, is_error);
    let Ok(mut st) = state().lock() else { return };
    apply(&mut st, session, tool, outcome);
    st.since_save += 1;
    // Failures are the interesting events; successes are batched.
    if outcome != Outcome::Ok || st.since_save >= 25 {
        save_locked(&mut st);
    }
}

/// Remember how many characters each tool's minimal definition costs.
pub fn note_def_sizes(defs: &[crate::llm::ToolDef]) {
    let Ok(mut st) = state().lock() else { return };
    for def in defs {
        if st.stats.get(&def.name).is_some_and(|s| s.def_chars != 0) {
            continue;
        }
        let chars = def.description.chars().count() + def.input_schema.to_string().chars().count();
        st.stats.entry(def.name.clone()).or_default().def_chars = chars as u64;
    }
}

/// Point the statistics at a JSON file: existing counts are loaded and merged
/// on top of what is already recorded, later saves overwrite it.
pub fn set_persist_path(path: PathBuf) {
    let Ok(mut st) = state().lock() else { return };
    if let Ok(text) = std::fs::read_to_string(&path) {
        if let Ok(saved) = serde_json::from_str::<HashMap<String, ToolStat>>(&text) {
            for (name, old) in saved {
                let cur = st.stats.entry(name).or_default();
                cur.calls += old.calls;
                cur.ok += old.ok;
                cur.correctable += old.correctable;
                cur.blocked += old.blocked;
                cur.environment += old.environment;
                cur.recovered += old.recovered;
                if cur.def_chars == 0 {
                    cur.def_chars = old.def_chars;
                }
            }
        }
    }
    st.path = Some(path);
}

fn save_locked(st: &mut State) {
    st.since_save = 0;
    let Some(path) = st.path.clone() else { return };
    if let Ok(text) = serde_json::to_string_pretty(&st.stats) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, text);
    }
}

/// Current statistics, most-called first.
pub fn snapshot() -> Vec<(String, ToolStat)> {
    let Ok(st) = state().lock() else { return Vec::new() };
    let mut rows: Vec<(String, ToolStat)> =
        st.stats.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    rows.sort_by(|a, b| b.1.calls.cmp(&a.1.calls).then_with(|| a.0.cmp(&b.0)));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_outcomes() {
        assert_eq!(classify("fine", false), Outcome::Ok);
        assert_eq!(classify("[循环检测] stop", true), Outcome::Blocked);
        assert_eq!(classify("bad\n[correctable]", true), Outcome::Correctable);
        assert_eq!(classify("bad\n[usage_help]\nx\n[/usage_help]", true), Outcome::Correctable);
        assert_eq!(classify("File not found", true), Outcome::Environment);
    }

    #[test]
    fn counts_recovery_only_for_the_same_session_and_tool() {
        let mut st = State::default();
        apply(&mut st, "s1", "t", Outcome::Correctable);
        apply(&mut st, "s2", "t", Outcome::Ok);
        assert_eq!(st.stats["t"].recovered, 0);
        apply(&mut st, "s1", "t", Outcome::Ok);
        assert_eq!(st.stats["t"].recovered, 1);
        apply(&mut st, "s1", "t", Outcome::Ok);
        assert_eq!(st.stats["t"].recovered, 1);
        apply(&mut st, "s1", "t", Outcome::Correctable);
        apply(&mut st, "s1", "t", Outcome::Environment);
        apply(&mut st, "s1", "t", Outcome::Ok);
        assert_eq!(st.stats["t"].recovered, 1);
        assert_eq!(st.stats["t"].calls, 7);
    }
}
