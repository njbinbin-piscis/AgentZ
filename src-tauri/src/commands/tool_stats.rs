//! Per-tool call-outcome statistics (see `piscis_kernel::tool_stats`).

use serde_json::{json, Value};

/// Rows sorted by call count. `first_try_fail_pct` is the share of calls that
/// failed with a correctable (call-shape) error; `recovery_pct` is how many of
/// those the same session then fixed on its next call of that tool.
#[tauri::command]
pub fn tool_stats_snapshot() -> Vec<Value> {
    piscis_kernel::tool_stats::snapshot()
        .into_iter()
        .map(|(name, s)| {
            let pct = |n: u64, d: u64| if d == 0 { 0.0 } else { (n as f64 * 1000.0 / d as f64).round() / 10.0 };
            json!({
                "tool": name,
                "calls": s.calls,
                "ok": s.ok,
                "correctable": s.correctable,
                "blocked": s.blocked,
                "environment": s.environment,
                "recovered": s.recovered,
                "def_chars": s.def_chars,
                "first_try_fail_pct": pct(s.correctable, s.calls),
                "recovery_pct": pct(s.recovered, s.correctable),
            })
        })
        .collect()
}
