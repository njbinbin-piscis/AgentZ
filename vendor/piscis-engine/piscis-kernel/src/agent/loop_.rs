/// Agent Loop — the core recursive query-tool-result cycle.
///
/// Runtime guards inspired by OpenClaw's middleware architecture:
/// - Per-tool loop detection (generic_repeat, known_poll, ping_pong, circuit_breaker)
/// - No-progress detection via result hash comparison
/// - Tool result size guard (dynamic, based on context window)
/// - In-memory message compaction for long-running tasks
/// - Checkpoint size guard for DB persistence
use super::compaction_strategy::{
    CompactionRequest, CompactionResult, CompactionStrategy, CompactionTrigger,
};
use super::harness::CompactionTier;
use super::messages::AgentEvent;
use super::tool::{ToolContext, ToolRegistry};
use super::vision;
use crate::llm::{ContentBlock, ImageSource, LlmClient, LlmMessage, MessageContent};
use crate::policy::{PolicyDecision, PolicyGate};
use crate::store::Database;
use anyhow::Result;
use futures::future::join_all;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, info, warn};

/// Invocation completion is separate from owned subprocess cleanup. A normally
/// returned error ends the invocation; dropped/aborted futures remain unconfirmed.
async fn call_with_completion(
    tool: &dyn super::tool::Tool,
    input: serde_json::Value,
    ctx: &ToolContext,
) -> Result<super::tool::ToolResult> {
    let receipt = ctx
        .owned_completion()
        .register()
        .map_err(anyhow::Error::msg)?;
    let result = tool.call(input, ctx).await;
    receipt.complete(Ok(()));
    result.map(|mut r| {
        if let std::borrow::Cow::Owned(clean) = crate::tools::output::sanitize_tool_text(&r.content) {
            r.content = clean;
        }
        r
    })
}

const DEFAULT_MAX_ITERATIONS: usize = 50;
const TOOL_TIMEOUT_SECS: u64 = 120;
/// After a cancel request, a running tool gets this long to finish its own teardown
/// (for example a sandboxed shell terminating its Job Object) before its future is dropped.
const CANCEL_GRACE_SECS: u64 = 8;
/// Slack over a tool's own timeout so it can finish its timeout handling
/// (e.g. moving a shell command to the background) before the loop gives up.
const TOOL_TIMEOUT_SLACK_SECS: u64 = 30;
/// Upper bound for tool-requested timeouts.
const TOOL_TIMEOUT_MAX_SECS: u64 = 3600;

/// Loop-level timeout for one tool call: the default, or longer when the
/// call asks for its own (`timeout` / `timeout_secs` seconds, `timeout_ms`).
fn tool_timeout_for(input: &serde_json::Value) -> u64 {
    let own = input["timeout"]
        .as_u64()
        .or_else(|| input["timeout_secs"].as_u64())
        .or_else(|| input["timeout_ms"].as_u64().map(|ms| ms.div_ceil(1000)));
    match own {
        Some(s) => (s.min(TOOL_TIMEOUT_MAX_SECS) + TOOL_TIMEOUT_SLACK_SECS).max(TOOL_TIMEOUT_SECS),
        None => TOOL_TIMEOUT_SECS + TOOL_TIMEOUT_SLACK_SECS,
    }
}
const READ_TOOL_MAX_CONCURRENCY: usize = 4;
// Logical primary-agent calls only: auxiliary compaction/vision and client wire
// retries remain outside this budget. Correction/overflow share one iteration window.
const MAX_SAME_MODEL_ATTEMPTS: u32 = 5;
const MAX_MODEL_CANDIDATES: usize = 4;
const MAX_MAIN_CALLS_PER_ITERATION: u32 = 12;

fn retry_backoff(attempt: u32) -> std::time::Duration {
    std::time::Duration::from_secs(1u64.checked_shl(attempt.min(3)).unwrap_or(8).min(8))
}

fn bounded_model_candidates(primary: String, fallbacks: &[String]) -> Vec<String> {
    let mut candidates = vec![primary];
    for fallback in fallbacks {
        if candidates.len() == MAX_MODEL_CANDIDATES {
            break;
        }
        if !candidates.contains(fallback) {
            candidates.push(fallback.clone());
        }
    }
    candidates
}

// ── Runtime guard thresholds ─────────────────────────────────────────────────
// Purpose: ONLY prevent true infinite loops / dead loops where the agent is
// stuck making zero progress with identical input+output.  We do NOT restrict
// exploration — agents must be free to try, fail, retry, and iterate through
// multi-step workflows (browser navigation, desktop automation, search
// refinement, etc.).  Thresholds are deliberately high so they only fire as
// a last-resort safety net, never as a premature "you called this too many
// times" nag.
const TOOL_CALL_HISTORY_SIZE: usize = 128;
const WARNING_THRESHOLD: usize = 64;
const CRITICAL_THRESHOLD: usize = 128;
const CIRCUIT_BREAKER_THRESHOLD: usize = 64;
const RESEARCH_WARNING_THRESHOLD: usize = 64;
const RESEARCH_CRITICAL_THRESHOLD: usize = 128;
const RESEARCH_RECENT_WINDOW: usize = 64;
const PING_PONG_WARNING: usize = 64;
const PING_PONG_CRITICAL: usize = 128;
const TOOL_RESULT_HARD_MAX_CHARS: usize = 48_000;
const CONTEXT_SINGLE_RESULT_SHARE: f64 = 0.5;
const CHECKPOINT_MAX_BYTES: usize = 8_000_000;

// ── Vision-loop (screenshot-heavy) nudge ────────────────────────────────
// Desktop-automation vision agents (WeChat/QQ input, screen-control tasks)
// frequently fall into "describe-then-verify" loops: move → screenshot →
// analyze → move → screenshot → analyze, repeating the same observation
// 3–5 times without actually executing the target action (click, type,
// press Enter). Generic detectors miss this pattern because the tool
// inputs/outputs differ across iterations (different coordinates, fresh
// screenshots each time). Instead we detect by *consecutive vision-only
// streak*: if the agent makes too many vision/observation calls in a row
// without a substantive action (click, type, hotkey, drag, file edit,
// shell command, etc.) in between, we nudge it to commit.
//
// The threshold is platform-aware:
//   * Windows — UIA (`uia.find`/`uia.click`/`uia.get_value`) lets the
//     agent act without re-screenshoting after every move, so a long
//     vision-only streak is a genuine sign of over-verification. We use
//     a tighter threshold.
//   * Linux / macOS — the only automation path is `desktop_automation`
//     (xdotool/AppleScript) combined with `screen_capture`. Visual
//     verification between moves is the *correct* workflow, so we use
//     a much more lenient threshold. Move→screenshot→move→screenshot
//     is treated as normal iterative calibration, not a loop.
#[cfg(target_os = "windows")]
const VISION_LOOP_CONSECUTIVE_THRESHOLD: usize = 8;
#[cfg(not(target_os = "windows"))]
const VISION_LOOP_CONSECUTIVE_THRESHOLD: usize = 15;

/// Tools that are known polling/status-checking tools. These get stricter
/// no-progress detection (inspired by OpenClaw's known_poll_no_progress).
const KNOWN_POLL_TOOLS: &[&str] = &["process_control", "shell", "powershell_query"];
const KNOWLEDGE_GATHERING_TOOLS: &[&str] = &["web_search", "browser"];
/// Keywords that identify screenshot / vision-analyze tools. Substring
/// match (case-insensitive) — covers typical names like
/// `take_screenshot`, `screen_analyze`, `vision_context`, `screen_capture`,
/// `vision_analyze`, `screenshot`, etc. without requiring exact names.
const VISION_LOOP_KEYWORDS: &[&str] = &[
    "screenshot",
    "screen_analyze",
    "vision_analyze",
    "screen_capture",
    "vision_context",
];

/// Desktop-automation actions that produce a real side-effect on the desktop
/// (clicking, typing, pressing keys, dragging, scrolling, launching apps).
/// Observation-only actions (`move_mouse`, `get_cursor_position`,
/// `list_windows`) are intentionally excluded — they do not change the state
/// of the target application.
const SUBSTANTIVE_DESKTOP_ACTIONS: &[&str] = &[
    "click",
    "double_click",
    "right_click",
    "type_text",
    "hotkey",
    "drag",
    "drag_to",
    "scroll",
    "launch_app",
    "activate_window",
];

/// Returns `true` if the tool name matches a vision / screenshot / analysis
/// tool (substring match against `VISION_LOOP_KEYWORDS`).
fn is_vision_tool(name: &str) -> bool {
    let lower = name.to_lowercase();
    VISION_LOOP_KEYWORDS.iter().any(|kw| lower.contains(kw))
}

/// Returns `true` when the pending tool call is a *substantive* desktop
/// automation action — i.e. one that actually changes application state
/// (click, type, hotkey, drag, scroll, launch_app, activate_window).
///
/// On Windows, `uia` actions are also considered substantive (UIA is the
/// preferred automation path and does not require screenshot verification).
fn is_substantive_desktop_action(name: &str, input: &serde_json::Value) -> bool {
    // desktop_automation(action="click"|"type_text"|...) — substantive
    if name == "desktop_automation" {
        if let Some(action) = input.get("action").and_then(|v| v.as_str()) {
            return SUBSTANTIVE_DESKTOP_ACTIONS.contains(&action);
        }
    }
    // Windows UIA: any uia action (find, click, type, send_hotkey, …)
    // is substantive — UIA can target elements without visual verification.
    #[cfg(target_os = "windows")]
    if name == "uia" {
        return true;
    }
    false
}

/// On non-Windows platforms, `desktop_automation(action="move_mouse")` is
/// treated as part of a legitimate calibration cycle
/// (move → screenshot → verify position → click). Returning `true` here
/// causes the vision-loop detector to treat the turn as *non*-vision-only,
/// effectively exempting the move→screenshot→move→screenshot pattern from
/// the consecutive-vision streak counter.
///
/// On Windows, UIA handles positioning without visual verification, so
/// move_mouse is never considered calibration — it always returns `false`.
fn is_calibration_move(name: &str, input: &serde_json::Value) -> bool {
    #[cfg(target_os = "windows")]
    {
        let _ = (name, input);
        return false;
    }
    #[cfg(not(target_os = "windows"))]
    {
        if name == "desktop_automation" {
            if let Some(action) = input.get("action").and_then(|v| v.as_str()) {
                return action == "move_mouse" || action == "get_cursor_position";
            }
        }
        false
    }
}

/// Build the user-facing warning message for the vision-loop detector.
fn vision_loop_warning(threshold: usize) -> String {
    format!(
        "[视觉循环检测] 已连续{}轮仅执行截图/视觉分析，未执行任何实质性动作\
         （点击输入框、输入文字、按回车等）。请立即执行以下操作：\
         (1) 点击输入框获得焦点（一次 click）；\
         (2) 在同一回合内直接输入目标文本；\
         (3) 按回车发送。不要再截图验证，不要再移动鼠标确认位置。",
        threshold
    )
}

static TOOL_RATE_STATE: Lazy<Mutex<HashMap<String, Vec<std::time::Instant>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// User-controlled confirmation flags from Settings.
#[derive(Debug, Clone, Copy)]
pub struct ConfirmFlags {
    pub confirm_shell: bool,
    pub confirm_file_write: bool,
}

/// Live confirmation preferences — shared across harness instances so a
/// settings change takes effect for in-flight agent runs without restart.
pub type ConfirmFlagsHandle = Arc<std::sync::RwLock<ConfirmFlags>>;

pub fn confirm_flags_handle(confirm_shell: bool, confirm_file_write: bool) -> ConfirmFlagsHandle {
    Arc::new(std::sync::RwLock::new(ConfirmFlags {
        confirm_shell,
        confirm_file_write,
    }))
}

pub fn sync_confirm_flags(
    handle: &ConfirmFlagsHandle,
    confirm_shell: bool,
    confirm_file_write: bool,
) {
    if let Ok(mut flags) = handle.write() {
        flags.confirm_shell = confirm_shell;
        flags.confirm_file_write = confirm_file_write;
    }
}

pub type ConfirmationResponseMap = Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<bool>>>>;

/// Shared plan-state map (session_id → plan todo items).
///
/// The agent loop reads this when the model wants to exit to detect
/// unfinished todos and inject a reminder. Desktop hosts wire up their
/// `AppState.plan_state` here; headless hosts typically leave `AgentLoop::
/// plan_state` as `None`.
pub type PlanStateHandle = Arc<Mutex<HashMap<String, Vec<crate::agent::plan::PlanTodoItem>>>>;

// ── Loop Detection (per-tool tracking, inspired by OpenClaw) ─────────────────

/// Severity level for loop detection, matching OpenClaw's warning/critical model.
#[derive(Debug, Clone, Copy, PartialEq)]
enum LoopLevel {
    Ok,
    Warning,
    Critical,
}

/// Which detector triggered.
#[derive(Debug, Clone)]
enum LoopDetector {
    GenericRepeat,
    KnownPollNoProgress,
    PingPong,
    GlobalCircuitBreaker,
    /// Vision-agent screenshot-analyze loop: the agent is spending too much
    /// time re-describing the scene with screenshots instead of committing
    /// to concrete actions (click, type, press). Nudges the model to
    /// execute rather than observe.
    VisionLoop,
}

/// Result of loop detection analysis.
#[derive(Debug, Clone)]
struct LoopDetectionResult {
    level: LoopLevel,
    detector: Option<LoopDetector>,
    count: usize,
    message: String,
}

impl LoopDetectionResult {
    fn ok() -> Self {
        Self {
            level: LoopLevel::Ok,
            detector: None,
            count: 0,
            message: String::new(),
        }
    }
}

/// A single recorded tool call with its outcome, for per-tool history tracking.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ToolCallRecord {
    name: String,
    input_hash: u64,
    result_hash: u64,
    /// Result hash with volatile parts (digits, recovery trailer) removed, so
    /// reruns that differ only in timings still compare equal.
    #[serde(default)]
    norm_hash: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AgentCheckpointPayload {
    base_context_hash: u64,
    base_message_count: usize,
    messages: Vec<LlmMessage>,
    loop_history: Vec<ToolCallRecord>,
    seen_notifications: Vec<String>,
}

/// Per-session tool call history for loop detection.
/// Maintains a sliding window of recent tool calls (like OpenClaw's toolCallHistory).
struct LoopDetectorState {
    history: Vec<ToolCallRecord>,
    /// Number of consecutive turns whose *only* tool calls were vision /
    /// observation calls (screenshot, screen_capture, screen_analyze,
    /// vision_context, desktop_automation(move_mouse),
    /// desktop_automation(list_windows), uia(find), etc.). Reset to 0
    /// the moment any substantive action runs (click, type, hotkey,
    /// drag, file edit, shell command, …).
    consecutive_vision_only_turns: usize,
    /// Warnings already issued per call hash; ignored warnings escalate to a block.
    warned: HashMap<u64, usize>,
}

impl LoopDetectorState {
    /// Rebuild the window from persisted history so a new turn does not start
    /// with a clean slate while the model is still repeating itself.
    fn seed_from_messages(&mut self, messages: &[LlmMessage]) {
        let mut calls: HashMap<String, (String, serde_json::Value)> = HashMap::new();
        for message in messages {
            let MessageContent::Blocks(blocks) = &message.content else {
                continue;
            };
            for block in blocks {
                match block {
                    ContentBlock::ToolUse { id, name, input } => {
                        calls.insert(id.clone(), (name.clone(), input.clone()));
                    }
                    ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        ..
                    } => {
                        if let Some((name, input)) = calls.get(tool_use_id) {
                            self.record(
                                name,
                                input,
                                stable_hash_result(content),
                                stable_hash_normalized(content),
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// Record a completed tool call with its result hash.
    fn record(
        &mut self,
        name: &str,
        input: &serde_json::Value,
        result_hash: u64,
        norm_hash: u64,
    ) {
        let input_hash = stable_hash_input(name, input);
        self.history.push(ToolCallRecord {
            name: name.to_string(),
            input_hash,
            result_hash,
            norm_hash,
        });
        if self.history.len() > TOOL_CALL_HISTORY_SIZE {
            self.history.remove(0);
        }
    }

    /// Run all detectors against the current history, return the most severe result.
    ///
    /// `batch` is the full set of tool calls being dispatched in the same
    /// LLM turn as `pending_name`/`pending_input`. We consult it to decide
    /// whether the pending turn contains a *substantive* action alongside
    /// the vision call (e.g., `screen_capture` + `desktop_automation(click)`
    /// dispatched in parallel) — that is exactly the “commit to action”
    /// pattern the vision-loop detector is trying to encourage, so it
    /// should reset the consecutive-vision streak.
    fn detect(
        &mut self,
        pending_name: &str,
        pending_input: &serde_json::Value,
        batch: &[(String, String, serde_json::Value)],
    ) -> LoopDetectionResult {
        let mut result = self.detect_inner(pending_name, pending_input, batch);
        if result.level == LoopLevel::Warning
            && !matches!(result.detector, Some(LoopDetector::VisionLoop))
        {
            let hash = stable_hash_input(pending_name, pending_input);
            let n = self.warned.entry(hash).or_insert(0);
            *n += 1;
            if *n > 3 {
                result = LoopDetectionResult {
                    level: LoopLevel::Critical,
                    detector: result.detector,
                    count: result.count,
                    message: format!(
                        "Tool '{}' was blocked: repeated warnings about this identical call were ignored. Do not retry it. Tell the user what is failing and ask how to proceed.",
                        pending_name
                    ),
                };
            }
        }
        result
    }

    fn detect_inner(
        &mut self,
        pending_name: &str,
        pending_input: &serde_json::Value,
        batch: &[(String, String, serde_json::Value)],
    ) -> LoopDetectionResult {
        let pending_hash = stable_hash_input(pending_name, pending_input);

        // 1. Global circuit breaker: same tool+input with no progress
        let no_progress_streak = self.count_no_progress_streak(pending_name, pending_hash);
        if no_progress_streak >= CIRCUIT_BREAKER_THRESHOLD {
            return LoopDetectionResult {
                level: LoopLevel::Critical,
                detector: Some(LoopDetector::GlobalCircuitBreaker),
                count: no_progress_streak,
                message: format!(
                    "全局熔断：工具 '{}' 已连续{}次调用且结果无变化，强制终止该工具调用。请换一种方法。",
                    pending_name, no_progress_streak
                ),
            };
        }

        // 2. Known poll tools: stricter thresholds for status-checking tools
        let is_poll = KNOWN_POLL_TOOLS.iter().any(|t| pending_name.contains(t));
        if is_poll {
            let streak = self.count_same_tool_streak(pending_name, pending_hash);
            if streak >= CRITICAL_THRESHOLD {
                return LoopDetectionResult {
                    level: LoopLevel::Critical,
                    detector: Some(LoopDetector::KnownPollNoProgress),
                    count: streak,
                    message: format!(
                        "轮询工具 '{}' 已连续调用{}次且无进展，强制终止。请检查目标状态或换一种方法。",
                        pending_name, streak
                    ),
                };
            }
            if streak >= WARNING_THRESHOLD {
                return LoopDetectionResult {
                    level: LoopLevel::Warning,
                    detector: Some(LoopDetector::KnownPollNoProgress),
                    count: streak,
                    message: format!(
                        "轮询工具 '{}' 已连续调用{}次，结果无变化。建议检查是否需要换一种方法或增加等待时间。",
                        pending_name, streak
                    ),
                };
            }
        }

        // 2.5. Research tools: allow query refinement, but stop endless "one more search"
        let is_research = KNOWLEDGE_GATHERING_TOOLS
            .iter()
            .any(|t| pending_name.contains(t));
        if is_research {
            // Count only calls with the same tool+input — different operations
            // like browser launch/navigate/type/press are not repetitions.
            let same_input_count = self.count_recent_tool_family_same_input(
                pending_name,
                pending_hash,
                RESEARCH_RECENT_WINDOW,
            ) + 1;
            if same_input_count >= RESEARCH_CRITICAL_THRESHOLD {
                return LoopDetectionResult {
                    level: LoopLevel::Critical,
                    detector: Some(LoopDetector::GenericRepeat),
                    count: same_input_count,
                    message: format!(
                        "调研工具 '{}' 在最近步骤中已累计调用{}次。请停止继续搜集，先基于现有证据总结结论、明确不确定性，再决定是否还需要补充一轮查询。",
                        pending_name, same_input_count
                    ),
                };
            }
            if same_input_count >= RESEARCH_WARNING_THRESHOLD {
                return LoopDetectionResult {
                    level: LoopLevel::Warning,
                    detector: Some(LoopDetector::GenericRepeat),
                    count: same_input_count,
                    message: format!(
                        "调研工具 '{}' 在最近步骤中已累计调用{}次。请优先收束：总结已有发现、列出分歧点，只在确有信息缺口时再补充搜索。",
                        pending_name, same_input_count
                    ),
                };
            }
        }

        // 3. Ping-pong detection: A→B→A→B alternating pattern
        let ping_pong_count = self.detect_ping_pong(pending_name, pending_hash);
        if ping_pong_count >= PING_PONG_CRITICAL {
            return LoopDetectionResult {
                level: LoopLevel::Critical,
                detector: Some(LoopDetector::PingPong),
                count: ping_pong_count,
                message: format!(
                    "检测到工具交替调用循环（ping-pong），已持续{}次。强制终止，请分析原因并换一种方法。",
                    ping_pong_count
                ),
            };
        }
        if ping_pong_count >= PING_PONG_WARNING {
            return LoopDetectionResult {
                level: LoopLevel::Warning,
                detector: Some(LoopDetector::PingPong),
                count: ping_pong_count,
                message: format!(
                    "检测到工具交替调用模式，已持续{}次。请检查是否陷入了循环，考虑换一种方法。",
                    ping_pong_count
                ),
            };
        }

        // 3.5. Vision-loop (screenshot-analyze) streak detection.
        //
        // Desktop-automation vision agents repeatedly do:
        //   move_mouse → take_screenshot → screen_analyze → move_mouse → …
        // and never actually commit to the target action (click the input,
        // type the message, press Enter). Generic detectors miss this
        // because each call uses different coordinates / fresh screenshots,
        // so hashes differ. Detect by *consecutive vision-only streak*:
        // count turns whose ONLY tool calls are vision/observation, reset
        // to zero the moment any substantive action runs (click, type,
        // hotkey, drag, file edit, shell command, …). When the streak
        // crosses the platform-specific threshold, emit a Warning nudge
        // and reset so we don't spam every subsequent turn.
        //
        // IMPORTANT: this is Warning-only (never Critical) — we never
        // *block* the screenshot. We just tell the model to stop
        // over-verifying and commit to a concrete action sequence.
        //
        // Evaluate the *whole* pending batch: if the model is issuing
        // e.g. `click + screen_capture` in parallel, treat the turn as
        // substantive and reset the streak — that is exactly the kind
        // of "commit to action" behavior we want to encourage.
        let is_vision_only_turn = is_vision_tool(pending_name)
            && !batch.iter().any(|(_, n, inp)| {
                n.as_str() != pending_name || inp != pending_input
            }) && !is_substantive_desktop_action(pending_name, pending_input)
            && !batch.iter().any(|(_, n, inp)| {
                !is_vision_tool(n) || is_substantive_desktop_action(n, inp)
            })
            // On Linux/macOS, a batch containing move_mouse + screen_capture
            // is normal iterative calibration (move → verify → click), not
            // a vision loop. Exempt it unconditionally: on Windows this
            // always returns false so the check is a no-op.
            && !batch.iter().any(|(_, n, inp)| is_calibration_move(n, inp));
        if is_vision_only_turn {
            self.consecutive_vision_only_turns =
                self.consecutive_vision_only_turns.saturating_add(1);
            if self.consecutive_vision_only_turns >= VISION_LOOP_CONSECUTIVE_THRESHOLD {
                // Reset so the nudge doesn't re-fire on the very next
                // vision-only call. If the model ignores the nudge and
                // keeps taking screenshots, we'll warn again after
                // another full streak of threshold turns.
                self.consecutive_vision_only_turns = 0;
                return LoopDetectionResult {
                    level: LoopLevel::Warning,
                    detector: Some(LoopDetector::VisionLoop),
                    count: VISION_LOOP_CONSECUTIVE_THRESHOLD,
                    message: vision_loop_warning(VISION_LOOP_CONSECUTIVE_THRESHOLD),
                };
            }
        } else {
            self.consecutive_vision_only_turns = 0;
        }

        // 3.8. Same call, equivalent outcome, again and again (not necessarily
        // consecutive: edit-then-rerun cycles that never change the result).
        // Only command-running tools: repeated reads of the same file
        // legitimately return the same content.
        let runs_commands = pending_name == "code_run";
        if runs_commands {
            if let Some(last) = self
                .history
                .iter()
                .rev()
                .find(|r| r.name == pending_name && r.input_hash == pending_hash)
            {
                let same = self
                    .history
                    .iter()
                    .filter(|r| {
                        r.name == pending_name
                            && r.input_hash == pending_hash
                            && r.norm_hash == last.norm_hash
                    })
                    .count();
                if same >= 8 {
                    return LoopDetectionResult {
                        level: LoopLevel::Critical,
                        detector: Some(LoopDetector::GlobalCircuitBreaker),
                        count: same,
                        message: format!(
                            "Tool '{}' was run {} times with the same outcome and no progress. Stop retrying: report to the user what is failing and ask for direction.",
                            pending_name, same
                        ),
                    };
                }
                if same >= 4 {
                    return LoopDetectionResult {
                        level: LoopLevel::Warning,
                        detector: Some(LoopDetector::GenericRepeat),
                        count: same,
                        message: format!(
                            "Tool '{}' has produced the same outcome {} times. Rerunning without a real change will not help; change the approach or report the blocker.",
                            pending_name, same
                        ),
                    };
                }
            }
        }

        // 3.9. Early nudge: the same call several rounds in a row. The generic
        // thresholds below are deliberately high; this one only asks the model
        // to change approach before some providers reject the repetition.
        if !KNOWN_POLL_TOOLS.contains(&pending_name) {
            let streak = self.count_same_tool_streak(pending_name, pending_hash);
            if streak >= 2 {
                return LoopDetectionResult {
                    level: LoopLevel::Warning,
                    detector: Some(LoopDetector::GenericRepeat),
                    count: streak + 1,
                    message: format!(
                        "Tool '{}' was called with identical arguments {} times in a row. Change the arguments or approach, or report what is blocking you.",
                        pending_name,
                        streak + 1
                    ),
                };
            }
        }

        // 4. Generic repeat: same tool+input appearing too many times
        let repeat_count = self.count_same_tool_total(pending_name, pending_hash);
        if repeat_count >= CRITICAL_THRESHOLD {
            return LoopDetectionResult {
                level: LoopLevel::Critical,
                detector: Some(LoopDetector::GenericRepeat),
                count: repeat_count,
                message: format!(
                    "工具 '{}' 以相同参数被调用了{}次，强制终止。请换一种方法解决问题。",
                    pending_name, repeat_count
                ),
            };
        }
        if repeat_count >= WARNING_THRESHOLD {
            return LoopDetectionResult {
                level: LoopLevel::Warning,
                detector: Some(LoopDetector::GenericRepeat),
                count: repeat_count,
                message: format!(
                    "工具 '{}' 以相同参数已被调用{}次。请检查是否需要换一种方法，避免无效重复。",
                    pending_name, repeat_count
                ),
            };
        }

        LoopDetectionResult::ok()
    }

    /// Count consecutive calls to the same tool+input at the tail of history
    /// where the result hash is also unchanged (no progress).
    fn count_no_progress_streak(&self, name: &str, input_hash: u64) -> usize {
        let mut count = 0usize;
        let mut last_result: Option<u64> = None;
        for rec in self.history.iter().rev() {
            if rec.name == name && rec.input_hash == input_hash {
                match last_result {
                    None => {
                        last_result = Some(rec.result_hash);
                        count += 1;
                    }
                    Some(lr) if lr == rec.result_hash => {
                        count += 1;
                    }
                    _ => break,
                }
            } else {
                break;
            }
        }
        count
    }

    /// Count consecutive calls to the same tool+input at the tail of history.
    fn count_same_tool_streak(&self, name: &str, input_hash: u64) -> usize {
        self.history
            .iter()
            .rev()
            .take_while(|r| r.name == name && r.input_hash == input_hash)
            .count()
    }

    /// Count total occurrences of the same tool+input in the history window.
    fn count_same_tool_total(&self, name: &str, input_hash: u64) -> usize {
        self.history
            .iter()
            .filter(|r| r.name == name && r.input_hash == input_hash)
            .count()
    }

    /// Count recent occurrences in the same tool family **with the same input hash**.
    /// Different tool operations (e.g. browser launch vs navigate vs type) count
    /// separately because they have different inputs — only truly identical calls
    /// are flagged as research repetition.
    fn count_recent_tool_family_same_input(
        &self,
        name: &str,
        input_hash: u64,
        window: usize,
    ) -> usize {
        self.history
            .iter()
            .rev()
            .take(window)
            .filter(|r| same_tool_family(&r.name, name) && r.input_hash == input_hash)
            .count()
    }

    /// Detect A→B→A→B alternating pattern at the tail of history.
    /// Returns the number of alternating pairs found.
    fn detect_ping_pong(&self, pending_name: &str, pending_hash: u64) -> usize {
        if self.history.len() < 2 {
            return 0;
        }

        let last = self.history.last().unwrap();
        if last.name == pending_name && last.input_hash == pending_hash {
            return 0; // Same as last — not a ping-pong, it's a repeat
        }

        // Check if the pattern is: ...A, B, A, B where pending is A and last is B
        let a_name = pending_name;
        let a_hash = pending_hash;
        let b_name = &last.name;
        let b_hash = last.input_hash;

        let mut alternations = 0usize;
        let mut expect_b = true; // Walking backwards from last, first should be B
        for rec in self.history.iter().rev() {
            if expect_b && rec.name == *b_name && rec.input_hash == b_hash {
                alternations += 1;
                expect_b = false;
            } else if !expect_b && rec.name == a_name && rec.input_hash == a_hash {
                expect_b = true;
            } else {
                break;
            }
        }
        alternations
    }

    /// Returns `(vision_calls, window_size)` over the last `window` tool
    /// calls in history. Vision calls are identified by substring match
    /// against `VISION_LOOP_KEYWORDS` (covers `take_screenshot`,
    /// `screen_analyze`, `screen_capture`, `vision_analyze`, `vision_context`,
    /// etc.). `window_size` may be smaller than `window` if history hasn't
    /// yet accumulated that many entries.
    #[allow(dead_code)]
    fn count_recent_vision_density(&self, window: usize) -> (usize, usize) {
        let start = self.history.len().saturating_sub(window);
        let slice = &self.history[start..];
        let mut vision = 0usize;
        for rec in slice {
            let lower = rec.name.to_lowercase();
            if VISION_LOOP_KEYWORDS.iter().any(|kw| lower.contains(kw)) {
                vision += 1;
            }
        }
        (vision, slice.len())
    }
}

/// Compute a stable hash for a tool name + normalized input.
fn stable_hash_input(name: &str, input: &serde_json::Value) -> u64 {
    let mut hasher = DefaultHasher::new();
    name.hash(&mut hasher);
    let mut normalized = input.clone();
    if let Some(obj) = normalized.as_object_mut() {
        obj.remove("_trace_id");
    }
    normalized.to_string().hash(&mut hasher);
    hasher.finish()
}

fn stable_hash_messages(messages: &[LlmMessage]) -> u64 {
    let mut hasher = DefaultHasher::new();
    serde_json::to_string(messages)
        .unwrap_or_default()
        .hash(&mut hasher);
    hasher.finish()
}

fn same_tool_family(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let a_research = KNOWLEDGE_GATHERING_TOOLS.iter().any(|t| a.contains(t));
    let b_research = KNOWLEDGE_GATHERING_TOOLS.iter().any(|t| b.contains(t));
    a_research && b_research
}

/// Compute a stable hash of a single tool result content string.
/// Hash of a tool result without volatile parts: everything from the recovery
/// trailer on (it carries a unique operation id) and all ASCII digits (timings,
/// counters) are ignored.
fn stable_hash_normalized(content: &str) -> u64 {
    let body = content.split("[recovery_v1]").next().unwrap_or(content);
    let mut hasher = DefaultHasher::new();
    for ch in body.chars().filter(|c| !c.is_ascii_digit()) {
        ch.hash(&mut hasher);
    }
    hasher.finish()
}

fn stable_hash_result(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

// ── Tool Result Guard ────────────────────────────────────────────────────────

/// Truncate a tool result string if it exceeds the limit, keeping head + tail.
/// The limit is the smaller of the hard max and a dynamic limit based on context window.
fn guard_tool_result_content(content: &str, max_chars: usize) -> String {
    let limit = max_chars.min(TOOL_RESULT_HARD_MAX_CHARS);
    let char_count = content.chars().count();
    if char_count <= limit {
        return content.to_string();
    }
    let head_size = (limit * 3) / 4;
    let tail_size = limit / 4;
    let head: String = content.chars().take(head_size).collect();
    let tail: String = content.chars().skip(char_count - tail_size).collect();
    format!(
        "{}\n\n[... truncated {} chars (limit: {}) ...]\n\n{}",
        head,
        char_count - head_size - tail_size,
        limit,
        tail
    )
}

/// Compute dynamic per-result char limit based on context window.
/// Inspired by OpenClaw's SINGLE_TOOL_RESULT_CONTEXT_SHARE.
fn dynamic_result_limit(context_window_tokens: usize) -> usize {
    let context_chars = context_window_tokens * 4; // ~4 chars per token
    let limit = (context_chars as f64 * CONTEXT_SINGLE_RESULT_SHARE) as usize;
    limit.clamp(4_000, TOOL_RESULT_HARD_MAX_CHARS)
}

// ── In-memory Message Compaction ─────────────────────────────────────────────

// ---------------------------------------------------------------------------
// Context compaction helpers
// ---------------------------------------------------------------------------

pub use super::compaction::{
    CTX_COMPACT_AFTER, CTX_FULL_TURNS, CTX_KEEP_RECENT_TOOL_CARRIERS, CTX_PRESERVE_RECENT_TURNS,
    CTX_TRIM_HEAD, CTX_TRIM_TAIL,
};
const SUMMARY_KEEP_RECENT_RATIO: f64 = 0.60; // keep newest 60% of budget intact

/// Level-1 compaction: trim oversized individual tool results (head + tail).
///
/// `single_limit_tokens` is a hard token ceiling measured by the shared
/// [`crate::llm::estimate_tokens`] estimator. The retained head, marker, and
/// tail together never exceed that ceiling. The caller handles the configured
/// zero value as "disabled" by skipping this helper.
pub fn compact_trim_tool_results(messages: &mut [LlmMessage], single_limit_tokens: usize) -> bool {
    let mut changed = false;
    for msg in messages.iter_mut() {
        if msg.role != "user" {
            continue;
        }
        if let MessageContent::Blocks(ref mut blocks) = msg.content {
            for block in blocks.iter_mut() {
                if let ContentBlock::ToolResult { content, .. } = block {
                    if crate::llm::estimate_tokens(content) > single_limit_tokens {
                        *content = trim_tool_result_to_token_limit(content, single_limit_tokens);
                        changed = true;
                    }
                }
            }
        }
    }
    changed
}

/// Select a Unicode-safe head/tail view under a token budget. Each binary
/// search probe builds at most one candidate and the source chars are collected
/// once, keeping the work O(n log n) without repeated nth-char scans.
fn trim_tool_result_to_token_limit(content: &str, token_limit: usize) -> String {
    let chars: Vec<char> = content.chars().collect();
    if chars.is_empty() || token_limit == 0 {
        return String::new();
    }

    let candidate = |keep_chars: usize| {
        let keep_chars = keep_chars.min(chars.len().saturating_sub(1));
        let ratio_total = CTX_TRIM_HEAD + CTX_TRIM_TAIL;
        let head_chars = if keep_chars == 0 {
            0
        } else {
            keep_chars
                .saturating_mul(CTX_TRIM_HEAD)
                .div_ceil(ratio_total)
        }
        .min(keep_chars);
        let tail_chars = keep_chars - head_chars;
        let removed = chars.len() - keep_chars;
        let head: String = chars[..head_chars].iter().collect();
        let tail_start = chars.len() - tail_chars;
        let tail: String = chars[tail_start..].iter().collect();
        format!("{}\n... [{} chars removed] ...\n{}", head, removed, tail)
    };

    let marker_only = candidate(0);
    if crate::llm::estimate_tokens(&marker_only) > token_limit {
        // A very small programmatic cap may not fit even the marker. Empty
        // content is the only representation that can honour the hard cap.
        return String::new();
    }

    let mut low = 0usize;
    let mut high = chars.len().saturating_sub(1);
    let mut best = marker_only;
    while low <= high {
        let keep_chars = low + (high - low) / 2;
        let probe = candidate(keep_chars);
        if crate::llm::estimate_tokens(&probe) <= token_limit {
            best = probe;
            low = keep_chars.saturating_add(1);
        } else if keep_chars == 0 {
            break;
        } else {
            high = keep_chars - 1;
        }
    }

    debug_assert!(crate::llm::estimate_tokens(&best) <= token_limit);
    best
}

pub struct CompactionOutcome {
    pub messages: Vec<LlmMessage>,
    pub summary: String,
    /// Prompt tokens billed for the summarisation call. Accumulated into the
    /// session's cumulative_input_tokens so ring indicators reflect reality.
    pub input_tokens: u32,
    /// Completion tokens billed for the summarisation call. Accumulated into
    /// cumulative_output_tokens.
    pub output_tokens: u32,
    /// p7: structured fields extracted from the summariser output (empty
    /// when the model falls back to plain prose). These feed the p6
    /// `StateFrame` so a resumed session knows the latest plan / hint.
    structured_plan_items: Vec<String>,
    structured_next_step_hint: Option<String>,
    /// Phase 2b: rich structured rolling summary (facts / decisions /
    /// open items / evidence / errors learned). `None` when the legacy
    /// prose-summary path was used and no structure was recovered.
    pub structured_rolling: Option<crate::agent::summary_worker::StructuredRollingSummary>,
}

/// The kernel's built-in proactive compaction policy.
///
/// Reproduces the previously-inline behavior: build a demoted request view,
/// estimate tokens, and run up to two Level-2 (rolling-summary) passes —
/// keeping the newest 60% then 30% of the message budget — until the estimate
/// drops under the 60% safety line. Lives in this module so it can reach the
/// private structured fields of [`CompactionOutcome`] and the in-module
/// helpers / constants.
#[derive(Default)]
pub struct DefaultCompaction;

#[async_trait::async_trait]
impl CompactionStrategy for DefaultCompaction {
    async fn compact(&self, req: CompactionRequest<'_>) -> CompactionResult {
        match req.trigger {
            CompactionTrigger::Proactive => self.compact_proactive(req).await,
            CompactionTrigger::Overflow => self.compact_overflow(req).await,
        }
    }
}

impl DefaultCompaction {
    /// Pre-call, estimate-driven adaptive compaction.
    async fn compact_proactive(&self, req: CompactionRequest<'_>) -> CompactionResult {
        let total_budget = req.budget.total as usize;
        let static_overhead_tokens =
            crate::llm::estimate_request_overhead_tokens(Some(req.system_prompt), req.tool_defs);
        let message_budget = total_budget.saturating_sub(static_overhead_tokens);

        let messages = req.messages;

        // Build the demoted request view (minimal receipts for older turns)
        // before classifying the request. Micro keeps this request-view-only
        // demotion and never creates a persisted semantic summary.
        let demoted = build_request_view_messages(
            &messages,
            req.tool_minimals,
            CTX_PRESERVE_RECENT_TURNS,
            CTX_KEEP_RECENT_TOOL_CARRIERS,
            message_budget,
            req.budget.max_tool_result_tokens,
        );
        let req_messages = vision::inject_selected_context(&demoted, req.session_id).await;
        let estimated = crate::llm::estimate_request_input_tokens(
            &req_messages,
            Some(req.system_prompt),
            req.tool_defs,
        );
        let tier = req.budget.classify(estimated.min(u32::MAX as usize) as u32);
        info!(
            "context check: {} messages, ~{} estimated request tokens (total_budget={}, message_budget={}, threshold={}, tier={})",
            messages.len(),
            estimated,
            total_budget,
            message_budget,
            req.budget.trigger_micro,
            tier.as_str(),
        );

        let keep_ratio = match tier {
            CompactionTier::None | CompactionTier::Micro => {
                return CompactionResult {
                    changed: false,
                    messages,
                    rolling_summary: req.rolling_summary.to_string(),
                    next_auto_compact_threshold: req.next_auto_compact_threshold,
                    ..Default::default()
                };
            }
            CompactionTier::Auto => SUMMARY_KEEP_RECENT_RATIO,
            CompactionTier::Full => SUMMARY_KEEP_RECENT_RATIO * 0.5,
        };

        let min_threshold_estimate = (total_budget as f64 * 0.35) as usize;
        let threshold_reached = req.threshold_step > 0
            && req.cumulative_input_tokens >= req.next_auto_compact_threshold
            && estimated >= min_threshold_estimate;
        let keep_tokens = (message_budget as f64 * keep_ratio) as usize;
        warn!(
            "proactive compaction tier={} estimated_tokens={} total_budget={} message_budget={} keep_tokens={} cumulative_input_tokens={} threshold_reached={} min_threshold_estimate={}",
            tier.as_str(), estimated, total_budget, message_budget, keep_tokens, req.cumulative_input_tokens, threshold_reached, min_threshold_estimate
        );
        match compact_summarise(
            messages.clone(),
            keep_tokens,
            req.client,
            req.model,
            req.max_tokens,
            (!req.rolling_summary.trim().is_empty()).then_some(req.rolling_summary),
        )
        .await
        {
            Some(compacted) => {
                let compacted_demoted = build_request_view_messages(
                    &compacted.messages,
                    req.tool_minimals,
                    CTX_PRESERVE_RECENT_TURNS,
                    CTX_KEEP_RECENT_TOOL_CARRIERS,
                    message_budget,
                    req.budget.max_tool_result_tokens,
                );
                let compacted_req_messages =
                    vision::inject_selected_context(&compacted_demoted, req.session_id).await;
                let new_estimated = crate::llm::estimate_request_input_tokens(
                    &compacted_req_messages,
                    Some(req.system_prompt),
                    req.tool_defs,
                );
                info!(
                    "proactive summarisation tier={} complete: {} → {} messages, tokens {} → {}",
                    tier.as_str(),
                    messages.len(),
                    compacted.messages.len(),
                    estimated,
                    new_estimated,
                );
                let mut next_auto_compact_threshold = req.next_auto_compact_threshold;
                if threshold_reached {
                    let step = req.threshold_step.max(1);
                    let cumulative_input_tokens = req
                        .cumulative_input_tokens
                        .saturating_add(i64::from(compacted.input_tokens));
                    let from_old = next_auto_compact_threshold.saturating_add(step);
                    let from_now = cumulative_input_tokens.saturating_add(step);
                    next_auto_compact_threshold = from_old.max(from_now);
                }
                CompactionResult {
                    changed: true,
                    messages: compacted.messages,
                    rolling_summary: compacted.summary,
                    summary_input_tokens: compacted.input_tokens,
                    summary_output_tokens: compacted.output_tokens,
                    next_auto_compact_threshold,
                    structured_plan_items: compacted.structured_plan_items,
                    structured_next_step_hint: compacted.structured_next_step_hint,
                }
            }
            None => {
                warn!(
                    "proactive summarisation tier={} failed, proceeding with current context",
                    tier.as_str()
                );
                CompactionResult {
                    changed: false,
                    messages,
                    rolling_summary: req.rolling_summary.to_string(),
                    next_auto_compact_threshold: req.next_auto_compact_threshold,
                    ..Default::default()
                }
            }
        }
    }

    /// Forced single-pass recovery after the provider rejected an oversized
    /// request. Unlike the proactive path this never gates on the local
    /// estimate (which under-counts — that's why the provider overflowed); it
    /// always summarises, keeping the newest 60% of the message budget. The
    /// cumulative-token threshold is left untouched (this is error recovery,
    /// not threshold-driven compaction). `changed = false` ⇒ unrecoverable.
    async fn compact_overflow(&self, req: CompactionRequest<'_>) -> CompactionResult {
        let total_budget = req.budget.total as usize;
        let static_overhead_tokens =
            crate::llm::estimate_request_overhead_tokens(Some(req.system_prompt), req.tool_defs);
        let message_budget = total_budget.saturating_sub(static_overhead_tokens);
        let keep_tokens = (message_budget as f64 * SUMMARY_KEEP_RECENT_RATIO) as usize;
        warn!(
            "context overflow — attempting LLM summarisation (keep_tokens={})",
            keep_tokens
        );
        match compact_summarise(
            req.messages.clone(),
            keep_tokens,
            req.client,
            req.model,
            req.max_tokens,
            (!req.rolling_summary.trim().is_empty()).then_some(req.rolling_summary),
        )
        .await
        {
            Some(c) => CompactionResult {
                changed: true,
                messages: c.messages,
                rolling_summary: c.summary,
                summary_input_tokens: c.input_tokens,
                summary_output_tokens: c.output_tokens,
                next_auto_compact_threshold: req.next_auto_compact_threshold,
                structured_plan_items: c.structured_plan_items.clone(),
                structured_next_step_hint: c.structured_next_step_hint.clone(),
            },
            None => CompactionResult {
                changed: false,
                messages: req.messages,
                rolling_summary: req.rolling_summary.to_string(),
                next_auto_compact_threshold: req.next_auto_compact_threshold,
                ..Default::default()
            },
        }
    }
}

/// Returns true when `msg` begins with a `ToolResult` block, i.e. it is the
/// reply to a previous `ToolUse`. Such a message must never be the first kept
/// message in a truncated window, or the provider rejects the orphaned result.
fn starts_with_tool_result(msg: &LlmMessage) -> bool {
    matches!(
        &msg.content,
        MessageContent::Blocks(blocks)
            if blocks.first().map(|b| matches!(b, ContentBlock::ToolResult { .. })).unwrap_or(false)
    )
}

/// Whether a message is plain conversational text (no tool_use / tool_result),
/// safe to re-insert out of strict adjacency during relevance retrieval.
fn is_plain_text_message(msg: &LlmMessage) -> bool {
    match &msg.content {
        MessageContent::Text(_) => true,
        MessageContent::Blocks(blocks) => blocks
            .iter()
            .all(|b| matches!(b, ContentBlock::Text { .. } | ContentBlock::Image { .. })),
    }
}

/// Pick the largest suffix of `messages` whose estimated tokens fit within
/// `keep_tokens`, then advance forward past any orphaned leading tool-result
/// message. Returns `(cut_index, kept)` where `cut_index` is how many leading
/// messages were dropped.
fn select_recent_window(messages: &[LlmMessage], keep_tokens: usize) -> usize {
    let mut acc = 0usize;
    let mut cut = messages.len();
    for (idx, msg) in messages.iter().enumerate().rev() {
        let cost = crate::llm::estimate_message_tokens(msg);
        if acc + cost > keep_tokens && idx + 1 < messages.len() {
            cut = idx + 1;
            break;
        }
        acc += cost;
        cut = idx;
    }
    // Never orphan a leading tool-result.
    while cut < messages.len() && starts_with_tool_result(&messages[cut]) {
        cut += 1;
    }
    cut
}

/// Token-budget truncation with no LLM call. Keeps the newest messages that fit
/// within `keep_ratio` of the message budget and drops older ones. Preserves
/// tool_use/tool_result adjacency by never starting the window on a tool
/// result. Does not produce a rolling summary (history before the window is
/// discarded), so it trades fidelity for zero summariser cost and latency.
pub struct SlidingWindowCompaction {
    /// Fraction of the message budget to retain (newest-first).
    pub keep_ratio: f64,
}

impl Default for SlidingWindowCompaction {
    fn default() -> Self {
        Self {
            keep_ratio: SUMMARY_KEEP_RECENT_RATIO,
        }
    }
}

#[async_trait::async_trait]
impl CompactionStrategy for SlidingWindowCompaction {
    async fn compact(&self, req: CompactionRequest<'_>) -> CompactionResult {
        let total_budget = req.budget.total as usize;
        let static_overhead_tokens =
            crate::llm::estimate_request_overhead_tokens(Some(req.system_prompt), req.tool_defs);
        let message_budget = total_budget.saturating_sub(static_overhead_tokens);
        // Overflow recovery shrinks harder than the proactive pass.
        let ratio = match req.trigger {
            CompactionTrigger::Proactive => self.keep_ratio,
            CompactionTrigger::Overflow => self.keep_ratio * 0.5,
        };
        let keep_tokens = (message_budget as f64 * ratio) as usize;

        // Proactive path only acts once the configured micro tier is reached.
        if matches!(req.trigger, CompactionTrigger::Proactive) {
            let estimated = crate::llm::estimate_request_input_tokens(
                &req.messages,
                Some(req.system_prompt),
                req.tool_defs,
            );
            if estimated < req.budget.trigger_micro as usize {
                return CompactionResult {
                    changed: false,
                    messages: req.messages,
                    rolling_summary: req.rolling_summary.to_string(),
                    next_auto_compact_threshold: req.next_auto_compact_threshold,
                    ..Default::default()
                };
            }
        }

        let cut = select_recent_window(&req.messages, keep_tokens);
        if cut == 0 {
            return CompactionResult {
                changed: false,
                messages: req.messages,
                rolling_summary: req.rolling_summary.to_string(),
                next_auto_compact_threshold: req.next_auto_compact_threshold,
                ..Default::default()
            };
        }
        let kept = req.messages[cut..].to_vec();
        CompactionResult {
            changed: true,
            messages: kept,
            rolling_summary: req.rolling_summary.to_string(),
            next_auto_compact_threshold: req.next_auto_compact_threshold,
            ..Default::default()
        }
    }
}

/// Relevance-based retention. Keeps the newest window (like the sliding window)
/// and, from the messages that would otherwise be dropped, re-introduces the
/// top-k plain-text messages most relevant to the latest user turn — scored
/// with [`crate::memory::vector::cosine_similarity`] over a hashed
/// bag-of-words embedding. Tool-call messages are excluded from retrieval to
/// avoid orphaning tool_use/tool_result pairs.
pub struct VectorRetrievalCompaction {
    pub keep_ratio: f64,
    pub top_k: usize,
}

impl Default for VectorRetrievalCompaction {
    fn default() -> Self {
        Self {
            keep_ratio: SUMMARY_KEEP_RECENT_RATIO * 0.5,
            top_k: 4,
        }
    }
}

/// Hashed bag-of-words embedding so we can reuse cosine similarity without an
/// external embedding model. Deterministic and cheap; good enough for ranking.
fn bow_embed(text: &str, dims: usize) -> Vec<f32> {
    let mut v = vec![0f32; dims];
    for tok in text
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
    {
        let mut h: usize = 0;
        for b in tok.bytes() {
            h = h.wrapping_mul(31).wrapping_add(b as usize);
        }
        v[h % dims] += 1.0;
    }
    v
}

#[async_trait::async_trait]
impl CompactionStrategy for VectorRetrievalCompaction {
    async fn compact(&self, req: CompactionRequest<'_>) -> CompactionResult {
        let total_budget = req.budget.total as usize;
        let static_overhead_tokens =
            crate::llm::estimate_request_overhead_tokens(Some(req.system_prompt), req.tool_defs);
        let message_budget = total_budget.saturating_sub(static_overhead_tokens);

        if matches!(req.trigger, CompactionTrigger::Proactive) {
            let estimated = crate::llm::estimate_request_input_tokens(
                &req.messages,
                Some(req.system_prompt),
                req.tool_defs,
            );
            if estimated < req.budget.trigger_micro as usize {
                return CompactionResult {
                    changed: false,
                    messages: req.messages,
                    rolling_summary: req.rolling_summary.to_string(),
                    next_auto_compact_threshold: req.next_auto_compact_threshold,
                    ..Default::default()
                };
            }
        }

        let keep_tokens = (message_budget as f64 * self.keep_ratio) as usize;
        let cut = select_recent_window(&req.messages, keep_tokens);
        if cut == 0 {
            return CompactionResult {
                changed: false,
                messages: req.messages,
                rolling_summary: req.rolling_summary.to_string(),
                next_auto_compact_threshold: req.next_auto_compact_threshold,
                ..Default::default()
            };
        }

        // Latest user turn as the retrieval query.
        let query_text = req
            .messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .map(|m| m.content.as_text())
            .unwrap_or_default();

        const DIMS: usize = 256;
        let query_vec = bow_embed(&query_text, DIMS);

        // Rank dropped plain-text messages by similarity to the query.
        let mut scored: Vec<(usize, f32)> = req.messages[..cut]
            .iter()
            .enumerate()
            .filter(|(_, m)| is_plain_text_message(m))
            .map(|(idx, m)| {
                let cand = bow_embed(&m.content.as_text(), DIMS);
                (
                    idx,
                    crate::memory::vector::cosine_similarity(&query_vec, &cand),
                )
            })
            .filter(|(_, s)| *s > 0.0)
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(self.top_k);
        // Restore chronological order for the retained, retrieved messages.
        let mut keep_idx: Vec<usize> = scored.into_iter().map(|(i, _)| i).collect();
        keep_idx.sort_unstable();

        let mut kept: Vec<LlmMessage> = keep_idx.iter().map(|&i| req.messages[i].clone()).collect();
        kept.extend_from_slice(&req.messages[cut..]);

        CompactionResult {
            changed: true,
            messages: kept,
            rolling_summary: req.rolling_summary.to_string(),
            next_auto_compact_threshold: req.next_auto_compact_threshold,
            ..Default::default()
        }
    }
}

/// Resolve a named compaction strategy to a shared trait object.
///
/// `summarizer_prompt` is reserved for summary-based strategies (currently the
/// kernel summariser uses its own internal prompt; the override is accepted for
/// forward compatibility). Returns `None` for unknown names so callers can fail
/// loudly on configuration typos.
pub fn resolve_compaction_strategy(
    name: &str,
    summarizer_prompt: Option<String>,
) -> Option<Arc<dyn CompactionStrategy>> {
    let _ = summarizer_prompt;
    match name {
        "" | "default" | "summary_based" => Some(Arc::new(DefaultCompaction)),
        "sliding_window" => Some(Arc::new(SlidingWindowCompaction::default())),
        "vector_retrieval" => Some(Arc::new(VectorRetrievalCompaction::default())),
        // Fall through to host-registered contrib strategies so newly authored
        // compaction algorithms are selectable without editing this match.
        _ => crate::agent::contrib::resolve_compaction_strategy(name),
    }
}

/// Serialize a batch of `ContentBlock::ToolResult` blocks into the DB's
/// `tool_results_json` column.
///
/// The base shape is the existing `ContentBlock` JSON (so legacy readers still
/// work). When `tool_minimals` / `tool_names` are supplied, each entry is
/// augmented with a `content_minimal` and/or `tool_name` field keyed by
/// `tool_use_id`. The middle-tier read path in `commands/chat.rs` picks those
/// up to swap in the minimal receipt for older turns.
fn serialize_tool_results_with_receipts(
    tool_results: &[&ContentBlock],
    tool_minimals: Option<&HashMap<String, String>>,
    tool_names: Option<&HashMap<String, String>>,
) -> String {
    let mut value = match serde_json::to_value(tool_results) {
        Ok(v) => v,
        Err(_) => return String::new(),
    };
    if let serde_json::Value::Array(ref mut arr) = value {
        for entry in arr.iter_mut() {
            let tool_use_id = entry
                .get("tool_use_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            if let Some(id) = tool_use_id {
                if let Some(map) = tool_minimals {
                    if let Some(min) = map.get(&id) {
                        entry["content_minimal"] = serde_json::Value::String(min.clone());
                    }
                }
                if let Some(map) = tool_names {
                    if let Some(name) = map.get(&id) {
                        entry["tool_name"] = serde_json::Value::String(name.clone());
                    }
                }
            }
        }
    }
    serde_json::to_string(&value).unwrap_or_default()
}

fn is_compaction_summary_text(text: &str) -> bool {
    text.starts_with("[会话滚动摘要]") || text.starts_with("[对话摘要]")
}

/// Build the outgoing LLM request messages from the in-memory full `messages`,
/// swapping in rule-based minimal receipts for tool-results belonging to turns
/// older than `recent_full_turns`.
///
/// Invariants:
/// - `messages` is never mutated. The in-memory log is the authoritative
///   full-fidelity source for Level-2 summarisation.
/// - A "turn boundary" is a `user` message whose content is plain `Text` (not a
///   tool-result carrier), walking from newest to oldest. The final iteration
///   of a run always counts as one full turn even before the next user message
///   arrives.
/// - p5 **two-boundary scheme**: two *independent* cutoffs are computed —
///   one counting `recent_full_turns` user-text turns (the classic one), one
///   counting the last [`CTX_KEEP_RECENT_TOOL_CARRIERS`] messages that carry
///   tool-result blocks. The effective cutoff is `min(turn_cutoff,
///   tool_cutoff)` so whichever boundary preserves *more* history wins. The
///   cutoff is then snapped **backwards** so it never lands between an
///   assistant `tool_use` and its matching `tool_result` — demoting only the
///   result (or only the call) would break provider pairing invariants.
/// - If `tool_minimals` lacks an entry for a given `tool_use_id`, the original
///   full content is kept. Callers that need a hard ceiling on tokens should
///   follow up with `compact_trim_tool_results` on the returned vector.
pub fn build_request_messages(
    messages: &[LlmMessage],
    tool_minimals: &HashMap<String, String>,
    recent_full_turns: usize,
    recent_tool_carriers: usize,
) -> Vec<LlmMessage> {
    let messages = crate::agent::message_utils::sanitize_tool_use_result_pairing(
        crate::agent::message_utils::strip_ephemeral_tool_exchanges(messages.to_vec()),
    );
    let turn_cutoff = turn_based_recent_start(&messages, recent_full_turns);
    let tool_cutoff = tool_carrier_recent_start(&messages, recent_tool_carriers);
    // `min` = whichever boundary is *further back* (lower index) in the
    // message vector, i.e. preserves more messages at full fidelity.
    let mut recent_start = turn_cutoff.min(tool_cutoff);
    // Snap so we never split a tool_use / tool_result pair across the
    // boundary. The rule: walk back over any assistant message that is
    // purely a `ToolUse` carrier (its matching `ToolResult` will appear in
    // the preserved region). Equivalently, if `recent_start` currently
    // points at a user message containing only `ToolResult` blocks, step
    // back one more so the preceding assistant `ToolUse` comes with it.
    recent_start = snap_to_pair_boundary(&messages, recent_start);

    // Second pass: materialise the request vector. For indices below
    // `recent_start`, substitute `content` of each `ToolResult` block with its
    // minimal receipt from the side-map.
    let mut out: Vec<LlmMessage> = Vec::with_capacity(messages.len());
    for (i, msg) in messages.iter().enumerate() {
        if i >= recent_start {
            out.push(msg.clone());
            continue;
        }
        match &msg.content {
            MessageContent::Blocks(blocks) => {
                let swapped: Vec<ContentBlock> = blocks
                    .iter()
                    .map(|b| match b {
                        ContentBlock::ToolResult {
                            tool_use_id,
                            content: _,
                            is_error,
                        } => {
                            if let Some(min) = tool_minimals.get(tool_use_id) {
                                ContentBlock::ToolResult {
                                    tool_use_id: tool_use_id.clone(),
                                    content: crate::agent::tool_receipt::with_recall_hint(
                                        min,
                                        tool_use_id,
                                    ),
                                    is_error: *is_error,
                                }
                            } else {
                                b.clone()
                            }
                        }
                        _ => b.clone(),
                    })
                    .collect();
                out.push(LlmMessage {
                    role: msg.role.clone(),
                    content: MessageContent::Blocks(swapped),
                });
            }
            MessageContent::Text(_) => out.push(msg.clone()),
        }
    }
    out
}

/// Build the same request-view message slice the live agent uses before an
/// LLM call: demote older tool results to receipts, then hard-trim oversized
/// single results against both the current message budget and the configured
/// per-result cap. A zero configured cap disables per-result trimming.
pub fn build_request_view_messages(
    messages: &[LlmMessage],
    tool_minimals: &HashMap<String, String>,
    recent_full_turns: usize,
    recent_tool_carriers: usize,
    message_budget_tokens: usize,
    max_tool_result_tokens: u32,
) -> Vec<LlmMessage> {
    let mut out = build_request_messages(
        messages,
        tool_minimals,
        recent_full_turns,
        recent_tool_carriers,
    );
    if max_tool_result_tokens != 0 {
        let dynamic_limit = (message_budget_tokens as f64 * CONTEXT_SINGLE_RESULT_SHARE) as usize;
        let configured_limit = max_tool_result_tokens as usize;
        compact_trim_tool_results(&mut out, dynamic_limit.min(configured_limit));
    }
    collapse_repeated_tool_rounds(out)
}

/// One assistant tool round (a single tool call plus its single result) as
/// two adjacent messages, used to find identical consecutive rounds.
fn single_tool_round(messages: &[LlmMessage], i: usize) -> Option<(String, String)> {
    let (a, b) = (messages.get(i)?, messages.get(i + 1)?);
    if a.role != "assistant" || b.role != "user" {
        return None;
    }
    let (MessageContent::Blocks(ab), MessageContent::Blocks(bb)) = (&a.content, &b.content) else {
        return None;
    };
    let mut uses = ab.iter().filter_map(|x| match x {
        ContentBlock::ToolUse { id, name, input } => Some((id, name, input)),
        _ => None,
    });
    let (id, name, input) = uses.next()?;
    if uses.next().is_some() {
        return None;
    }
    let mut results = bb.iter().filter_map(|x| match x {
        ContentBlock::ToolResult { tool_use_id, .. } => Some(tool_use_id),
        _ => None,
    });
    if results.next()? != id || results.next().is_some() {
        return None;
    }
    Some((name.clone(), input.to_string()))
}

/// Some providers (e.g. Qwen on DashScope) reject a request with HTTP 400 when
/// the history holds the same tool call (same name and arguments) in several
/// consecutive rounds. Keep only the last round of every such run and note the
/// repetition in its result, so the model still learns it was repeating
/// itself. Operates on the request view only; stored history is untouched.
pub fn collapse_repeated_tool_rounds(messages: Vec<LlmMessage>) -> Vec<LlmMessage> {
    let mut drop = vec![false; messages.len()];
    let mut notes: HashMap<usize, usize> = HashMap::new();
    let mut i = 0;
    while i < messages.len() {
        let Some(sig) = single_tool_round(&messages, i) else {
            i += 1;
            continue;
        };
        let mut j = i + 2;
        while single_tool_round(&messages, j).as_ref() == Some(&sig) {
            j += 2;
        }
        let rounds = (j - i) / 2;
        if rounds >= 2 {
            drop[i..j - 2].fill(true);
            notes.insert(j - 1, rounds);
        }
        i = j;
    }
    messages
        .into_iter()
        .enumerate()
        .filter(|(idx, _)| !drop[*idx])
        .map(|(idx, mut m)| {
            if let (Some(n), MessageContent::Blocks(blocks)) = (notes.get(&idx), &mut m.content) {
                for b in blocks.iter_mut() {
                    if let ContentBlock::ToolResult { content, .. } = b {
                        content.push_str(&format!(
                            "\n[Note: this identical call was made {n} times in a row; earlier repeats were omitted. Change the arguments or approach instead of repeating it.]"
                        ));
                    }
                }
            }
            m
        })
        .collect()
}

/// Rebuild rule-based minimal receipts for complete tool exchanges already in
/// the hydrated history. AgentLoop starts a fresh side-map on every run, so
/// without this scan only tool calls executed during the current run can ever
/// be demoted in the request view.
fn rebuild_tool_receipts_from_messages(
    messages: &[LlmMessage],
) -> (HashMap<String, String>, HashMap<String, String>) {
    let mut calls: HashMap<String, (String, serde_json::Value)> = HashMap::new();
    let mut minimals = HashMap::new();
    let mut names = HashMap::new();

    for message in messages {
        let MessageContent::Blocks(blocks) = &message.content else {
            continue;
        };
        for block in blocks {
            match block {
                ContentBlock::ToolUse { id, name, input } => {
                    calls.insert(id.clone(), (name.clone(), input.clone()));
                    names.insert(id.clone(), name.clone());
                }
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } => {
                    let (name, input) = calls
                        .get(tool_use_id)
                        .map(|(name, input)| (name.as_str(), input))
                        .unwrap_or(("unknown", &serde_json::Value::Null));
                    if crate::agent::message_utils::is_ephemeral_tool_call(name, input) {
                        continue;
                    }
                    minimals.insert(
                        tool_use_id.clone(),
                        crate::agent::tool_receipt::render_receipt(
                            name, input, content, *is_error, None,
                        ),
                    );
                }
                ContentBlock::Text { .. } | ContentBlock::Image { .. } => {}
            }
        }
    }

    (minimals, names)
}

/// Turn-based boundary: index of the oldest message kept full when the
/// policy is "keep the last N user-text turns". Returns 0 when there are
/// fewer than N qualifying user-text boundaries (i.e. keep everything).
fn turn_based_recent_start(messages: &[LlmMessage], recent_full_turns: usize) -> usize {
    if recent_full_turns == 0 {
        return messages.len();
    }
    let mut recent_start: usize = 0;
    let mut turns_seen: usize = 0;
    for (i, msg) in messages.iter().enumerate().rev() {
        let is_user_text_boundary = msg.role == "user"
            && matches!(&msg.content, MessageContent::Text(t) if !t.is_empty() && !is_compaction_summary_text(t));
        if is_user_text_boundary {
            turns_seen += 1;
            if turns_seen == recent_full_turns {
                recent_start = i;
            } else if turns_seen > recent_full_turns {
                break;
            }
        }
    }
    recent_start
}

/// Tool-carrier boundary: index of the oldest message that still falls
/// within the most recent `keep` messages carrying `ToolResult` blocks.
/// Returns 0 when there are fewer than `keep` such carriers (i.e. keep
/// everything full).
fn tool_carrier_recent_start(messages: &[LlmMessage], keep: usize) -> usize {
    if keep == 0 {
        return messages.len();
    }
    let mut carriers_seen: usize = 0;
    for (i, msg) in messages.iter().enumerate().rev() {
        let has_tool_result = matches!(
            &msg.content,
            MessageContent::Blocks(blocks) if blocks.iter().any(|b| matches!(b, ContentBlock::ToolResult { .. }))
        );
        if has_tool_result {
            carriers_seen += 1;
            if carriers_seen >= keep {
                return i;
            }
        }
    }
    // Fewer than `keep` carriers exist anywhere — keep everything full.
    0
}

/// Snap a candidate cutoff *backwards* so it never sits between an
/// assistant `ToolUse` and its matching `ToolResult`. Called after the
/// two boundaries have been minned; safe no-op when the cutoff is
/// already at a pair boundary or at 0.
fn snap_to_pair_boundary(messages: &[LlmMessage], mut start: usize) -> usize {
    // Cheap short-circuits.
    if start == 0 || start >= messages.len() {
        return start;
    }
    // Step backwards while:
    //   (a) the message at `start` is a user message containing ONLY
    //       ToolResult blocks (i.e. it's a tool-result carrier, meaning
    //       the assistant's ToolUse lives at start-1), OR
    //   (b) the message at start-1 is an assistant message containing
    //       ToolUse blocks — keeping the pair together requires
    //       back-stepping across it.
    // Bound the walk to avoid pathological all-tool-result sequences.
    let mut guard = 0;
    while start > 0 && guard < 16 {
        let here = &messages[start];
        let starts_with_tool_result = is_tool_result_carrier(here);
        let prev_has_tool_use = matches!(
            &messages[start - 1].content,
            MessageContent::Blocks(blocks) if blocks.iter().any(|b| matches!(b, ContentBlock::ToolUse { .. }))
        );
        if starts_with_tool_result || prev_has_tool_use {
            start -= 1;
            guard += 1;
            continue;
        }
        break;
    }
    start
}

/// A provider tool-result carrier may also include image attachments emitted
/// by that tool. Treat the whole message as the result half of the pair.
fn is_tool_result_carrier(message: &LlmMessage) -> bool {
    message.role == "user"
        && matches!(
            &message.content,
            MessageContent::Blocks(blocks)
                if !blocks.is_empty()
                    && blocks.iter().any(|block| matches!(block, ContentBlock::ToolResult { .. }))
                    && blocks.iter().all(|block| matches!(block, ContentBlock::ToolResult { .. } | ContentBlock::Image { .. }))
        )
}

/// Pick the semantic-summary boundary while preserving an atomic
/// ToolUse/ToolResult pair in the recent tail.
fn summary_split_index(messages: &[LlmMessage], keep_tokens: usize) -> usize {
    let mut accumulated = 0usize;
    let mut split_idx = messages.len().saturating_sub(6);
    for (index, message) in messages.iter().enumerate().rev() {
        accumulated += crate::llm::estimate_message_tokens(message);
        if accumulated >= keep_tokens && index > 0 {
            split_idx = index;
            break;
        }
    }
    snap_to_pair_boundary(messages, split_idx)
}

/// Level-2 compaction: call LLM to summarise old messages, optionally merging
/// an existing rolling summary with the newly compacted history.
///
/// `keep_tokens` is the approximate token budget for the "recent tail" that
/// stays verbatim; everything older is fed to the summariser. Using tokens
/// (rather than the old char-based heuristic) prevents CJK-heavy sessions from
/// systematically under-keeping because 1 char is worth ~1 token in CJK but
/// ~0.25 tokens in English.
pub async fn compact_summarise(
    messages: Vec<LlmMessage>,
    keep_tokens: usize,
    client: &dyn crate::llm::LlmClient,
    model: &str,
    max_tokens: u32,
    existing_summary: Option<&str>,
) -> Option<CompactionOutcome> {
    if messages.len() < 2 {
        // Nothing meaningful to summarise if there are fewer than 2 messages.
        return None;
    }

    // Walk from the end, accumulating estimated tokens until we exceed
    // keep_tokens. Everything before the boundary index gets summarised.
    // We always keep at least the last 2 messages intact so the LLM has
    // immediate context regardless of how large they are.
    let split_idx = summary_split_index(&messages, keep_tokens);

    if split_idx == 0 {
        // All messages fit within keep_chars — nothing to summarise.
        return None;
    }

    let old_msgs = &messages[..split_idx];
    if old_msgs.is_empty() {
        return None;
    }

    let history_text: String = old_msgs
        .iter()
        .map(|m| {
            let role = if m.role == "user" {
                "用户/工具结果"
            } else {
                "智能体"
            };
            // as_text() returns empty for Blocks(ToolUse/ToolResult) — extract manually.
            let text = match &m.content {
                crate::llm::MessageContent::Text(t) => t.clone(),
                crate::llm::MessageContent::Blocks(blocks) => blocks
                    .iter()
                    .filter_map(|b| match b {
                        crate::llm::ContentBlock::Text { text } => {
                            if text.is_empty() {
                                None
                            } else {
                                Some(text.clone())
                            }
                        }
                        crate::llm::ContentBlock::ToolUse { name, input, .. } => {
                            let input_str = input.to_string();
                            let preview: String = input_str.chars().take(200).collect();
                            Some(format!("调用工具 {}: {}", name, preview))
                        }
                        crate::llm::ContentBlock::ToolResult { content, .. } => {
                            let preview: String = content.chars().take(200).collect();
                            Some(format!("工具结果: {}", preview))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            };
            if text.is_empty() || is_compaction_summary_text(&text) {
                return String::new();
            }
            let snippet = if text.chars().count() > 500 {
                format!("{}...", text.chars().take(500).collect::<String>())
            } else {
                text
            };
            format!("[{}]: {}", role, snippet)
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    if history_text.trim().is_empty() && existing_summary.unwrap_or("").trim().is_empty() {
        return None;
    }

    let existing_summary_block = existing_summary
        .map(str::trim)
        .filter(|summary| !summary.is_empty())
        .map(|summary| format!("已有滚动摘要：\n{}\n\n", summary))
        .unwrap_or_default();
    let summary_prompt = format!(
        "请将以下内容合并为一条新的滚动摘要。\n\
         摘要必须覆盖五部分：当前任务契约/用户目标、已完成工作、当前状态、未完成或待交接事项、关键文件/命令/结果。\n\
         必须保留仍然有效的任务目标、todo id、显式 handoff 目标（如 @Reviewer）、`[ProjectStatus]` 信号、阻塞原因、关键路径、错误和结论，省略重复中间步骤。\n\
         如果历史里出现了明确的下一位执行者、完成条件或待验证项，除非后续内容已经明确覆盖，否则不要丢失这些信息。\n\n\
         {}新近待压缩的对话历史：\n{}{}",
        existing_summary_block,
        history_text,
        crate::agent::summary_worker::STRUCTURED_SUMMARY_PROMPT_SUFFIX
    );

    let req = crate::llm::LlmRequest {
        messages: vec![crate::llm::LlmMessage {
            role: "user".into(),
            content: crate::llm::MessageContent::text(&summary_prompt),
        }],
        system: None,
        tools: vec![],
        model: model.to_string(),
        // Use at least 512 tokens for the summary regardless of the main model's
        // max_tokens setting, capped at 1024 to avoid wasting quota on a summary.
        max_tokens: max_tokens.clamp(512, 1024),
        stream: false,
        vision_override: Some(false),
    };

    match client.complete(req).await {
        Ok(resp) if !resp.content.is_empty() => {
            // p7: attempt structured JSON parse first; fall back to prose.
            let structured = crate::agent::summary_worker::parse_structured_summary(&resp.content);
            let merged_summary = if structured.summary.is_empty() {
                resp.content.trim().to_string()
            } else {
                structured.summary.clone()
            };
            let summary_msg = crate::agent::message_utils::rolling_summary_message(&merged_summary);
            let mut new_messages = vec![summary_msg];
            new_messages.extend_from_slice(&messages[split_idx..]);
            Some(CompactionOutcome {
                messages: new_messages,
                summary: merged_summary,
                input_tokens: resp.input_tokens,
                output_tokens: resp.output_tokens,
                structured_plan_items: structured.active_plan_items,
                structured_next_step_hint: structured.next_step_hint,
                structured_rolling: None,
            })
        }
        Ok(_) | Err(_) => None,
    }
}

/// Phase 2a: **Incremental / predictive-coding** Level-2 compaction.
///
/// Instead of re-summarising the entire old-message slab, this flow
/// feeds the LLM only
///
/// 1. the previous [`StructuredRollingSummary`] (prior),
/// 2. the "delta" = messages whose index is > `prev.last_msg_idx_covered`
///    (plus older uncovered messages on first run), and
/// 3. an optional `memory_snapshot` acting as a personalised codebook
///    (Phase 4b) — facts/decisions already in long-term memory don't
///    need to be re-summarised.
///
/// The LLM returns a list of [`MergeInstruction`]s and
/// [`apply_merge_instructions`] produces the new summary atomically —
/// if any step fails, the caller receives `None` and should keep the
/// previous summary unchanged (atomic rollback, FEC-style).
///
/// Benefits vs. the whole-history path:
/// - **O(|delta|)** input size instead of O(|history|) — up to 5–10×
///   latency reduction for long sessions.
/// - **Predictive coding**: only the residual is coded, satisfying
///   rate-distortion R(D) more tightly.
/// - **Memory-conditioned**: H(X | M) < H(X) reduces LLM work in
///   proportion to how much the agent already "knows".
///
/// Phase 6: The delta is first passed through `rule_preprocess` at
/// L2 aggressiveness to strip low-entropy noise before the LLM sees
/// it.
///
/// Returns `None` when:
/// - there is nothing new to summarise (delta empty), or
/// - the LLM call fails, or
/// - the returned merge instructions cannot be parsed / applied.
pub async fn compact_summarise_incremental(
    messages: Vec<LlmMessage>,
    keep_tokens: usize,
    client: &dyn crate::llm::LlmClient,
    model: &str,
    max_tokens: u32,
    prev: Option<&crate::agent::summary_worker::StructuredRollingSummary>,
    memory_snapshot: &[String],
) -> Option<CompactionOutcome> {
    use crate::agent::summary_worker as sw;

    if messages.len() < 2 {
        return None;
    }

    let split_idx = summary_split_index(&messages, keep_tokens);
    if split_idx == 0 {
        return None;
    }

    // Delta = messages [last_covered .. split_idx].
    let last_covered = prev.map(|p| p.last_msg_idx_covered).unwrap_or(0);
    let delta_start = last_covered.min(split_idx);
    let delta = &messages[delta_start..split_idx];
    if delta.is_empty() {
        return None;
    }

    // Phase 6: aggressive rule preprocessing on the LLM input only.
    // Never touches the real conversation vector.
    let pre_delta: Vec<LlmMessage> = crate::agent::rule_preprocess::preprocess_messages(
        delta,
        crate::agent::rule_preprocess::Level::L2,
    );

    let delta_text: String = pre_delta
        .iter()
        .map(format_message_for_summariser)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    if delta_text.trim().is_empty() {
        return None;
    }

    // Render the prior summary + memory snapshot as a "known" block
    // that the LLM must not re-emit. Dictionary coding.
    let prior_block = match prev {
        Some(p) if !p.is_empty() => format!(
            "【先验摘要（已知事实/决策/待办，不要重复）】\n{}\n\n",
            p.render_for_prompt(4_096)
        ),
        _ => String::new(),
    };
    let memory_block = if memory_snapshot.is_empty() {
        String::new()
    } else {
        let lines: String = memory_snapshot
            .iter()
            .take(40)
            .map(|s| format!("- {}", s))
            .collect::<Vec<_>>()
            .join("\n");
        format!("【长期记忆码本（已固化的事实，不要重复）】\n{}\n\n", lines)
    };

    let prompt = format!(
        "你正在维护一份结构化的会话滚动摘要。请阅读下面的先验 + 新增片段，\
         仅针对**新增信息**给出 merge 指令（JSON 数组）。\n\n\
         {}{}【新增对话片段】\n{}\n{}",
        prior_block,
        memory_block,
        delta_text,
        sw::INCREMENTAL_MERGE_PROMPT_SUFFIX
    );

    let req = crate::llm::LlmRequest {
        messages: vec![crate::llm::LlmMessage {
            role: "user".into(),
            content: crate::llm::MessageContent::text(&prompt),
        }],
        system: None,
        tools: vec![],
        model: model.to_string(),
        max_tokens: max_tokens.clamp(512, 1024),
        stream: false,
        vision_override: Some(false),
    };

    let resp = match client.complete(req).await {
        Ok(r) if !r.content.is_empty() => r,
        _ => return None,
    };

    let instructions = match sw::parse_merge_instructions(&resp.content) {
        Ok(v) => v,
        Err(_) => return None,
    };

    let prev_snapshot = prev.cloned().unwrap_or_default();
    let new_summary = match sw::apply_merge_instructions(&prev_snapshot, &instructions, split_idx) {
        Ok(s) => s,
        Err(_) => return None,
    };

    let rendered = new_summary.render_for_prompt(4_096);
    let summary_msg = crate::agent::message_utils::rolling_summary_message(&rendered);
    let mut new_messages = vec![summary_msg];
    new_messages.extend_from_slice(&messages[split_idx..]);

    let plan_items: Vec<String> = new_summary
        .open_items
        .iter()
        .map(|o| o.text.clone())
        .collect();

    Some(CompactionOutcome {
        messages: new_messages,
        summary: new_summary.to_prose(),
        input_tokens: resp.input_tokens,
        output_tokens: resp.output_tokens,
        structured_plan_items: plan_items,
        structured_next_step_hint: None,
        structured_rolling: Some(new_summary),
    })
}

/// Render a single message as a compact snippet for the summariser
/// prompt. Extracted so both legacy and incremental paths share
/// identical formatting.
fn format_message_for_summariser(m: &LlmMessage) -> String {
    let role = if m.role == "user" {
        "用户/工具结果"
    } else {
        "智能体"
    };
    let text = match &m.content {
        crate::llm::MessageContent::Text(t) => t.clone(),
        crate::llm::MessageContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(|b| match b {
                crate::llm::ContentBlock::Text { text } => (!text.is_empty()).then(|| text.clone()),
                crate::llm::ContentBlock::ToolUse { name, input, .. } => {
                    let input_str = input.to_string();
                    let preview: String = input_str.chars().take(200).collect();
                    Some(format!("调用工具 {}: {}", name, preview))
                }
                crate::llm::ContentBlock::ToolResult {
                    content,
                    tool_use_id,
                    ..
                } => {
                    let preview: String = content.chars().take(200).collect();
                    Some(format!("[{}] 工具结果: {}", tool_use_id, preview))
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
    };
    if text.is_empty() || is_compaction_summary_text(&text) {
        return String::new();
    }
    let snippet = if text.chars().count() > 500 {
        format!("{}...", text.chars().take(500).collect::<String>())
    } else {
        text
    };
    format!("[{}]: {}", role, snippet)
}

/// Returns true if the error message indicates a context overflow.
fn is_context_overflow_error(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    lower.contains("context length exceeded")
        || lower.contains("maximum context length")
        || lower.contains("prompt is too long")
        || lower.contains("exceeds model context window")
        || lower.contains("context_window_exceeded")
        || lower.contains("request_too_large")
        || lower.contains("上下文过长")
        || lower.contains("input is too long")
        || lower.contains("reduce the length")
}

/// Unified entry point for the per-iteration LLM call. When
/// `enable_streaming` is false, delegates to `LlmClient::complete` (the
/// historical single-response path). When true, drives `LlmClient::stream`,
/// forwards each text delta as an `AgentEvent::TextDelta`, and folds the
/// emitted chunks back into an `LlmResponse` so the caller's downstream
/// bookkeeping (token counters, tool-call extraction, persistence) stays
/// unchanged.
async fn llm_call_unified(
    client: &dyn LlmClient,
    req: crate::llm::LlmRequest,
    enable_streaming: bool,
    event_tx: &mpsc::Sender<AgentEvent>,
    partial_text: Option<Arc<Mutex<String>>>,
) -> Result<crate::llm::LlmResponse> {
    if !enable_streaming {
        return client.complete(req).await;
    }

    let (tx, mut rx) = mpsc::channel::<crate::llm::LlmChunk>(32);
    let mut text = String::new();
    let mut tool_calls: Vec<crate::llm::ToolCall> = Vec::new();
    let mut input_tokens = 0u32;
    let mut output_tokens = 0u32;
    let mut stream_error: Option<String> = None;

    let stream_fut = client.stream(req, tx);
    let recv_fut = async {
        while let Some(chunk) = rx.recv().await {
            match chunk {
                crate::llm::LlmChunk::TextDelta(delta) => {
                    text.push_str(&delta);
                    if let Some(ref partial) = partial_text {
                        partial.lock().await.push_str(&delta);
                    }
                    let _ = event_tx.send(AgentEvent::TextDelta { delta }).await;
                }
                crate::llm::LlmChunk::ToolUse { id, name, input } => {
                    tool_calls.push(crate::llm::ToolCall { id, name, input });
                }
                crate::llm::LlmChunk::Done {
                    input_tokens: it,
                    output_tokens: ot,
                } => {
                    input_tokens = it;
                    output_tokens = ot;
                }
                crate::llm::LlmChunk::Error(e) => {
                    stream_error = Some(e);
                }
            }
        }
    };

    let (stream_res, _) = tokio::join!(stream_fut, recv_fut);
    stream_res?;
    if let Some(err) = stream_error {
        return Err(anyhow::anyhow!(err));
    }

    Ok(crate::llm::LlmResponse {
        content: text,
        tool_calls,
        input_tokens,
        output_tokens,
    })
}

pub struct AgentLoop {
    pub client: Box<dyn LlmClient>,
    pub registry: Arc<ToolRegistry>,
    pub policy: Arc<PolicyGate>,
    pub system_prompt: String,
    pub model: String,
    pub max_tokens: u32,
    /// Input context window size in tokens (0 = auto, derived from max_tokens).
    /// Used for dynamic compaction budget calculation.
    pub context_window: u32,
    /// Model-derived safe input budget and adaptive compaction thresholds.
    pub budget: crate::agent::harness::LayeredBudget,
    /// Fallback models tried in order when the primary model fails with
    /// rate_limit / overloaded / model_not_found errors.
    pub fallback_models: Vec<String>,
    /// Optional database for audit logging
    pub db: Option<Arc<Mutex<Database>>>,
    /// Shared plan-state map (session_id -> plan todo items). Set when the
    /// agent is driven by a host that tracks execution plans (desktop).
    /// Replaces the previous `app_handle: Option<tauri::AppHandle>` coupling
    /// so the agent loop is portable to headless / CLI hosts.
    pub plan_state: Option<PlanStateHandle>,
    /// Shared map of pending permission confirmation channels
    pub confirmation_responses: Option<ConfirmationResponseMap>,
    /// User confirmation preferences from Settings (read on each tool call)
    pub confirm_flags: ConfirmFlagsHandle,
    /// User-configured vision override (from settings.vision_enabled).
    /// None = auto-detect from model name.
    pub vision_override: Option<bool>,
    /// Optional separate vision model client for image analysis delegation.
    /// When set, images from vision artifacts are sent to this client for
    /// analysis, and the resulting text description is injected instead.
    /// Used when vision_use_main_llm=false and a separate vision model is configured.
    pub vision_delegate: Option<Box<dyn crate::llm::LlmClient>>,
    /// The model id for the vision delegate. Used to populate the `model` field
    /// in vision analysis requests so the provider knows which model to route to.
    /// Empty when no separate vision model is configured.
    pub vision_model: String,
    /// Receives runtime notifications (e.g. @mention alerts) injected into the
    /// message stream so the agent can react mid-execution.
    pub notification_rx: Option<Mutex<mpsc::Receiver<String>>>,
    /// Automatically trigger rolling-summary compaction once cumulative input
    /// tokens reach this threshold. `0` disables threshold-driven compaction.
    pub auto_compact_input_tokens_threshold: u32,
    /// When true, main-loop LLM calls go through `LlmClient::stream` and
    /// text deltas are forwarded as they arrive. When false, calls go
    /// through `LlmClient::complete` and the full text is emitted once per
    /// turn.
    pub enable_streaming: bool,
    /// Optional host lifecycle hooks (tool before/after, context events).
    /// `None` for hosts that don't observe the loop (CLI, fish, tests).
    pub hooks: Option<Arc<dyn crate::agent::hooks::AgentHooks>>,
    /// Proactive context-compaction policy. Hosts may swap this; defaults to
    /// [`DefaultCompaction`] which reproduces the built-in behavior.
    pub compaction_strategy: Arc<dyn CompactionStrategy>,
    /// Optional pluggable long-term memory backend. When set, the loop retrieves
    /// relevant memories at run start and injects them into the system prompt
    /// (formatted by [`Self::memory_retrieval_prompt`]). `None` => no memory
    /// injection (behaviour-preserving default).
    pub memory_plugin: Option<Arc<dyn crate::memory::plugin::MemoryPlugin>>,
    /// Optional pluggable project-context discovery strategy. When set, the loop
    /// renders project instructions from the workspace root and prepends them to
    /// the system prompt. `None` => the host injects context by other means.
    pub context_manager: Option<Arc<dyn crate::context::ContextManager>>,
    /// Optional template used to format retrieved memories before they are
    /// appended to the system prompt. The literal `{memories}` placeholder is
    /// replaced with the rendered hit list; when absent the hits are appended
    /// after the template text. `None` uses a built-in default heading.
    pub memory_retrieval_prompt: Option<String>,
    /// Optional pluggable loop-control strategy (context transform, stop, and
    /// next-turn hooks). `None` uses the kernel's built-in ReAct control flow.
    pub loop_strategy: Option<Arc<dyn crate::agent::loop_strategy::LoopStrategy>>,
    /// Same-model transient retries (timeout / 502 / 503). DimRouter already
    /// fails over to another supplier of this id, so cloud-gateway hosts set
    /// this to 1. Direct official APIs keep the default of 3.
    pub same_model_transient_retries: u32,
}

impl AgentLoop {
    /// Execute a single tool call with policy checks, permission handling, timeout, audit logging.
    async fn execute_single_tool(
        &self,
        id: &str,
        name: &str,
        input: &serde_json::Value,
        ctx: &ToolContext,
        event_tx: &mpsc::Sender<AgentEvent>,
        cancel: &Arc<AtomicBool>,
    ) -> Vec<ContentBlock> {
        let span = tracing::info_span!("tool_exec", tool = %name, session_id = %ctx.session_id);
        info!(parent: &span, "executing tool");
        let trace_id = uuid::Uuid::new_v4().simple().to_string();
        let mut blocks = Vec::new();

        if let Some(wait_reason) = self.check_tool_rate_limit(ctx).await {
            let _ = event_tx
                .send(AgentEvent::ToolEnd {
                    id: id.to_string(),
                    name: name.to_string(),
                    result: wait_reason.clone(),
                    is_error: true,
                })
                .await;
            blocks.push(ContentBlock::ToolResult {
                tool_use_id: id.to_string(),
                content: wait_reason,
                is_error: true,
            });
            return blocks;
        }

        // Policy check
        let decision = self.policy.check_tool_call(name, input);
        match &decision {
            PolicyDecision::Deny(reason) => {
                warn!("Tool '{}' denied by policy: {}", name, reason);
                let _ = event_tx
                    .send(AgentEvent::ToolEnd {
                        id: id.to_string(),
                        name: name.to_string(),
                        result: format!("Denied by policy: {}", reason),
                        is_error: true,
                    })
                    .await;
                blocks.push(ContentBlock::ToolResult {
                    tool_use_id: id.to_string(),
                    content: format!("Error: {}", reason),
                    is_error: true,
                });
                return blocks;
            }
            PolicyDecision::Warn(msg) => {
                let tool_wants_confirm = self
                    .registry
                    .get(name)
                    .map(|t| t.needs_confirmation(input))
                    .unwrap_or(false);
                let (confirm_shell, confirm_file_write) = self
                    .confirm_flags
                    .read()
                    .map(|f| (f.confirm_shell, f.confirm_file_write))
                    .unwrap_or((true, true));
                let user_disabled = match name {
                    "shell" | "bash" | "powershell" | "powershell_query" => !confirm_shell,
                    "file_write" | "file_edit" => !confirm_file_write,
                    _ => false,
                };
                if tool_wants_confirm && !user_disabled {
                    if let Some(confirms) = &self.confirmation_responses {
                        let request_id = uuid::Uuid::new_v4().to_string();
                        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
                        {
                            confirms.lock().await.insert(request_id.clone(), resp_tx);
                        }
                        let _ = event_tx
                            .send(AgentEvent::PermissionRequest {
                                request_id,
                                tool_name: name.to_string(),
                                tool_input: input.clone(),
                                description: msg.clone(),
                            })
                            .await;
                        let cancel_for_perm = Arc::clone(cancel);
                        let approved = tokio::select! {
                            biased;
                            // User cancelled the whole run while waiting for permission
                            _ = async {
                                loop {
                                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                                    if cancel_for_perm.load(Ordering::Relaxed) { break; }
                                }
                            } => false,
                            // 60-second timeout waiting for the user to click approve/deny
                            result = tokio::time::timeout(
                                std::time::Duration::from_secs(60),
                                resp_rx,
                            ) => matches!(result, Ok(Ok(true))),
                        };
                        if approved {
                            debug!("User approved tool '{}' execution", name);
                        } else {
                            let reason = if cancel.load(Ordering::Relaxed) {
                                "已被用户取消"
                            } else {
                                "User denied this operation"
                            };
                            warn!("Tool '{}' denied/cancelled: {}", name, reason);
                            let _ = event_tx
                                .send(AgentEvent::ToolEnd {
                                    id: id.to_string(),
                                    name: name.to_string(),
                                    result: reason.into(),
                                    is_error: true,
                                })
                                .await;
                            blocks.push(ContentBlock::ToolResult {
                                tool_use_id: id.to_string(),
                                content: reason.into(),
                                is_error: true,
                            });
                            return blocks;
                        }
                    }
                } else {
                    warn!("Tool '{}' policy warning: {}", name, msg);
                }
            }
            PolicyDecision::Allow => {}
        }

        let mut input_with_trace = input.clone();
        if let Some(obj) = input_with_trace.as_object_mut() {
            obj.insert(
                "_trace_id".into(),
                serde_json::Value::String(trace_id.clone()),
            );
        }
        let _ = event_tx
            .send(AgentEvent::ToolStart {
                id: id.to_string(),
                name: name.to_string(),
                input: input_with_trace,
            })
            .await;

        // Lifecycle hook: before_tool. Lets a host capture pre-state (e.g.
        // snapshot a file before it is overwritten) or deny the call.
        let hook_event = crate::agent::hooks::ToolHookEvent {
            session_id: &ctx.session_id,
            tool_use_id: id,
            tool_name: name,
            input,
            workspace_root: &ctx.workspace_root,
        };
        let deny_reason = if let Some(hooks) = &self.hooks {
            match hooks.before_tool(&hook_event).await {
                crate::agent::hooks::HookDecision::Deny(r) => Some(r),
                crate::agent::hooks::HookDecision::Continue => None,
            }
        } else {
            None
        };

        let mut schema_correction_envelope: Option<String> = None;
        let result = if let Some(reason) = deny_reason {
            warn!("Tool '{}' denied by host hook: {}", name, reason);
            super::tool::ToolResult::err(reason)
        } else {
            match self.registry.get(name) {
                Some(tool) => {
                    // Log key input fields to aid debugging (path, command, query, etc.)
                    let input_hint = match name {
                        "file_read" | "file_write" => {
                            input["path"].as_str().unwrap_or("?").to_string()
                        }
                        "shell" => format!(
                            "[{}] {}",
                            input["interpreter"].as_str().unwrap_or("powershell"),
                            input["command"]
                                .as_str()
                                .unwrap_or("?")
                                .chars()
                                .take(100)
                                .collect::<String>()
                        ),
                        "powershell_query" => format!(
                            "query={} arch={}",
                            input["query"].as_str().unwrap_or("?"),
                            input["arch"].as_str().unwrap_or("x64")
                        ),
                        "web_search" => input["query"]
                            .as_str()
                            .unwrap_or("?")
                            .chars()
                            .take(80)
                            .collect(),
                        "browser" => format!(
                            "action={} url={}",
                            input["action"].as_str().unwrap_or("?"),
                            input["url"].as_str().unwrap_or("")
                        ),
                        "com_invoke" => format!(
                            "action={} prog_id={} arch={}",
                            input["action"].as_str().unwrap_or("?"),
                            input["prog_id"].as_str().unwrap_or("?"),
                            input["arch"].as_str().unwrap_or("x64")
                        ),
                        "wmi" => format!(
                            "preset={} query={}",
                            input["preset"].as_str().unwrap_or(""),
                            input["query"]
                                .as_str()
                                .unwrap_or("?")
                                .chars()
                                .take(80)
                                .collect::<String>()
                        ),
                        "uia" => format!(
                            "action={} name={} window={}",
                            input["action"].as_str().unwrap_or("?"),
                            input["name"].as_str().unwrap_or(""),
                            input["window_title"].as_str().unwrap_or("")
                        ),
                        _ => input.to_string().chars().take(100).collect(),
                    };
                    // Check cancel before starting the tool
                    if cancel.load(Ordering::Relaxed) {
                        let _ = event_tx
                            .send(AgentEvent::ToolEnd {
                                id: id.to_string(),
                                name: name.to_string(),
                                result: "已取消".into(),
                                is_error: true,
                            })
                            .await;
                        blocks.push(ContentBlock::ToolResult {
                            tool_use_id: id.to_string(),
                            content: "已取消".into(),
                            is_error: true,
                        });
                        return blocks;
                    }

                    debug!("Executing tool: {} | input: {}", name, input_hint);
                    let mut tool_ctx = ctx.clone();
                    tool_ctx.tool_use_id = Some(id.to_string());
                    let cancel_clone = Arc::clone(cancel);
                    // Poll cancel flag every 200 ms while the tool runs
                    let cancel_watcher = async move {
                        loop {
                            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                            if cancel_clone.load(Ordering::Relaxed) {
                                break;
                            }
                        }
                    };
                    // Owned-completion tools (shell, code_run) observe `ctx.cancel` themselves and
                    // terminate their own process tree. Dropping the future at the first watcher poll
                    // raced that teardown and misreported the outcome as interrupted. Instead, after
                    // a cancel request the tool gets a bounded grace period; only then is it dropped.
                    let tool_timeout_secs = tool_timeout_for(&input);
                    let mut tool_future = Box::pin(tokio::time::timeout(
                        std::time::Duration::from_secs(tool_timeout_secs),
                        call_with_completion(tool, input.clone(), &tool_ctx),
                    ));
                    let outcome = tokio::select! {
                        biased;
                        res = &mut tool_future => Some(res),
                        _ = cancel_watcher => {
                            warn!(
                                "Tool '{}' cancel requested; allowing {}s for its own teardown",
                                name, CANCEL_GRACE_SECS
                            );
                            tokio::time::timeout(
                                std::time::Duration::from_secs(CANCEL_GRACE_SECS),
                                &mut tool_future,
                            )
                            .await
                            .ok()
                        }
                    };
                    match outcome {
                        Some(Ok(Ok(r))) => r,
                        Some(Ok(Err(e))) => {
                            let err_msg = e.to_string();
                            warn!("Tool '{}' error: {} | input: {}", name, err_msg, input_hint);
                            schema_correction_envelope = maybe_schema_correction_envelope(
                                &self.registry,
                                name,
                                &err_msg,
                            );
                            let friendly = friendly_tool_error(name, &err_msg);
                            super::tool::ToolResult::err(friendly)
                        }
                        Some(Err(_)) => {
                            warn!("Tool '{}' timed out after {}s", name, tool_timeout_secs);
                            super::tool::ToolResult::err(format!(
                                "工具 '{}' 执行超时（{}秒）。可能原因：命令阻塞、网络超时或进程挂起。请尝试简化命令或分步执行。",
                                name, tool_timeout_secs
                            ))
                        }
                        None => {
                            warn!("Tool '{}' interrupted by user cancel", name);
                            super::tool::ToolResult::err("已被用户取消".to_string())
                        }
                    }
                }
                None => {
                    warn!("Tool '{}' not found in registry", name);
                    let available: Vec<String> = self
                        .registry
                        .all()
                        .iter()
                        .map(|t| t.name().to_string())
                        .collect();
                    super::tool::ToolResult::err(format!(
                        "Tool '{}' does not exist. Available tools: {}.",
                        name,
                        available.join(", ")
                    ))
                }
            }
        };

        // Lifecycle hook: after_tool. Observe the result (journaling,
        // telemetry, post-edit checks). Runs before the result is emitted.
        if let Some(hooks) = &self.hooks {
            hooks.after_tool(&hook_event, &result).await;
        }

        let mut final_result_content =
            decorate_tool_failure_for_agent(name, input, &result.content, result.is_error);
        if let Some(envelope) = schema_correction_envelope {
            if !final_result_content.is_empty() {
                final_result_content.push_str("\n\n");
            }
            final_result_content.push_str(&envelope);
        }
        let end_result = format!("[trace_id:{}] {}", trace_id, final_result_content);
        let _ = event_tx
            .send(AgentEvent::ToolEnd {
                id: id.to_string(),
                name: name.to_string(),
                result: end_result,
                is_error: result.is_error,
            })
            .await;

        if let Some(ref db_arc) = self.db {
            let action = format!("{} [trace:{}]", audit_action_label(name, input), trace_id);
            let redacted_input = self.policy.redact_text(&summarize_tool_input(name, input));
            let redacted_result = self.policy.redact_text(&final_result_content);
            let input_summary = Some(truncate_str(&redacted_input, 300));
            let result_summary = Some(truncate_str(&redacted_result, 200));
            let is_err = result.is_error;
            let tool_name_clone = name.to_string();
            let session_id_clone = ctx.session_id.clone();
            let db_clone = db_arc.clone();
            tokio::spawn(async move {
                let db = db_clone.lock().await;
                let _ = db.append_audit(
                    &session_id_clone,
                    &tool_name_clone,
                    &action,
                    input_summary.as_deref(),
                    result_summary.as_deref(),
                    is_err,
                );
            });
        }

        let mut guarded_content = guard_tool_result_content(
            &final_result_content,
            dynamic_result_limit(
                crate::llm::compute_total_input_budget(self.context_window, self.max_tokens)
                    .saturating_sub(crate::llm::estimate_request_overhead_tokens(
                        Some(&self.system_prompt),
                        &self
                            .registry
                            .to_tool_defs(crate::agent::tool::ToolDefMode::Minimal),
                    )),
            ),
        );
        if let Some(img) = result.image.as_ref() {
            let artifact = vision::store_tool_image(&ctx.session_id, name, None, img).await;
            guarded_content.push_str(&format!(
                "\n\n[vision_artifact] id={} label=\"{}\" media_type={}\nUse vision_context to list/select reusable images for a later reasoning step.",
                artifact.id, artifact.label, artifact.media_type
            ));
        }
        blocks.push(ContentBlock::ToolResult {
            tool_use_id: id.to_string(),
            content: guarded_content,
            is_error: result.is_error,
        });
        if let Some(img) = result.image {
            blocks.push(ContentBlock::Image {
                source: ImageSource {
                    source_type: "base64".into(),
                    media_type: img.media_type,
                    data: img.base64,
                },
            });
        }
        blocks
    }

    /// Delegate image analysis to a separate vision model.
    /// Finds the last user message with image blocks, sends them to the vision
    /// model for analysis, and replaces image blocks with text descriptions.
    async fn delegate_vision_analysis(
        messages: &[crate::llm::LlmMessage],
        vision_client: &dyn crate::llm::LlmClient,
        vision_model: &str,
    ) -> Vec<crate::llm::LlmMessage> {
        use crate::llm::{ContentBlock, LlmMessage, LlmRequest, MessageContent};

        // Find the last user message that contains image blocks
        let last_vision_idx = messages.iter().rposition(|m| {
            m.role == "user" && matches!(&m.content, MessageContent::Blocks(blocks) if blocks.iter().any(|b| matches!(b, ContentBlock::Image { .. })))
        });

        let Some(idx) = last_vision_idx else {
            return messages.to_vec();
        };

        let msg = &messages[idx];
        let MessageContent::Blocks(blocks) = &msg.content else {
            return messages.to_vec();
        };

        // Extract image blocks and text blocks
        let images: Vec<&ContentBlock> = blocks
            .iter()
            .filter(|b| matches!(b, ContentBlock::Image { .. }))
            .collect();
        let text_parts: Vec<String> = blocks
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.clone()),
                _ => None,
            })
            .collect();

        if images.is_empty() {
            return messages.to_vec();
        }

        const VISION_INSTRUCTION: &str = concat!(
            "Describe what you see in this image in detail. ",
            "Focus on UI elements, text content, layout, and any actionable information. ",
            "Respond in the same language as the original text.\n\n",
            "## CRITICAL RULES — Anti-hallucination\n",
            "1. If the image is completely black, blank, corrupted, or unreadable, respond ONLY with: ",
            "\"[无法识别: 图片为全黑/空白/损坏，无法获取有效视觉信息]\"\n",
            "2. NEVER fabricate, guess, or invent visual content that is not clearly visible in the image.\n",
            "3. If the requested task (e.g. \"find the button\") cannot be completed from what is visible, ",
            "explicitly state: \"[无法完成任务: <reason>]\" and explain why.\n",
            "4. Only describe elements you can CONFIDENTLY identify in the image. When uncertain about an element, ",
            "use hedging language (\"appears to be\", \"may be\") rather than asserting it as fact.\n",
            "5. Under no circumstances pretend you can see content that is not there.",
        );
        const VISION_SYSTEM: &str =
            "You are a visual analysis assistant. Your ONLY job is to describe images ACCURATELY. \
             You MUST NEVER fabricate, hallucinate, or invent any visual content. \
             If an image is blank, black, corrupted, or otherwise unreadable, say so honestly. \
             If a user's question cannot be answered from the visible content, say it cannot be determined. \
             Honesty and accuracy are absolute priorities over completeness.";

        tracing::info!(
            "vision_delegate: analyzing {} image(s) via separate vision model (one request per image)",
            images.len()
        );

        let mut analysis_sections: Vec<String> = Vec::new();
        for (image_idx, img) in images.iter().enumerate() {
            let vision_blocks = vec![
                ContentBlock::Text {
                    text: VISION_INSTRUCTION.to_string(),
                },
                (*img).clone(),
            ];
            let vision_req = LlmRequest {
                messages: vec![LlmMessage {
                    role: "user".into(),
                    content: MessageContent::Blocks(vision_blocks),
                }],
                system: Some(VISION_SYSTEM.to_string()),
                tools: vec![],
                model: vision_model.to_string(),
                max_tokens: 2048,
                stream: false,
                vision_override: Some(true),
            };

            match vision_client.complete(vision_req).await {
                Ok(resp) if !resp.content.is_empty() => {
                    tracing::info!(
                        "vision_delegate: image {} got {} chars description",
                        image_idx + 1,
                        resp.content.len()
                    );
                    analysis_sections.push(resp.content.trim().to_string());
                }
                Ok(_) => {
                    tracing::warn!(
                        "vision_delegate: empty response for image {}",
                        image_idx + 1
                    );
                    analysis_sections.push(
                        "[视觉模型分析失败: 视觉模型返回了空响应，未能获取任何视觉信息]"
                            .to_string(),
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "vision_delegate: vision model failed for image {}: {}",
                        image_idx + 1,
                        e
                    );
                    analysis_sections.push(format!("[视觉模型分析失败: {}]", e));
                }
            }
        }

        let mut result = messages.to_vec();
        let mut new_blocks: Vec<ContentBlock> = Vec::new();
        for t in &text_parts {
            new_blocks.push(ContentBlock::Text { text: t.clone() });
        }
        for section in analysis_sections {
            new_blocks.push(ContentBlock::Text {
                text: format!("\n[视觉模型分析结果]\n{section}"),
            });
        }
        result[idx] = LlmMessage {
            role: "user".into(),
            content: MessageContent::Blocks(new_blocks),
        };
        result
    }

    async fn check_tool_rate_limit(&self, ctx: &ToolContext) -> Option<String> {
        let limit = self.policy.tool_rate_limit_per_minute as usize;
        if limit == 0 {
            return None;
        }
        let now = std::time::Instant::now();
        let mut state = TOOL_RATE_STATE.lock().await;
        let entries = state.entry(ctx.session_id.clone()).or_default();
        entries.retain(|t| now.duration_since(*t).as_secs() < 60);
        if entries.len() >= limit {
            return Some(format!(
                "Tool rate limit exceeded for session '{}' ({} calls/min)",
                ctx.session_id, limit
            ));
        }
        entries.push(now);
        None
    }

    /// Run the agent loop for a single user turn.
    ///
    /// Sends `AgentEvent`s through `event_tx` for streaming to the frontend.
    /// Returns `(final_messages, input_tokens, output_tokens)` when the LLM produces
    /// a final response with no tool calls, when `cancel` is set, or after MAX_ITERATIONS.
    /// Write a single LlmMessage to the database immediately (real-time persistence).
    /// Called after every new assistant/tool message is appended during the agent loop,
    /// so messages survive even if the process is killed mid-run.
    async fn persist_message(&self, session_id: &str, msg: &LlmMessage, turn_index: Option<i64>) {
        self.persist_message_with_receipts(session_id, msg, turn_index, None, None)
            .await;
    }

    /// Variant of `persist_message` that also writes dual-version tool results.
    ///
    /// `tool_minimals` maps `tool_use_id → minimal receipt` and `tool_names` maps
    /// `tool_use_id → tool name`. When both are provided (typically only for the
    /// tool-result-carrier message), the serialized `tool_results_json` gains a
    /// `content_minimal` and `tool_name` field per entry, which the read path
    /// consumes to swap in the minimal form for older turns.
    async fn persist_message_with_receipts(
        &self,
        session_id: &str,
        msg: &LlmMessage,
        turn_index: Option<i64>,
        tool_minimals: Option<&HashMap<String, String>>,
        tool_names: Option<&HashMap<String, String>>,
    ) {
        let Some(ref db_arc) = self.db else { return };
        let db = db_arc.lock().await;
        use crate::llm::{ContentBlock, MessageContent};
        match &msg.content {
            MessageContent::Blocks(blocks) => {
                let tool_uses: Vec<&ContentBlock> = blocks
                    .iter()
                    .filter(|b| matches!(b, ContentBlock::ToolUse { .. }))
                    .collect();
                let tool_results: Vec<&ContentBlock> = blocks
                    .iter()
                    .filter(|b| matches!(b, ContentBlock::ToolResult { .. }))
                    .collect();
                if !tool_uses.is_empty() {
                    let raw_text = msg.content.as_text();
                    let text = crate::agent::message_utils::strip_send_markers(&raw_text);
                    let calls_json = serde_json::to_string(&tool_uses).unwrap_or_default();
                    let _ = db.append_message_full(
                        session_id,
                        "assistant",
                        &text,
                        Some(&calls_json),
                        None,
                        turn_index,
                    );
                } else if !tool_results.is_empty() {
                    let results_json = serialize_tool_results_with_receipts(
                        &tool_results,
                        tool_minimals,
                        tool_names,
                    );
                    let _ = db.append_message_full(
                        session_id,
                        "user",
                        "",
                        None,
                        Some(&results_json),
                        turn_index,
                    );
                } else {
                    let text = msg.content.as_text();
                    if !text.is_empty() {
                        let _ = db.append_message_full(
                            session_id, &msg.role, &text, None, None, turn_index,
                        );
                    }
                }
            }
            MessageContent::Text(text) => {
                if !text.is_empty() {
                    let clean = if msg.role == "assistant" {
                        crate::agent::message_utils::strip_send_markers(text).into_owned()
                    } else {
                        text.clone()
                    };
                    if !clean.is_empty() {
                        let _ = db.append_message_full(
                            session_id, &msg.role, &clean, None, None, turn_index,
                        );
                    }
                }
            }
        }
    }

    /// Compose the effective system prompt for this run.
    ///
    /// Layers, in order: discovered project context (when a
    /// [`context_manager`](Self::context_manager) is wired), the base system
    /// prompt, then retrieved long-term memories (when a
    /// [`memory_plugin`](Self::memory_plugin) is wired). When neither optional
    /// plugin is set this returns [`Self::system_prompt`] verbatim, so the
    /// default code path is byte-for-byte unchanged.
    fn compose_system_prompt(&self, messages: &[LlmMessage], ctx: &ToolContext) -> String {
        if self.context_manager.is_none() && self.memory_plugin.is_none() {
            return self.system_prompt.clone();
        }

        let mut prompt = String::new();

        // L_context: project instructions discovered from the workspace root.
        if let Some(mgr) = &self.context_manager {
            if let Ok(rendered) = mgr.render(
                &ctx.workspace_root,
                crate::context::DEFAULT_CONTEXT_BUDGET_CHARS,
            ) {
                if !rendered.trim().is_empty() {
                    prompt.push_str(rendered.trim_end());
                    prompt.push_str("\n\n");
                }
            }
        }

        prompt.push_str(&self.system_prompt);

        // L_memory: retrieve relevant memories keyed off the latest user turn.
        if let Some(mem) = &self.memory_plugin {
            let query_text = messages
                .iter()
                .rev()
                .find(|m| m.role == "user")
                .map(|m| m.content.as_text())
                .unwrap_or_default();
            if !query_text.trim().is_empty() {
                let query = crate::memory::plugin::MemoryQuery {
                    text: Some(query_text),
                    limit: 5,
                    ..Default::default()
                };
                if let Ok(hits) = mem.search(&query) {
                    if !hits.is_empty() {
                        let rendered: String = hits
                            .iter()
                            .map(|h| format!("- {}", h.entry.content))
                            .collect::<Vec<_>>()
                            .join("\n");
                        let block = match &self.memory_retrieval_prompt {
                            Some(tpl) if tpl.contains("{memories}") => {
                                tpl.replace("{memories}", &rendered)
                            }
                            Some(tpl) => format!("{tpl}\n{rendered}"),
                            None => format!("## Relevant Memories\n{rendered}"),
                        };
                        prompt.push_str("\n\n");
                        prompt.push_str(&block);
                    }
                }
            }
        }

        prompt
    }

    ///
    /// NOTE: The caller is responsible for emitting `AgentEvent::Done` AFTER persisting
    /// the result to the database, to avoid a race condition where the frontend reloads
    /// messages before the DB write completes.
    pub async fn run(
        &self,
        mut messages: Vec<LlmMessage>,
        event_tx: mpsc::Sender<AgentEvent>,
        cancel: Arc<AtomicBool>,
        ctx: ToolContext,
    ) -> Result<(Vec<LlmMessage>, u32, u32)> {
        let span =
            tracing::info_span!("agent_loop", session_id = %ctx.session_id, model = %self.model);
        let _enter = span.enter();
        drop(_enter); // Don't hold across awaits — use span for structured correlation only
        info!(parent: &span, "agent loop starting");
        let mut total_input = 0u32;
        let mut total_output = 0u32;
        // Accumulate new messages produced during this run in a separate buffer.
        // This is immune to compaction: compaction only modifies `messages` (the LLM context
        // window), but new_messages always grows monotonically with every new assistant/tool
        // message. The caller persists new_messages to the DB.
        let mut new_messages: Vec<LlmMessage> = Vec::new();
        // Dual-version tool-result side-maps (Phase C). Keyed by `tool_use_id`.
        // They are seeded from the hydrated history after checkpoint restore,
        // then extended with results produced during this run.
        // Determine the turn_index for this run once, so all messages share the same index.
        // This must be computed before any messages are written.
        let turn_index: Option<i64> = if let Some(ref db_arc) = self.db {
            let db = db_arc.lock().await;
            let idx = db
                .get_messages_latest(&ctx.session_id, 2000)
                .map(|msgs| {
                    let max_turn = msgs.iter().filter_map(|m| m.turn_index).max().unwrap_or(0);
                    max_turn + 1
                })
                .unwrap_or(1);
            Some(idx)
        } else {
            None
        };
        let base_context_hash = stable_hash_messages(&messages);
        let base_message_count = messages.len();
        let mut restored_loop_history: Option<Vec<ToolCallRecord>> = None;
        let mut restored_seen_notifications: Option<HashSet<String>> = None;

        // Check for a resumable checkpoint from a previous (crashed) run
        if let Some(ref db_arc) = self.db {
            let db = db_arc.lock().await;
            match db.load_checkpoint(&ctx.session_id) {
                Ok(Some((iter, json))) => {
                    match serde_json::from_str::<AgentCheckpointPayload>(&json) {
                        Ok(payload)
                            if !payload.messages.is_empty()
                                && payload.base_context_hash == base_context_hash
                                && payload.base_message_count == base_message_count =>
                        {
                            info!(
                                "Resuming from checkpoint at iteration {} for session {}",
                                iter, ctx.session_id
                            );
                            restored_loop_history = Some(payload.loop_history.clone());
                            restored_seen_notifications =
                                Some(payload.seen_notifications.into_iter().collect());
                            messages = payload.messages;
                            info!("Checkpoint restored: {} messages", messages.len());
                            let _ = db.finish_checkpoint(&ctx.session_id, "resumed");
                        }
                        Ok(payload) => {
                            warn!(
                                "Checkpoint stale for session {} (base hash/count mismatch: {}:{}, current {}:{}); ignoring",
                                ctx.session_id,
                                payload.base_context_hash,
                                payload.base_message_count,
                                base_context_hash,
                                base_message_count
                            );
                            let _ = db.finish_checkpoint(&ctx.session_id, "stale");
                        }
                        Err(_) => {
                            warn!("Checkpoint JSON invalid; clearing and starting from scratch");
                            let _ = db.finish_checkpoint(&ctx.session_id, "invalid");
                        }
                    }
                }
                Ok(None) => {}
                Err(e) => warn!("Could not load checkpoint: {}", e),
            }
        }

        let (mut tool_minimals, mut tool_names_by_id) =
            rebuild_tool_receipts_from_messages(&messages);

        // Compose the effective system prompt once per run, folding in any
        // wired context-manager / memory-plugin output. Falls back to the base
        // prompt verbatim when neither is set (behaviour-preserving default).
        let effective_system_prompt = self.compose_system_prompt(&messages, &ctx);

        let max_iterations = ctx.max_iterations.unwrap_or(DEFAULT_MAX_ITERATIONS as u32) as usize;
        let mut loop_detector = LoopDetectorState {
            history: restored_loop_history.clone().unwrap_or_default(),
            consecutive_vision_only_turns: 0,
            warned: HashMap::new(),
        };
        if restored_loop_history.is_none() {
            loop_detector.seed_from_messages(&messages);
        }
        let mut rolling_summary = String::new();
        let mut seen_notifications: HashSet<String> =
            restored_seen_notifications.unwrap_or_default();
        // Guard: limit how many times we re-prompt for unfinished todos.
        // After this many consecutive text-only (no tool call) responses with
        // unfinished todos, stop injecting reminders and exit gracefully.
        const TODO_REMINDER_MAX: usize = 2;
        let mut todo_reminder_count: usize = 0;
        let mut rolling_summary_version = 0i64;
        let mut cumulative_input_tokens = 0i64;
        let mut cumulative_output_tokens = 0i64;
        if let Some(ref db_arc) = self.db {
            let db = db_arc.lock().await;
            match db.get_session_context_state(&ctx.session_id) {
                Ok(Some(state)) => {
                    rolling_summary = state.rolling_summary;
                    rolling_summary_version = state.rolling_summary_version;
                    cumulative_input_tokens = state.total_input_tokens;
                    cumulative_output_tokens = state.total_output_tokens;
                }
                Ok(None) => {}
                Err(error) => warn!("Failed to load session context state: {}", error),
            }
        }
        let threshold_step = i64::from(self.auto_compact_input_tokens_threshold);
        let mut next_auto_compact_threshold = if threshold_step > 0 {
            ((cumulative_input_tokens / threshold_step) + 1) * threshold_step
        } else {
            i64::MAX
        };

        // Loop-strategy turn tracking (fed into `prepare_next_turn`).
        let mut last_turn_had_tool_calls = false;
        let mut last_turn_text = String::new();

        'iterations: for _iteration in 0..max_iterations {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let tool_defs = self
                .registry
                .to_tool_defs(crate::agent::tool::ToolDefMode::Minimal);

            // Drain pending notifications (e.g. @mention alerts from other Koi)
            if let Some(ref rx_mutex) = self.notification_rx {
                let mut rx = rx_mutex.lock().await;
                while let Ok(msg) = rx.try_recv() {
                    if !seen_notifications.insert(msg.clone()) {
                        info!("Skipping duplicate notification already seen in this run");
                        continue;
                    }
                    let preview = if msg.chars().count() > 80 {
                        format!("{}...", msg.chars().take(80).collect::<String>())
                    } else {
                        msg.clone()
                    };
                    info!("Injecting notification into agent loop: {}", preview);
                    messages.push(LlmMessage {
                        role: "user".into(),
                        content: MessageContent::text(&msg),
                    });
                }
            }

            // Proactive context compaction, delegated to the configured
            // strategy (default reproduces the built-in Level-2 behavior).
            // The loop retains orchestration: token accounting, DB persistence
            // of the rolling summary / state frame, and the UI usage event.
            {
                let total_budget = self.budget.total as usize;
                let static_overhead_tokens = crate::llm::estimate_request_overhead_tokens(
                    Some(&effective_system_prompt),
                    &tool_defs,
                );
                let message_budget = total_budget.saturating_sub(static_overhead_tokens);
                let outcome = self
                    .compaction_strategy
                    .compact(CompactionRequest {
                        trigger: CompactionTrigger::Proactive,
                        budget: self.budget,
                        messages: messages.clone(),
                        rolling_summary: &rolling_summary,
                        system_prompt: &effective_system_prompt,
                        model: &self.model,
                        max_tokens: self.max_tokens,
                        context_window: self.context_window,
                        tool_defs: &tool_defs,
                        tool_minimals: &tool_minimals,
                        session_id: &ctx.session_id,
                        cumulative_input_tokens,
                        next_auto_compact_threshold,
                        threshold_step,
                        client: self.client.as_ref(),
                    })
                    .await;

                if outcome.changed {
                    if let Some(hooks) = &self.hooks {
                        hooks
                            .on_context_event(
                                &crate::agent::hooks::ContextHookEvent::AfterCompact {
                                    session_id: &ctx.session_id,
                                    message_count: outcome.messages.len(),
                                },
                            )
                            .await;
                    }
                    // Account for summariser billing.
                    total_input = total_input.saturating_add(outcome.summary_input_tokens);
                    total_output = total_output.saturating_add(outcome.summary_output_tokens);
                    cumulative_input_tokens = cumulative_input_tokens
                        .saturating_add(i64::from(outcome.summary_input_tokens));
                    cumulative_output_tokens = cumulative_output_tokens
                        .saturating_add(i64::from(outcome.summary_output_tokens));
                    next_auto_compact_threshold = outcome.next_auto_compact_threshold;
                    rolling_summary = outcome.rolling_summary;
                    rolling_summary_version += 1;
                    messages = outcome.messages;

                    if let Some(ref db_arc) = self.db {
                        let db = db_arc.lock().await;
                        if let Err(error) = db.update_session_rolling_summary(
                            &ctx.session_id,
                            &rolling_summary,
                            rolling_summary_version,
                        ) {
                            warn!("Failed to persist rolling summary: {}", error);
                        }
                        // p6 + p7: refresh the state frame so a resume right
                        // after compaction picks up the latest bearings. Merge
                        // in the structured plan / next-step hint when present.
                        let mut frame =
                            crate::agent::state_frame::derive_frame_from_tail(&messages, 24);
                        if !outcome.structured_plan_items.is_empty() {
                            frame.active_plan_items = outcome.structured_plan_items.clone();
                        }
                        if outcome.structured_next_step_hint.is_some() {
                            frame.next_step_hint = outcome.structured_next_step_hint.clone();
                        }
                        let frame_json = frame.to_json();
                        if let Err(error) = db
                            .update_session_state_frame_json(&ctx.session_id, frame_json.as_deref())
                        {
                            warn!("Failed to persist state frame: {}", error);
                        }
                    }
                }

                // Build the (post-compaction) request view + estimate purely
                // for the UI usage event, so the ring reflects what we're about
                // to send. This mirrors the view the LLM-call path rebuilds.
                let demoted = build_request_view_messages(
                    &messages,
                    &tool_minimals,
                    CTX_PRESERVE_RECENT_TURNS,
                    CTX_KEEP_RECENT_TOOL_CARRIERS,
                    message_budget,
                    self.budget.max_tool_result_tokens,
                );
                let req_view = vision::inject_selected_context(&demoted, &ctx.session_id).await;
                let estimated = crate::llm::estimate_request_input_tokens(
                    &req_view,
                    Some(&effective_system_prompt),
                    &tool_defs,
                );
                let trigger_threshold = self.budget.trigger_micro as usize;

                // p8 — best-effort per-layer breakdown for the ring indicator.
                let breakdown_snapshot = {
                    let rolling_tokens = if rolling_summary.trim().is_empty() {
                        0
                    } else {
                        crate::llm::estimate_tokens(&rolling_summary) as u32
                    };
                    let bd = crate::agent::harness::context_builder::compute_layered_breakdown(
                        &req_view,
                        Some(&effective_system_prompt),
                        &tool_defs,
                        &tool_minimals,
                        rolling_tokens,
                        0,
                    );
                    crate::agent::messages::LayeredTokenBreakdownSnapshot {
                        persona: bd.prompt.persona,
                        scene: bd.prompt.scene,
                        memory: bd.prompt.memory,
                        project: bd.prompt.project,
                        platform_hint: bd.prompt.platform_hint,
                        tool_defs: bd.tool_def_tokens,
                        history_text: bd.history_text_tokens,
                        history_tool_result_full: bd.history_tool_result_full_tokens,
                        history_tool_result_receipt: bd.history_tool_result_receipt_tokens,
                        rolling_summary: bd.rolling_summary_tokens,
                        state_frame: bd.state_frame_tokens,
                        vision: bd.vision_tokens,
                        request_overhead: bd.request_overhead_tokens,
                    }
                };

                let _ = event_tx
                    .send(AgentEvent::ContextUsage {
                        estimated_input_tokens: estimated.min(u32::MAX as usize) as u32,
                        total_input_budget: total_budget.min(u32::MAX as usize) as u32,
                        trigger_threshold: trigger_threshold.min(u32::MAX as usize) as u32,
                        cumulative_input_tokens: cumulative_input_tokens.clamp(0, u32::MAX as i64)
                            as u32,
                        cumulative_output_tokens: cumulative_output_tokens.clamp(0, u32::MAX as i64)
                            as u32,
                        rolling_summary_version: rolling_summary_version.clamp(0, u32::MAX as i64)
                            as u32,
                        auto_compact_threshold: self.auto_compact_input_tokens_threshold,
                        layered_breakdown: Some(breakdown_snapshot),
                    })
                    .await;
            }

            info!(
                "agent loop iteration={} messages={}",
                _iteration,
                messages.len()
            );

            // Signal frontend that a new LLM call is starting — it should replace the
            // current streaming bubble with a fresh one (slide old out, slide new in).
            let _ = event_tx
                .send(AgentEvent::TextSegmentStart {
                    iteration: _iteration as u32 + 1,
                })
                .await;

            // Call LLM with exponential-backoff retry for transient failures,
            // model fallback for rate_limit/model_not_found errors,
            // and level-2 LLM summarisation for context overflow errors.
            //
            // req_messages is rebuilt inside the loop so that after compact_summarise
            // updates `messages`, the next attempt uses the compacted context.
            info!("calling LLM: model={}", self.model);
            let mut cancelled_partial_text: Option<String> = None;
            // Caps the "please resend the full tool call JSON" corrective
            // retry (P0-2) per iteration, so a model that keeps truncating
            // its output cannot loop forever.
            let mut tool_args_corrections = 0u8;
            let mut context_overflow_attempted = false;
            let mut main_calls_remaining = MAX_MAIN_CALLS_PER_ITERATION;
            let mut usage_model = self.model.clone();
            let response = 'attempt_with_correction: loop {
                // Loop-strategy next-turn hint may override the primary model
                // for this iteration (default: no override).
                let primary_model = self
                    .loop_strategy
                    .as_ref()
                    .and_then(|strat| {
                        strat
                            .prepare_next_turn(&crate::agent::loop_strategy::TurnContext {
                                iteration: _iteration,
                                had_tool_calls: last_turn_had_tool_calls,
                                last_text: last_turn_text.clone(),
                            })
                            .model
                    })
                    .unwrap_or_else(|| self.model.clone());
                let models_to_try =
                    bounded_model_candidates(primary_model.clone(), &self.fallback_models);
                let mut last_err: Option<anyhow::Error> = None;
                let mut resp: Option<crate::llm::LlmResponse> = None;
                let mut succeeded_model: Option<String> = None;
                let mut model_index = 0usize;

                'model_loop: while let Some(model_candidate) = models_to_try.get(model_index) {
                    // Build req_messages inside the model loop so that after
                    // compact_summarise updates `messages`, we use the fresh context.
                    // Tool-result blocks from older turns are swapped to their
                    // minimal receipts before vision-context injection.
                    let static_overhead_tokens = crate::llm::estimate_request_overhead_tokens(
                        Some(&effective_system_prompt),
                        &tool_defs,
                    );
                    let message_budget =
                        (self.budget.total as usize).saturating_sub(static_overhead_tokens);
                    let demoted_messages = build_request_view_messages(
                        &messages,
                        &tool_minimals,
                        CTX_PRESERVE_RECENT_TURNS,
                        CTX_KEEP_RECENT_TOOL_CARRIERS,
                        message_budget,
                        self.budget.max_tool_result_tokens,
                    );
                    let req_messages =
                        vision::inject_selected_context(&demoted_messages, &ctx.session_id).await;
                    // Vision delegation: if a separate vision model is configured
                    // and there are image blocks in the messages, send them to the
                    // vision model for analysis and replace with text descriptions.
                    let req_messages = if let Some(ref vision_client) = self.vision_delegate {
                        let delegated = Self::delegate_vision_analysis(
                            &req_messages,
                            vision_client.as_ref(),
                            &self.vision_model,
                        )
                        .await;
                        // Clear selection after delegation so the same images are not
                        // re-injected on the next iteration.  The vision analysis text
                        // is already embedded in req_messages for this LLM call; the
                        // LLM's response will reference what it learned.  If the agent
                        // needs to examine images again, it can re-select via
                        // vision_context.
                        vision::clear_selection(&ctx.session_id).await;
                        // Safety net: strip any remaining image blocks that the delegate
                        // didn't process (e.g. from older tool results kept by strip_images).
                        // The main LLM must NEVER see image content.
                        delegated
                            .into_iter()
                            .map(|mut m| {
                                if let crate::llm::MessageContent::Blocks(ref mut blocks) =
                                    m.content
                                {
                                    blocks.retain(|b| {
                                        !matches!(b, crate::llm::ContentBlock::Image { .. })
                                    });
                                }
                                m
                            })
                            .collect()
                    } else {
                        // Main model handles vision directly.  Still clear the
                        // selection so the same images are not re-injected on
                        // subsequent iterations, which would waste tokens and
                        // could confuse the model into re-processing images it
                        // already described.  The model's response (persisted
                        // to `messages`) will capture what it learned.
                        vision::clear_selection(&ctx.session_id).await;
                        req_messages
                    };
                    // Loop-strategy context transform: last chance for a host
                    // policy to reshape the exact message list going on the wire
                    // (default is identity, so behaviour is unchanged).
                    let req_messages = match &self.loop_strategy {
                        Some(strat) => strat.transform_context(req_messages),
                        None => req_messages,
                    };
                    // Route through RequestBuilder so provider-specific
                    // ceilings (e.g. Anthropic's 8192 max_tokens cap) are
                    // applied in one place instead of leaking into every
                    // call site. The builder is cheap to construct and we
                    // rebuild per-iteration so fallback to another model —
                    // possibly a different provider — picks up the right
                    // limits automatically.
                    let provider_kind =
                        crate::agent::harness::ProviderKind::from_model_id(model_candidate);
                    let req = crate::agent::harness::RequestBuilder::new(
                        req_messages,
                        Some(effective_system_prompt.clone()),
                        tool_defs.clone(),
                        model_candidate.clone(),
                        self.max_tokens,
                    )
                    .with_stream(self.enable_streaming)
                    .with_vision_override(self.vision_override)
                    .build_for(provider_kind);

                    let same_model_attempts = self
                        .same_model_transient_retries
                        .clamp(1, MAX_SAME_MODEL_ATTEMPTS);
                    for attempt in 0..same_model_attempts {
                        // Check cancel before each LLM attempt
                        if cancel.load(Ordering::Relaxed) {
                            break 'model_loop;
                        }
                        if main_calls_remaining == 0 {
                            last_err = Some(anyhow::anyhow!("primary agent request budget exhausted (maximum {} logical calls per iteration)", MAX_MAIN_CALLS_PER_ITERATION));
                            break 'model_loop;
                        }
                        main_calls_remaining -= 1;

                        let streaming_partial = self
                            .enable_streaming
                            .then(|| Arc::new(Mutex::new(String::new())));

                        // Race the LLM call against the cancel flag (200ms poll)
                        let cancel_for_llm = Arc::clone(&cancel);
                        let llm_result = tokio::select! {
                            biased;
                            _ = async {
                                loop {
                                    tokio::time::sleep(
                                        std::time::Duration::from_millis(200),
                                    ).await;
                                    if cancel_for_llm.load(Ordering::Relaxed) { break; }
                                }
                            } => {
                                info!("LLM call cancelled by user");
                                if let Some(partial) = &streaming_partial {
                                    let partial = partial.lock().await.clone();
                                    if !partial.trim().is_empty() {
                                        cancelled_partial_text = Some(partial);
                                    }
                                }
                                break 'model_loop;
                            }
                            r = llm_call_unified(
                                self.client.as_ref(),
                                req.clone(),
                                self.enable_streaming,
                                &event_tx,
                                streaming_partial.clone(),
                            ) => r,
                        };
                        if cancel.load(Ordering::Relaxed) {
                            if let Some(partial) = streaming_partial {
                                let partial = partial.lock().await.clone();
                                if !partial.trim().is_empty() {
                                    cancelled_partial_text = Some(partial);
                                }
                            }
                        }

                        match llm_result {
                            Ok(r) => {
                                resp = Some(r);
                                succeeded_model = Some(model_candidate.clone());
                                break 'model_loop;
                            }
                            Err(e) => {
                                let msg = e.to_string();
                                // P1-3 observability: structured fields so failures are
                                // greppable/alertable without parsing free-text messages.
                                let error_class = crate::llm::error_class::classify_error(&msg);
                                let attempt_error_code = error_class.code();
                                warn!(
                                    model = %model_candidate,
                                    attempt = attempt + 1,
                                    max_attempts = same_model_attempts,
                                    error_code = attempt_error_code,
                                    "LLM call attempt {}/{} model={} failed: {}",
                                    attempt + 1,
                                    same_model_attempts,
                                    model_candidate,
                                    msg
                                );

                                if matches!(
                                    error_class,
                                    crate::llm::error_class::ErrorClass::AuthFailed
                                        | crate::llm::error_class::ErrorClass::QuotaExceeded
                                ) {
                                    last_err = Some(e);
                                    break 'model_loop;
                                }
                                if is_context_overflow_error(&msg)
                                    && !context_overflow_attempted
                                    && main_calls_remaining > 0
                                {
                                    context_overflow_attempted = true;
                                    // Forced recovery via the same pluggable
                                    // strategy (Overflow trigger) so a custom
                                    // host policy is never bypassed. Uses the
                                    // candidate model — possibly a fallback —
                                    // as the summariser.
                                    let outcome = self
                                        .compaction_strategy
                                        .compact(CompactionRequest {
                                            trigger: CompactionTrigger::Overflow,
                                            budget: self.budget,
                                            messages: messages.clone(),
                                            rolling_summary: &rolling_summary,
                                            system_prompt: &effective_system_prompt,
                                            model: model_candidate,
                                            max_tokens: self.max_tokens,
                                            context_window: self.context_window,
                                            tool_defs: &tool_defs,
                                            tool_minimals: &tool_minimals,
                                            session_id: &ctx.session_id,
                                            cumulative_input_tokens,
                                            next_auto_compact_threshold,
                                            threshold_step,
                                            client: self.client.as_ref(),
                                        })
                                        .await;
                                    if outcome.changed {
                                        if let Some(hooks) = &self.hooks {
                                            hooks
                                                .on_context_event(
                                                    &crate::agent::hooks::ContextHookEvent::AfterCompact {
                                                        session_id: &ctx.session_id,
                                                        message_count: outcome.messages.len(),
                                                    },
                                                )
                                                .await;
                                        }
                                        total_input = total_input
                                            .saturating_add(outcome.summary_input_tokens);
                                        total_output = total_output
                                            .saturating_add(outcome.summary_output_tokens);
                                        cumulative_input_tokens = cumulative_input_tokens
                                            .saturating_add(i64::from(
                                                outcome.summary_input_tokens,
                                            ));
                                        cumulative_output_tokens = cumulative_output_tokens
                                            .saturating_add(i64::from(
                                                outcome.summary_output_tokens,
                                            ));
                                        next_auto_compact_threshold =
                                            outcome.next_auto_compact_threshold;
                                        rolling_summary = outcome.rolling_summary;
                                        rolling_summary_version += 1;
                                        messages = outcome.messages;
                                        if let Some(ref db_arc) = self.db {
                                            let db = db_arc.lock().await;
                                            if let Err(error) = db.update_session_rolling_summary(
                                                &ctx.session_id,
                                                &rolling_summary,
                                                rolling_summary_version,
                                            ) {
                                                warn!(
                                                    "Failed to persist rolling summary after overflow: {}",
                                                    error
                                                );
                                            }
                                            // p6 + p7: refresh state frame on overflow-triggered
                                            // compaction as well so the UI sees a fresh snapshot.
                                            let mut frame =
                                                crate::agent::state_frame::derive_frame_from_tail(
                                                    &messages, 24,
                                                );
                                            if !outcome.structured_plan_items.is_empty() {
                                                frame.active_plan_items =
                                                    outcome.structured_plan_items.clone();
                                            }
                                            if outcome.structured_next_step_hint.is_some() {
                                                frame.next_step_hint =
                                                    outcome.structured_next_step_hint.clone();
                                            }
                                            let frame_json = frame.to_json();
                                            if let Err(error) = db.update_session_state_frame_json(
                                                &ctx.session_id,
                                                frame_json.as_deref(),
                                            ) {
                                                warn!(
                                                    "Failed to persist state frame after overflow: {}",
                                                    error
                                                );
                                            }
                                        }
                                        info!(
                                            "summarisation complete, messages={}",
                                            messages.len()
                                        );
                                        // Restart model_loop with the compacted context.
                                        last_err = Some(e);
                                        continue 'model_loop;
                                    } else {
                                        // Summarisation failed — cannot recover from overflow.
                                        warn!("summarisation failed, cannot recover from context overflow");
                                        last_err = Some(e);
                                        break 'model_loop;
                                    }
                                } else if error_class.is_fallback_eligible() {
                                    // rate_limit / model_not_found: try next fallback model.
                                    // overloaded is intentionally excluded — it should be
                                    // retried with backoff on the same model.
                                    info!(
                                        fallback_from = %model_candidate,
                                        error_code = attempt_error_code,
                                        "abandoning model after fallback-eligible error, trying next candidate"
                                    );
                                    last_err = Some(e);
                                    break;
                                } else {
                                    if !error_class.is_retryable()
                                        || attempt + 1 == same_model_attempts
                                        || main_calls_remaining == 0
                                    {
                                        last_err = Some(e);
                                        break 'model_loop;
                                    }
                                    // Interruptible backoff sleep — cancel exits immediately
                                    let backoff = retry_backoff(attempt);
                                    let cancel_for_sleep = Arc::clone(&cancel);
                                    tokio::select! {
                                        biased;
                                        _ = async {
                                            loop {
                                                tokio::time::sleep(
                                                    std::time::Duration::from_millis(200),
                                                ).await;
                                                if cancel_for_sleep.load(Ordering::Relaxed) {
                                                    break;
                                                }
                                            }
                                        } => { break 'model_loop; }
                                        _ = tokio::time::sleep(backoff) => {}
                                    }
                                    last_err = Some(e);
                                }
                            }
                        }
                    }
                    model_index += 1;
                }
                match resp {
                    Some(r) => {
                        // P1-2 lock-in decision: switching away from the primary
                        // model must never be silent. Disclose "A → B" via a
                        // non-terminal Notice (does not touch running/streaming
                        // UI state) whenever the model that actually answered
                        // isn't the one the caller originally asked for.
                        if let Some(ref used_model) = succeeded_model {
                            if used_model != &primary_model {
                                info!(
                                    "model fallback: primary={} used={}",
                                    primary_model, used_model
                                );
                                let _ = event_tx
                                    .send(AgentEvent::Notice {
                                        message: format!(
                                            "主模型「{}」当前不可用，已自动切换到备选模型「{}」继续为你处理。",
                                            primary_model, used_model
                                        ),
                                        code: Some("model_fallback".to_string()),
                                        details: Some(serde_json::json!({
                                            "from": primary_model,
                                            "to": used_model,
                                        })),
                                    })
                                    .await;
                            }
                        }
                        if let Some(ref m) = succeeded_model {
                            usage_model = m.clone();
                        }
                        break 'attempt_with_correction r;
                    }
                    None => {
                        // If cancelled, break the outer iteration loop cleanly
                        if cancel.load(Ordering::Relaxed) {
                            if let Some(partial) = cancelled_partial_text.take() {
                                let asst_msg = LlmMessage {
                                    role: "assistant".into(),
                                    content: MessageContent::text(&partial),
                                };
                                new_messages.push(asst_msg.clone());
                                messages.push(asst_msg.clone());
                                self.persist_message(&ctx.session_id, &asst_msg, turn_index)
                                    .await;
                            }
                            break 'iterations;
                        }
                        let err = last_err.unwrap_or_else(|| anyhow::anyhow!("LLM call failed"));
                        let msg = err.to_string();
                        let error_class = crate::llm::error_class::classify_error(&msg);

                        // P0-2: a truncated/malformed tool-call `arguments` JSON is
                        // never silently executed (see llm::openai) and never
                        // silently retried forever either. Give the model exactly
                        // one chance, in the SAME iteration, to resend a complete
                        // and valid call before treating it as a hard failure.
                        if tool_args_corrections < 2
                            && error_class == crate::llm::error_class::ErrorClass::ToolArgsInvalid
                        {
                            tool_args_corrections += 1;
                            warn!(
                                "tool_args_invalid on iteration {}: corrective retry {}: {}",
                                _iteration, tool_args_corrections, msg
                            );
                            let text = if tool_args_corrections == 1 {
                                "系统提示：你上一次的工具调用参数(arguments) JSON 不完整或无法解析，\
                                 因此未被执行，也没有产生任何副作用。请重新完整输出这次工具调用\
                                 （不要截断、不要省略字段）；如内容较长，可考虑拆分为多次更短的调用。"
                            } else {
                                "系统提示：工具调用参数 JSON 再次无法解析，仍未执行。字符串中的双引号必须写成 \\\"，\
                                 反斜杠写成 \\\\，换行写成 \\n。请把大段代码拆成多次较短的 file_write/file_edit 调用，\
                                 不要在一次调用里放入整份长文件。"
                            };
                            let corrective = LlmMessage {
                                role: "user".into(),
                                content: MessageContent::text(text),
                            };
                            new_messages.push(corrective.clone());
                            messages.push(corrective.clone());
                            self.persist_message(&ctx.session_id, &corrective, turn_index)
                                .await;
                            // Notice, not Error: the run is NOT stopping — we're
                            // about to retry in this same iteration. Sending
                            // `Error` here would make frontends (which treat
                            // Error as terminal) clear the running/streaming
                            // state out from under an in-flight retry.
                            let _ = event_tx
                                .send(AgentEvent::Notice {
                                    message: format!(
                                        "工具调用参数不完整，已请求模型重新输出（自动纠偏 1 次）：{}",
                                        msg
                                    ),
                                    code: Some(error_class.code().to_string()),
                                    details: None,
                                })
                                .await;
                            continue 'attempt_with_correction;
                        }

                        let _ = event_tx
                            .send(AgentEvent::Error {
                                message: err.to_string(),
                                code: Some(error_class.code().to_string()),
                                details: None,
                            })
                            .await;
                        return Err(err);
                    }
                }
            };
            info!(
                "LLM response: input_tokens={} output_tokens={} tool_calls={} text_len={}",
                response.input_tokens,
                response.output_tokens,
                response.tool_calls.len(),
                response.content.len()
            );
            total_input += response.input_tokens;
            total_output += response.output_tokens;
            cumulative_input_tokens += i64::from(response.input_tokens);
            cumulative_output_tokens += i64::from(response.output_tokens);
            if let Some(ref db_arc) = self.db {
                let db = db_arc.lock().await;
                if let Err(error) = db.update_session_usage_totals(
                    &ctx.session_id,
                    response.input_tokens,
                    response.output_tokens,
                ) {
                    warn!("Failed to persist usage totals: {}", error);
                }
                if let Err(error) = db.add_model_usage(
                    &ctx.session_id,
                    &usage_model,
                    response.input_tokens,
                    response.output_tokens,
                ) {
                    warn!("Failed to persist model usage: {}", error);
                }
            }

            let text_buf = response.content.clone();
            let tool_calls: Vec<(String, String, serde_json::Value)> = response
                .tool_calls
                .iter()
                .map(|tc| (tc.id.clone(), tc.name.clone(), tc.input.clone()))
                .collect();

            // In non-streaming mode we emit the whole response as a single
            // `TextDelta`. Streaming mode already forwarded per-chunk deltas
            // inside `llm_call_unified`, so re-emitting here would duplicate
            // the text in the UI.
            if !self.enable_streaming && !text_buf.is_empty() {
                let _ = event_tx
                    .send(AgentEvent::TextDelta {
                        delta: text_buf.clone(),
                    })
                    .await;
            }

            // If no tool calls, check for unfinished plan_todo items before exiting.
            // If any todo is still in_progress or pending, inject a reminder and continue.
            if tool_calls.is_empty() {
                // Check if there are any in_progress or pending todos that haven't been resolved
                let unfinished_todos = if let Some(ref plan_state_arc) = self.plan_state {
                    let plan_state = plan_state_arc.lock().await;
                    plan_state
                        .get(&ctx.session_id)
                        .map(|todos| {
                            todos
                                .iter()
                                .filter(|t| t.status == "in_progress" || t.status == "pending")
                                .map(|t| format!("- [{}] {}", t.status, t.content))
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default()
                } else {
                    vec![]
                };

                if !unfinished_todos.is_empty() && todo_reminder_count < TODO_REMINDER_MAX {
                    // Inject a reminder and continue the loop instead of breaking
                    todo_reminder_count += 1;
                    warn!(
                        "LLM tried to exit with {} unfinished todo(s), injecting reminder ({}/{})",
                        unfinished_todos.len(),
                        todo_reminder_count,
                        TODO_REMINDER_MAX
                    );
                    let asst_msg = LlmMessage {
                        role: "assistant".into(),
                        content: MessageContent::text(&text_buf),
                    };
                    new_messages.push(asst_msg.clone());
                    messages.push(asst_msg.clone());
                    self.persist_message(&ctx.session_id, &asst_msg, turn_index)
                        .await;
                    let reminder = format!(
                        "⚠️ 你的计划中还有未完成的步骤，请继续执行或将其标记为 cancelled：\n{}\n\n\
                         `plan_todo` 只更新计划板，本身不算实际进展。请继续使用能真正推进任务的工具，或直接产出可交付结果；如果这些步骤无法完成，再用 `plan_todo` 标记为 cancelled 并说明原因。",
                        unfinished_todos.join("\n")
                    );
                    let reminder_msg = LlmMessage {
                        role: "user".into(),
                        content: MessageContent::text(&reminder),
                    };
                    new_messages.push(reminder_msg.clone());
                    messages.push(reminder_msg.clone());
                    self.persist_message(&ctx.session_id, &reminder_msg, turn_index)
                        .await;
                    // Continue the loop (don't break)
                } else {
                    // All todos are done (or no plan exists) — normal exit
                    let asst_msg = LlmMessage {
                        role: "assistant".into(),
                        content: MessageContent::text(&text_buf),
                    };
                    new_messages.push(asst_msg.clone());
                    messages.push(asst_msg.clone());
                    self.persist_message(&ctx.session_id, &asst_msg, turn_index)
                        .await;
                    break;
                }
            }

            // ── Per-tool loop detection (before execution) ──────────────────
            // Check each tool call against the sliding window history.
            // Critical = block the tool call; Warning = inject hint but continue.
            // Reset the todo reminder counter since the LLM is making progress.
            todo_reminder_count = 0;
            let mut blocked_tool_ids: Vec<String> = Vec::new();
            let mut warning_messages: Vec<String> = Vec::new();
            for (id, name, input) in &tool_calls {
                let detection = loop_detector.detect(name, input, &tool_calls);
                match detection.level {
                    LoopLevel::Critical => {
                        warn!(
                            "Loop CRITICAL [{}]: tool='{}' count={} detector={:?}",
                            ctx.session_id, name, detection.count, detection.detector
                        );
                        blocked_tool_ids.push(id.clone());
                        warning_messages.push(detection.message);
                    }
                    LoopLevel::Warning => {
                        warn!(
                            "Loop WARNING [{}]: tool='{}' count={} detector={:?}",
                            ctx.session_id, name, detection.count, detection.detector
                        );
                        warning_messages.push(detection.message);
                    }
                    LoopLevel::Ok => {}
                }
            }

            let all_tools_blocked =
                !blocked_tool_ids.is_empty() && blocked_tool_ids.len() == tool_calls.len();

            // If all tool calls are blocked, surface a stronger reminder but still
            // let the agent see synthetic tool failures and produce a final answer
            // from the evidence it already has.
            if all_tools_blocked {
                let combined_msg = warning_messages.join("\n");
                let _ = event_tx
                    .send(AgentEvent::TextDelta {
                        delta: format!("\n\n[系统] {}\n", combined_msg),
                    })
                    .await;
            }

            // Build assistant message with tool calls
            let mut assistant_blocks: Vec<ContentBlock> = Vec::new();
            if !text_buf.is_empty() {
                assistant_blocks.push(ContentBlock::Text {
                    text: text_buf.clone(),
                });
            }
            for (id, name, input) in &tool_calls {
                assistant_blocks.push(ContentBlock::ToolUse {
                    id: id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                });
            }
            let asst_tool_msg = LlmMessage {
                role: "assistant".into(),
                content: MessageContent::Blocks(assistant_blocks),
            };
            new_messages.push(asst_tool_msg.clone());
            messages.push(asst_tool_msg.clone());
            let persist_asst = crate::agent::message_utils::strip_ephemeral_tool_exchanges(vec![
                asst_tool_msg.clone(),
            ]);
            if let Some(msg) = persist_asst.into_iter().next() {
                self.persist_message(&ctx.session_id, &msg, turn_index)
                    .await;
            }

            // Execute tools — read-only concurrently, write serially.
            // Blocked tools (by loop detector) get a synthetic error result instead.
            let mut tool_result_blocks: Vec<ContentBlock> = Vec::new();

            // Separate blocked, read-only, and write calls
            let active_calls: Vec<_> = tool_calls
                .iter()
                .filter(|(id, _, _)| !blocked_tool_ids.contains(id))
                .cloned()
                .collect();
            let read_only_calls: Vec<_> = active_calls
                .iter()
                .filter(|(_, name, _)| {
                    self.registry
                        .get(name)
                        .map(|t| t.is_read_only())
                        .unwrap_or(false)
                })
                .cloned()
                .collect();
            let write_calls: Vec<_> = active_calls
                .iter()
                .filter(|(_, name, _)| {
                    !self
                        .registry
                        .get(name)
                        .map(|t| t.is_read_only())
                        .unwrap_or(false)
                })
                .cloned()
                .collect();

            // Inject synthetic error results for blocked tools
            for (id, name, _) in &tool_calls {
                if blocked_tool_ids.contains(id) {
                    let msg = warning_messages
                        .iter()
                        .find(|m| m.contains(name.as_str()))
                        .cloned()
                        .unwrap_or_else(|| format!("工具 '{}' 被循环检测器阻断。", name));
                    tool_result_blocks.push(ContentBlock::ToolResult {
                        tool_use_id: id.clone(),
                        content: format!("[循环检测] {}", msg),
                        is_error: true,
                    });
                    let _ = event_tx
                        .send(AgentEvent::ToolEnd {
                            id: id.clone(),
                            name: name.clone(),
                            result: format!("[循环检测] {}", msg),
                            is_error: true,
                        })
                        .await;
                }
            }

            // Execute read-only tools concurrently
            if !read_only_calls.is_empty() {
                let mut start = 0usize;
                while start < read_only_calls.len() {
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    let end = (start + READ_TOOL_MAX_CONCURRENCY).min(read_only_calls.len());
                    let batch = &read_only_calls[start..end];
                    let futs: Vec<_> = batch
                        .iter()
                        .map(|(id, name, input)| {
                            self.execute_single_tool(id, name, input, &ctx, &event_tx, &cancel)
                        })
                        .collect();
                    for blocks in join_all(futs).await {
                        tool_result_blocks.extend(blocks);
                    }
                    start = end;
                }
            }

            // Execute write tools serially
            for (id, name, input) in &write_calls {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                let blocks = self
                    .execute_single_tool(id, name, input, &ctx, &event_tx, &cancel)
                    .await;
                tool_result_blocks.extend(blocks);
            }

            crate::agent::message_utils::ensure_tool_results_complete(
                &tool_calls,
                &mut tool_result_blocks,
                "Tool execution was cancelled or interrupted before completion.",
            );

            // ── Record results into loop detector + compute minimal receipts ─
            for block in &tool_result_blocks {
                if let ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                    ..
                } = block
                {
                    if let Some((_, name, input)) =
                        tool_calls.iter().find(|(id, _, _)| id == tool_use_id)
                    {
                        if crate::agent::message_utils::is_ephemeral_tool_call(name, input) {
                            continue;
                        }
                        let rh = stable_hash_result(content);
                        loop_detector.record(name, input, rh, stable_hash_normalized(content));
                        let receipt = super::tool_receipt::render_receipt(
                            name, input, content, *is_error, None,
                        );
                        tool_minimals.insert(tool_use_id.clone(), receipt);
                        tool_names_by_id.insert(tool_use_id.clone(), name.clone());
                    } else {
                        // No matching ToolUse (shouldn't happen) — still emit a
                        // generic receipt so the middle-tier read path doesn't
                        // have to backfill from an unknown tool name.
                        let receipt = super::tool_receipt::render_receipt(
                            "unknown",
                            &serde_json::Value::Null,
                            content,
                            *is_error,
                            None,
                        );
                        tool_minimals.insert(tool_use_id.clone(), receipt);
                    }
                }
            }

            let warning_reminder = if all_tools_blocked {
                Some(format!(
                    "[系统提醒]\n{}\n本轮所有工具调用都已被循环检测器阻断。现在必须停止继续沿用刚才的工具路径，直接基于已有证据给出当前最佳答复；如果信息仍有缺口，请明确列出不确定项、现有判断依据与后续建议，不要继续调用工具。",
                    warning_messages.join("\n")
                ))
            } else if !warning_messages.is_empty() && blocked_tool_ids.is_empty() {
                Some(format!(
                    "[系统提醒]\n{}\n请先基于现有结果收束、总结或切换方法，不要机械重复刚才的工具路径。",
                    warning_messages.join("\n")
                ))
            } else {
                None
            };

            // Add tool results as user message
            let tool_result_msg = LlmMessage {
                role: "user".into(),
                content: MessageContent::Blocks(tool_result_blocks),
            };
            new_messages.push(tool_result_msg.clone());
            messages.push(tool_result_msg.clone());
            let persist_results =
                crate::agent::message_utils::strip_ephemeral_tool_exchanges(vec![
                    tool_result_msg.clone()
                ]);
            if let Some(msg) = persist_results.into_iter().next() {
                self.persist_message_with_receipts(
                    &ctx.session_id,
                    &msg,
                    turn_index,
                    Some(&tool_minimals),
                    Some(&tool_names_by_id),
                )
                .await;
            }
            messages = crate::agent::message_utils::collapse_superseded_tool_failures(messages);
            messages = crate::agent::message_utils::sanitize_tool_use_result_pairing(messages);
            messages = crate::agent::message_utils::strip_ephemeral_tool_exchanges(messages);
            new_messages =
                crate::agent::message_utils::strip_ephemeral_tool_exchanges(new_messages);

            if cancel.load(Ordering::Relaxed) {
                info!("agent loop cancelled after tool-result persistence");
                break;
            }

            if ctx
                .loop_halt
                .as_ref()
                .is_some_and(|h| h.load(Ordering::Relaxed))
            {
                info!("agent loop halted by host (plan ready / await user build)");
                break;
            }

            if let Some(reminder) = warning_reminder {
                let reminder_msg = LlmMessage {
                    role: "user".into(),
                    content: MessageContent::text(&reminder),
                };
                let _ = event_tx
                    .send(AgentEvent::TextDelta {
                        delta: format!("\n\n{}\n", reminder),
                    })
                    .await;
                new_messages.push(reminder_msg.clone());
                messages.push(reminder_msg.clone());
                self.persist_message(&ctx.session_id, &reminder_msg, turn_index)
                    .await;
            }

            // Write checkpoint after each iteration (with size guard)
            if let Some(ref db_arc) = self.db {
                let db = db_arc.lock().await;
                let payload = AgentCheckpointPayload {
                    base_context_hash,
                    base_message_count,
                    messages: messages.clone(),
                    loop_history: loop_detector.history.clone(),
                    seen_notifications: seen_notifications.iter().cloned().collect(),
                };
                match serde_json::to_string(&payload) {
                    Ok(json) => {
                        if json.len() > CHECKPOINT_MAX_BYTES {
                            warn!(
                                "Checkpoint too large ({} bytes > {} limit), skipping write",
                                json.len(),
                                CHECKPOINT_MAX_BYTES
                            );
                            let _ = db.finish_checkpoint(&ctx.session_id, "oversized");
                        } else if let Err(e) =
                            db.upsert_checkpoint(&ctx.session_id, _iteration, &json)
                        {
                            warn!("Failed to write checkpoint: {}", e);
                        }
                    }
                    Err(e) => warn!("Failed to serialise checkpoint messages: {}", e),
                }
            }

            // Loop-strategy turn boundary: record turn state for the next-turn
            // hint, and honour an explicit early-stop request (default: never).
            last_turn_had_tool_calls = !tool_calls.is_empty();
            last_turn_text = text_buf.clone();
            if let Some(strat) = &self.loop_strategy {
                let turn = crate::agent::loop_strategy::TurnContext {
                    iteration: _iteration,
                    had_tool_calls: last_turn_had_tool_calls,
                    last_text: last_turn_text.clone(),
                };
                if strat.should_stop_after_turn(&turn) {
                    info!(
                        "loop strategy '{}' requested stop after turn {}",
                        strat.name(),
                        _iteration + 1
                    );
                    break;
                }
            }
        }

        // Mark checkpoint as completed so it won't be resumed next run
        if let Some(ref db_arc) = self.db {
            let db = db_arc.lock().await;
            let _ = db.finish_checkpoint(&ctx.session_id, "completed");
            // Prune checkpoints older than 24 hours
            let _ = db.prune_checkpoints(24);
        }

        // Return only the new messages produced during this run (not the full context).
        // new_messages is immune to compaction: it accumulates every assistant/tool message
        // appended during the run, regardless of how many times the context was compacted.
        // The caller (persist_agent_turn) saves exactly these messages to the DB.
        Ok((new_messages, total_input, total_output))
    }
}

/// Convert low-level tool errors into actionable, user-friendly messages.
fn friendly_tool_error(tool_name: &str, raw_error: &str) -> String {
    let raw_lower = raw_error.to_lowercase();

    if is_structural_schema_error(raw_error) {
        return format!(
            "[{}] 工具输入与 schema 不匹配。请根据下方 schema_correction 修正参数，仅重试这个工具一次。\n详情：{}",
            tool_name, raw_error
        );
    }

    // File system errors
    if raw_lower.contains("no such file")
        || raw_lower.contains("not found")
        || raw_lower.contains("cannot find")
    {
        return format!(
            "[{}] 文件或路径不存在。请确认路径正确，或先用 file_write 创建文件。\n详情：{}",
            tool_name, raw_error
        );
    }
    if raw_lower.contains("permission denied")
        || raw_lower.contains("access is denied")
        || raw_lower.contains("拒绝访问")
        || raw_lower.contains("0x80070005")
    {
        if tool_name == "shell" || tool_name == "file_write" {
            return format!(
                "[{}] 权限不足（Access Denied）。\
                 如需管理员权限，请对 shell 工具使用 elevated: true 参数，\
                 Windows 会弹出 UAC 对话框请用户确认。\n详情：{}",
                tool_name, raw_error
            );
        }
        return format!(
            "[{}] 权限不足，无法访问该文件/目录。\
             如需管理员权限，请使用 shell 工具并设置 elevated: true。\n详情：{}",
            tool_name, raw_error
        );
    }
    if raw_lower.contains("already exists") {
        return format!(
            "[{}] 文件或目录已存在。如需覆盖，请使用 file_write（会自动覆盖）。\n详情：{}",
            tool_name, raw_error
        );
    }

    // Network errors
    if raw_lower.contains("connection refused") || raw_lower.contains("connection reset") {
        return format!(
            "[{}] 网络连接失败。请检查网络连接或目标服务是否可用。\n详情：{}",
            tool_name, raw_error
        );
    }
    if raw_lower.contains("timeout") || raw_lower.contains("timed out") {
        return format!(
            "[{}] 网络请求超时。请检查网络状态，或稍后重试。\n详情：{}",
            tool_name, raw_error
        );
    }
    if raw_lower.contains("dns") || raw_lower.contains("resolve") || raw_lower.contains("no route")
    {
        return format!(
            "[{}] DNS 解析失败，无法访问目标地址。请检查网络连接。\n详情：{}",
            tool_name, raw_error
        );
    }

    // Shell/process errors
    if tool_name == "shell" || tool_name == "powershell_query" {
        if raw_lower.contains("not recognized") || raw_lower.contains("not found") {
            return format!(
                "[{}] 命令未找到。请确认命令名称正确，或该程序已安装并在 PATH 中。\n详情：{}",
                tool_name, raw_error
            );
        }
        if raw_lower.contains("exit code") {
            return format!(
                "[{}] 命令执行失败（非零退出码）。请检查命令语法和参数。\n详情：{}",
                tool_name, raw_error
            );
        }
    }

    // Browser errors
    if tool_name == "browser" {
        if raw_lower.contains("chrome")
            || raw_lower.contains("browser")
            || raw_lower.contains("cdp")
        {
            return format!(
                "[{}] 浏览器连接失败。请确认 Chrome 已安装，或在设置中检查浏览器配置。\n详情：{}",
                tool_name, raw_error
            );
        }
        if raw_lower.contains("element") || raw_lower.contains("selector") {
            return format!(
                "[{}] 页面元素未找到。页面可能尚未加载完成，或选择器有误。建议先截图确认页面状态。\n详情：{}",
                tool_name, raw_error
            );
        }
    }

    // WMI / COM errors
    if (tool_name == "wmi" || tool_name == "com")
        && (raw_lower.contains("wmi")
            || raw_lower.contains("com")
            || raw_lower.contains("dispatch"))
    {
        return format!(
            "[{}] Windows 系统接口调用失败。请确认以管理员权限运行，或该功能在当前系统版本可用。\n详情：{}",
            tool_name, raw_error
        );
    }

    // com_invoke errors
    if tool_name == "com_invoke" {
        if raw_lower.contains("regdb_e_classnotreg") || raw_lower.contains("0x80040154") {
            return format!(
                "[com_invoke] COM 对象未注册（REGDB_E_CLASSNOTREG）。\
                 最常见原因：该 COM 对象是 32 位组件，需要用 arch=x86 参数。\
                 请重试并添加 arch: \"x86\"。\n详情：{}",
                raw_error
            );
        }
        if raw_lower.contains("0x80020009") || raw_lower.contains("disp_e_exception") {
            return format!(
                "[com_invoke] COM 方法调用抛出异常。请检查方法名称和参数是否正确。\n详情：{}",
                raw_error
            );
        }
        if raw_lower.contains("0x80070005") || raw_lower.contains("e_accessdenied") {
            return format!(
                "[com_invoke] COM 对象访问被拒绝。可能需要管理员权限，或该对象不允许外部调用。\n详情：{}",
                raw_error
            );
        }
        if raw_lower.contains("progid") || raw_lower.contains("new-object") {
            return format!(
                "[com_invoke] 无法创建 COM 对象。请确认 ProgID 正确，软件已安装，\
                 并尝试 arch=x86（32位软件）。\n详情：{}",
                raw_error
            );
        }
    }

    // Generic fallback
    format!("[{}] 工具执行失败：{}", tool_name, raw_error)
}

fn is_structural_schema_error(raw_error: &str) -> bool {
    let lower = raw_error.to_lowercase();
    lower.contains("missing field")
        || lower.contains("missing required")
        || lower.contains("invalid type")
        || lower.contains("invalid value")
        || lower.contains("unknown field")
        || lower.contains("unknown variant")
        || lower.contains("did not match any variant")
        || lower.contains("no variant of enum")
        || lower.contains("additional properties are not allowed")
        || lower.contains("additionalproperties")
        || lower.contains("expected u")
        || lower.contains("expected i")
        || lower.contains("expected a string")
        || lower.contains("expected a boolean")
        || lower.contains("expected an array")
        || lower.contains("expected a map")
        || lower.contains("expected struct")
}

fn maybe_schema_correction_envelope(
    registry: &crate::agent::tool::ToolRegistry,
    tool_name: &str,
    raw_error: &str,
) -> Option<String> {
    if !is_structural_schema_error(raw_error) {
        return None;
    }
    let tool_def = registry.to_tool_defs_for(tool_name, crate::agent::tool::ToolDefMode::Full)?;
    let full_schema_json = serde_json::to_string(&tool_def.input_schema).ok()?;
    Some(format!(
        "[schema_correction tool={}]\n{}\n[/schema_correction]",
        tool_name, full_schema_json
    ))
}

fn decorate_tool_failure_for_agent(
    tool_name: &str,
    input: &serde_json::Value,
    content: &str,
    is_error: bool,
) -> String {
    if !is_error || content.contains("[ConstraintViolation]") {
        return content.to_string();
    }

    let lower = content.to_lowercase();
    let looks_like_constraint = lower.contains("missing required parameter")
        || lower.contains(" requires ")
        || lower.contains("requires '")
        || lower.contains("requires \"")
        || lower.contains("unknown action")
        || lower.contains("not configured")
        || lower.contains("tool is disabled")
        || lower.contains("working directory does not exist")
        || lower.contains("file not found")
        || lower.contains("path_a not found")
        || lower.contains("path_b not found")
        || lower.contains("too large")
        || lower.contains("permission denied")
        || lower.contains("access is denied")
        || lower.contains("denied by policy");

    if !looks_like_constraint {
        return content.to_string();
    }

    let input_preview = serde_json::to_string(input).unwrap_or_default();
    let mut suggestions: Vec<&str> = Vec::new();
    if lower.contains("missing required parameter")
        || lower.contains(" requires ")
        || lower.contains("requires '")
        || lower.contains("requires \"")
    {
        suggestions.push("补齐工具要求的必填参数后重试");
    }
    if lower.contains("not configured") || lower.contains("tool is disabled") {
        suggestions.push("先在 Settings 中启用或配置该工具");
    }
    if lower.contains("working directory does not exist")
        || lower.contains("file not found")
        || lower.contains("path_a not found")
        || lower.contains("path_b not found")
    {
        suggestions.push("先确认路径存在，必要时先列目录或创建目标");
    }
    if lower.contains("too large") {
        suggestions.push("改用 offset/limit、分页、分块或更小范围参数");
    }
    if lower.contains("permission denied")
        || lower.contains("access is denied")
        || lower.contains("denied by policy")
    {
        suggestions.push("改用允许的路径/命令，或请求用户确认后再重试");
    }
    if suggestions.is_empty() {
        suggestions.push("修正输入或先满足前置条件后再重试");
    }

    let mut deduped = HashSet::new();
    let suggestion_text = suggestions
        .into_iter()
        .filter(|item| deduped.insert(*item))
        .collect::<Vec<_>>()
        .join("；");

    format!(
        "[ConstraintViolation] 工具 `{}` 本次调用未生效。请不要重复相同调用，先按提示调整后再试。\n建议：{}\n输入：{}\n原始结果：{}",
        tool_name, suggestion_text, input_preview, content
    )
}

fn truncate_str(s: &str, max: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max {
        s.to_string()
    } else {
        let end = s.char_indices().nth(max).map(|(i, _)| i).unwrap_or(s.len());
        format!("{}...", &s[..end])
    }
}

fn summarize_tool_input(tool_name: &str, input: &serde_json::Value) -> String {
    if tool_name == "browser" {
        let action = input["action"].as_str().unwrap_or("unknown");
        let mut parts = vec![format!("action={}", action)];
        if let Some(v) = input["url"].as_str() {
            parts.push(format!("url={}", v));
        }
        if let Some(v) = input["selector"].as_str() {
            parts.push(format!("selector={}", v));
        }
        if let Some(v) = input["tab_id"].as_str() {
            parts.push(format!("tab_id={}", v));
        }
        if let Some(v) = input["wait_condition"].as_str() {
            parts.push(format!("wait_condition={}", v));
        }
        return parts.join(", ");
    }
    input.to_string()
}

/// Generate a short human-readable label for the audit log's "action" column.
/// Each tool has a primary identifying field; fall back to the tool name itself.
fn audit_action_label(tool_name: &str, input: &serde_json::Value) -> String {
    fn truncate(s: &str, n: usize) -> String {
        if s.chars().count() <= n {
            s.to_string()
        } else {
            let t: String = s.chars().take(n).collect();
            format!("{}…", t)
        }
    }

    match tool_name {
        "shell" | "powershell" => {
            let cmd = input["command"].as_str().unwrap_or("");
            truncate(cmd, 60)
        }
        "powershell_query" => {
            let cmd = input["command"]
                .as_str()
                .or_else(|| input["query"].as_str())
                .unwrap_or("");
            truncate(cmd, 60)
        }
        "file_read" => {
            let path = input["path"].as_str().unwrap_or("");
            format!("read {}", truncate(path, 55))
        }
        "file_write" => {
            let path = input["path"].as_str().unwrap_or("");
            format!("write {}", truncate(path, 54))
        }
        "web_search" => {
            let q = input["query"].as_str().unwrap_or("");
            truncate(q, 60)
        }
        "browser" => {
            let action = input["action"].as_str().unwrap_or("?");
            if let Some(url) = input["url"].as_str() {
                format!("{} {}", action, truncate(url, 50))
            } else if let Some(sel) = input["selector"].as_str() {
                format!("{} {}", action, truncate(sel, 50))
            } else {
                action.to_string()
            }
        }
        "screen_capture" => input["mode"].as_str().unwrap_or("fullscreen").to_string(),
        "uia" => {
            let action = input["action"].as_str().unwrap_or("");
            if let Some(name) = input["name"].as_str() {
                format!("{} {}", action, truncate(name, 50))
            } else {
                action.to_string()
            }
        }
        "wmi" => {
            let q = input["query"].as_str().unwrap_or("");
            truncate(q, 60)
        }
        "com" => {
            let prog = input["prog_id"].as_str().unwrap_or("");
            let method = input["method"].as_str().unwrap_or("");
            if prog.is_empty() {
                method.to_string()
            } else {
                format!("{}.{}", prog, method)
            }
        }
        "office" => {
            let action = input["action"].as_str().unwrap_or("");
            let path = input["path"].as_str().unwrap_or("");
            format!("{} {}", action, truncate(path, 50))
        }
        _ => {
            // Generic: find the first non-empty string value
            if let Some(obj) = input.as_object() {
                for (_, v) in obj.iter().take(3) {
                    if let Some(s) = v.as_str() {
                        if !s.is_empty() {
                            return truncate(s, 60);
                        }
                    }
                }
            }
            tool_name.to_string()
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::{
        build_request_messages, build_request_view_messages, compact_summarise,
        compact_trim_tool_results, confirm_flags_handle, is_structural_schema_error,
        maybe_schema_correction_envelope, serialize_tool_results_with_receipts, AgentEvent,
        AgentLoop, CTX_KEEP_RECENT_TOOL_CARRIERS, CTX_PRESERVE_RECENT_TURNS, CTX_TRIM_HEAD,
        CTX_TRIM_TAIL, SUMMARY_KEEP_RECENT_RATIO,
    };
    use crate::agent::tool::{Tool, ToolContext, ToolRegistry, ToolSettings};
    use crate::llm::{ContentBlock, LlmChunk, LlmMessage, LlmRequest, LlmResponse, MessageContent};
    use crate::policy::PolicyGate;
    use anyhow::Result;
    use async_trait::async_trait;
    use serde_json::{json, Value};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::{borrow::Cow, collections::HashMap, path::PathBuf, sync::Arc};
    use tokio::sync::mpsc;

    fn tool_round(id: &str, args: Value) -> Vec<LlmMessage> {
        vec![
            LlmMessage {
                role: "assistant".into(),
                content: MessageContent::Blocks(vec![ContentBlock::ToolUse {
                    id: id.into(),
                    name: "read_file".into(),
                    input: args,
                }]),
            },
            LlmMessage {
                role: "user".into(),
                content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                    tool_use_id: id.into(),
                    content: "same".into(),
                    is_error: false,
                }]),
            },
        ]
    }

    #[test]
    fn repeated_identical_tool_rounds_collapse_to_the_last_one() {
        let mut msgs = vec![LlmMessage {
            role: "user".into(),
            content: MessageContent::text("go"),
        }];
        for n in 0..4 {
            msgs.extend(tool_round(&format!("a{n}"), json!({"path": "x"})));
        }
        msgs.extend(tool_round("b", json!({"path": "y"})));
        let out = super::collapse_repeated_tool_rounds(msgs);
        // user + 1 collapsed round + the different round
        assert_eq!(out.len(), 5);
        let MessageContent::Blocks(b) = &out[2].content else { panic!() };
        let ContentBlock::ToolResult { content, tool_use_id, .. } = &b[0] else { panic!() };
        assert_eq!(tool_use_id, "a3");
        assert!(content.contains("4 times in a row"));
    }

    fn fresh_detector() -> super::LoopDetectorState {
        super::LoopDetectorState {
            history: Vec::new(),
            consecutive_vision_only_turns: 0,
            warned: HashMap::new(),
        }
    }

    fn record_run(d: &mut super::LoopDetectorState, input: &Value, n: u32) {
        let content = format!("tests {n}ms\nfail 5\n[recovery_v1]\n{{\"operation_id\":\"op{n}\"}}");
        d.record(
            "code_run",
            input,
            super::stable_hash_result(&content),
            super::stable_hash_normalized(&content),
        );
        // An edit between runs keeps the consecutive-repeat nudge out of the way.
        d.record("file_edit", &json!({"n": n}), u64::from(n), u64::from(n));
    }

    #[test]
    fn code_run_with_same_outcome_warns_then_blocks() {
        let input = json!({"command": "node --test"});
        let batch = vec![("id".to_string(), "code_run".to_string(), input.clone())];
        let mut d = fresh_detector();
        for n in 0..3 {
            record_run(&mut d, &input, n);
        }
        assert_eq!(d.detect("code_run", &input, &batch).level, super::LoopLevel::Ok);
        record_run(&mut d, &input, 3);
        assert_eq!(d.detect("code_run", &input, &batch).level, super::LoopLevel::Warning);
        for n in 4..8 {
            record_run(&mut d, &input, n);
        }
        assert_eq!(d.detect("code_run", &input, &batch).level, super::LoopLevel::Critical);
    }

    #[test]
    fn ignored_warnings_escalate_to_a_block() {
        let input = json!({"command": "node --test"});
        let batch = vec![("id".to_string(), "code_run".to_string(), input.clone())];
        let mut d = fresh_detector();
        for n in 0..4 {
            record_run(&mut d, &input, n);
        }
        for _ in 0..3 {
            assert_eq!(d.detect("code_run", &input, &batch).level, super::LoopLevel::Warning);
        }
        assert_eq!(d.detect("code_run", &input, &batch).level, super::LoopLevel::Critical);
    }

    #[test]
    fn history_seeds_the_detector_for_a_new_turn() {
        let input = json!({"command": "node --test"});
        let mut msgs = Vec::new();
        for n in 0..5 {
            msgs.extend(tool_round(&format!("c{n}"), input.clone()));
        }
        // tool_round uses `read_file`; seeding keeps whatever tool name was used.
        let mut d = fresh_detector();
        d.seed_from_messages(&msgs);
        assert_eq!(d.history.len(), 5);
    }


    #[test]
    fn different_arguments_are_not_collapsed() {
        let mut msgs = Vec::new();
        msgs.extend(tool_round("a", json!({"path": "x"})));
        msgs.extend(tool_round("b", json!({"path": "y"})));
        assert_eq!(super::collapse_repeated_tool_rounds(msgs).len(), 4);
    }

    #[test]
    fn retry_candidates_and_backoff_are_hard_bounded() {
        let fallbacks = vec!["primary", "one", "one", "two", "three", "four"]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        assert_eq!(
            super::bounded_model_candidates("primary".into(), &fallbacks),
            vec!["primary", "one", "two", "three"]
        );
        assert_eq!(super::retry_backoff(0).as_secs(), 1);
        assert_eq!(super::retry_backoff(1).as_secs(), 2);
        assert_eq!(super::retry_backoff(u32::MAX).as_secs(), 8);
    }

    struct RetryPolicyClient {
        error: &'static str,
        calls: Arc<AtomicUsize>,
        cancel_on_call: Option<Arc<AtomicBool>>,
    }
    #[async_trait]
    impl crate::llm::LlmClient for RetryPolicyClient {
        async fn stream(&self, _req: LlmRequest, _tx: mpsc::Sender<LlmChunk>) -> Result<()> {
            unreachable!("retry fixture uses complete")
        }
        async fn complete(&self, _req: LlmRequest) -> Result<LlmResponse> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if let Some(cancel) = &self.cancel_on_call {
                cancel.store(true, Ordering::SeqCst);
            }
            Err(anyhow::anyhow!(self.error))
        }
    }
    async fn retry_fixture(error: &'static str, cancel_on_call: bool) -> usize {
        retry_fixture_inner(error, cancel_on_call, false).await
    }
    async fn retry_fixture_inner(
        error: &'static str,
        cancel_on_call: bool,
        cancel_during_backoff: bool,
    ) -> usize {
        retry_fixture_timed(error, cancel_on_call, cancel_during_backoff)
            .await
            .0
    }
    async fn retry_fixture_timed(
        error: &'static str,
        cancel_on_call: bool,
        cancel_during_backoff: bool,
    ) -> (usize, std::time::Duration) {
        let calls = Arc::new(AtomicUsize::new(0));
        let cancel = Arc::new(AtomicBool::new(false));
        let mut agent = tier_agent(
            RetryPolicyClient {
                error,
                calls: calls.clone(),
                cancel_on_call: cancel_on_call.then(|| cancel.clone()),
            },
            None,
            crate::agent::harness::LayeredBudget::from_context_window(
                TIER_TEST_CONTEXT_WINDOW,
                TIER_TEST_MAX_TOKENS,
            ),
        );
        agent.same_model_transient_retries = u32::MAX;
        agent.fallback_models = (0..50).map(|i| format!("fallback-{i}")).collect();
        let delayed_cancel = cancel_during_backoff.then(|| {
            let calls = calls.clone();
            let cancel = cancel.clone();
            tokio::spawn(async move {
                while calls.load(Ordering::SeqCst) == 0 {
                    tokio::task::yield_now().await;
                }
                // The immediate fake failure has entered its one-second backoff.
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                cancel.store(true, Ordering::SeqCst);
            })
        });
        let (tx, mut rx) = mpsc::channel(64);
        let drain = tokio::spawn(async move { while rx.recv().await.is_some() {} });
        let started = std::time::Instant::now();
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            agent.run(
                vec![make_text_msg("user", "retry fixture")],
                tx,
                cancel.clone(),
                tier_tool_context("retry-fixture".into()),
            ),
        )
        .await
        .expect("bounded retry fixture must finish");
        let elapsed = started.elapsed();
        if !cancel_on_call && !cancel_during_backoff {
            assert!(result.is_err());
        }
        drain.await.unwrap();
        if let Some(delayed_cancel) = delayed_cancel {
            delayed_cancel.await.unwrap();
        }
        (calls.load(Ordering::SeqCst), elapsed)
    }
    #[tokio::test]
    async fn permanent_mixed_errors_never_retry_or_fallback() {
        assert_eq!(
            retry_fixture(
                "OpenAI API error 401 Unauthorized: connection timeout context length exceeded",
                false
            )
            .await,
            1
        );
        assert_eq!(
            retry_fixture(
                "OpenAI API error 429: insufficient_quota connection timeout",
                false
            )
            .await,
            1
        );
    }
    #[tokio::test]
    async fn fallback_candidates_and_cancel_are_bounded_in_actual_loop() {
        assert_eq!(
            retry_fixture("OpenAI API error 429: rate_limit", false).await,
            4
        );
        assert_eq!(retry_fixture("connection timeout", true).await, 1);
    }

    #[tokio::test]
    async fn extreme_retry_count_stops_after_five_real_attempts() {
        assert_eq!(retry_fixture("connection timeout", false).await, 5);
    }
    #[tokio::test]
    async fn cancellation_during_real_backoff_starts_no_second_request() {
        let (calls, elapsed) = retry_fixture_timed("connection timeout", false, true).await;
        assert_eq!(calls, 1);
        // Lower bound: cancellation is raised >=50ms after the first request, so a run that
        // never entered backoff (finishing immediately) is rejected.
        assert!(
            elapsed >= std::time::Duration::from_millis(50),
            "run finished before the backoff window was entered: {elapsed:?}"
        );
        // Upper bound: the first real backoff is one second; finishing under 800ms shows the
        // wait was interrupted by cancellation rather than slept out.
        assert!(
            elapsed < std::time::Duration::from_millis(800),
            "cancelled backoff took {elapsed:?}"
        );
    }
    struct CorrectionBudgetClient {
        calls: Arc<AtomicUsize>,
    }
    #[async_trait]
    impl crate::llm::LlmClient for CorrectionBudgetClient {
        async fn stream(&self, _req: LlmRequest, _tx: mpsc::Sender<LlmChunk>) -> Result<()> {
            unreachable!("budget fixture uses complete")
        }
        async fn complete(&self, _req: LlmRequest) -> Result<LlmResponse> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            match call {
                1..=3 | 9..=11 => Err(anyhow::anyhow!("OpenAI API error 429: rate_limit")),
                8 => Err(anyhow::anyhow!("tool_args_invalid: incomplete JSON")),
                _ => Err(anyhow::anyhow!("connection timeout")),
            }
        }
    }
    #[tokio::test]
    async fn correction_shares_twelve_primary_call_budget() {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut agent = tier_agent(
            CorrectionBudgetClient {
                calls: calls.clone(),
            },
            None,
            crate::agent::harness::LayeredBudget::from_context_window(
                TIER_TEST_CONTEXT_WINDOW,
                TIER_TEST_MAX_TOKENS,
            ),
        );
        agent.same_model_transient_retries = u32::MAX;
        agent.fallback_models = vec!["one".into(), "two".into(), "three".into()];
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, mut rx) = mpsc::channel(64);
        let drain = tokio::spawn(async move {
            let mut events = Vec::new();
            while let Some(event) = rx.recv().await {
                events.push(event);
            }
            events
        });
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            agent.run(
                vec![make_text_msg("user", "bounded correction")],
                tx,
                cancel,
                tier_tool_context("correction-budget".into()),
            ),
        )
        .await
        .expect("correction budget must terminate");
        assert!(result.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 12);
        let events = drain.await.unwrap();
        assert_eq!(events.iter().filter(|event| matches!(event, AgentEvent::Notice { code: Some(code), .. } if code == "tool_args_invalid")).count(), 1);
        assert!(!events
            .iter()
            .any(|event| matches!(event, AgentEvent::ToolStart { .. })));
    }

    // ── Mock LLM clients ──────────────────────────────────────────────────────

    /// Returns a fixed summary string — simulates a successful LLM summarisation call.
    struct MockLlmClient {
        response: String,
    }

    impl MockLlmClient {
        fn new(response: impl Into<String>) -> Self {
            Self {
                response: response.into(),
            }
        }
    }

    #[async_trait]
    impl crate::llm::LlmClient for MockLlmClient {
        async fn stream(
            &self,
            _req: LlmRequest,
            _tx: tokio::sync::mpsc::Sender<LlmChunk>,
        ) -> Result<()> {
            Ok(())
        }

        async fn complete(&self, _req: LlmRequest) -> Result<LlmResponse> {
            Ok(LlmResponse {
                content: self.response.clone(),
                tool_calls: vec![],
                input_tokens: 10,
                output_tokens: 10,
            })
        }
    }

    /// Always returns an error — simulates a failed LLM call.
    struct FailingLlmClient;

    #[async_trait]
    impl crate::llm::LlmClient for FailingLlmClient {
        async fn stream(
            &self,
            _req: LlmRequest,
            _tx: tokio::sync::mpsc::Sender<LlmChunk>,
        ) -> Result<()> {
            Ok(())
        }

        async fn complete(&self, _req: LlmRequest) -> Result<LlmResponse> {
            Err(anyhow::anyhow!("simulated LLM failure"))
        }
    }

    struct StreamingCancelClient {
        cancel: Arc<AtomicBool>,
    }

    #[async_trait]
    impl crate::llm::LlmClient for StreamingCancelClient {
        async fn stream(
            &self,
            _req: LlmRequest,
            tx: tokio::sync::mpsc::Sender<LlmChunk>,
        ) -> Result<()> {
            tx.send(LlmChunk::TextDelta("partial answer".into()))
                .await
                .unwrap();
            self.cancel.store(true, Ordering::SeqCst);
            std::future::pending::<()>().await;
            Ok(())
        }

        async fn complete(&self, _req: LlmRequest) -> Result<LlmResponse> {
            unreachable!("streaming regression test should not call complete")
        }
    }

    struct SchemaTool;

    struct CompletionFixtureTool {
        started: Arc<tokio::sync::Notify>,
        pending: bool,
    }
    #[async_trait]
    impl Tool for CompletionFixtureTool {
        fn name(&self) -> &str {
            "completion_fixture"
        }
        fn description(&self) -> &str {
            "controlled completion fixture"
        }
        fn input_schema(&self) -> Value {
            json!({"type":"object"})
        }
        async fn call(&self, _: Value, _: &ToolContext) -> Result<crate::agent::tool::ToolResult> {
            self.started.notify_one();
            if self.pending {
                std::future::pending::<()>().await;
            }
            Err(anyhow::anyhow!("normal tool failure"))
        }
    }
    #[tokio::test]
    async fn dropped_generic_invocation_is_unconfirmed_even_without_process_worker() {
        let ctx = tier_tool_context("completion".into());
        let registry = ctx.owned_completion();
        let started = Arc::new(tokio::sync::Notify::new());
        let tool = CompletionFixtureTool {
            started: started.clone(),
            pending: true,
        };
        let mut call = Box::pin(super::call_with_completion(&tool, json!({}), &ctx));
        tokio::select! { result = &mut call => panic!("pending tool unexpectedly returned: {result:?}"), _ = started.notified() => {} }
        ctx.cancel.store(true, Ordering::SeqCst);
        drop(call);
        assert_eq!(
            registry.drain(std::time::Duration::ZERO).await,
            crate::agent::tool_completion::DrainOutcome::Failed {
                active: 0,
                failures: 1
            }
        );
    }
    #[tokio::test]
    async fn normal_tool_error_completes_invocation_and_cancelled_context_never_calls() {
        let ctx = tier_tool_context("completion".into());
        let registry = ctx.owned_completion();
        let tool = CompletionFixtureTool {
            started: Arc::new(tokio::sync::Notify::new()),
            pending: false,
        };
        assert!(super::call_with_completion(&tool, json!({}), &ctx)
            .await
            .is_err());
        assert_eq!(
            registry.drain(std::time::Duration::ZERO).await,
            crate::agent::tool_completion::DrainOutcome::Complete
        );
        let other = tier_tool_context("cancelled".into());
        other.cancel.store(true, Ordering::SeqCst);
        let pending = CompletionFixtureTool {
            started: Arc::new(tokio::sync::Notify::new()),
            pending: true,
        };
        assert!(super::call_with_completion(&pending, json!({}), &other)
            .await
            .is_err());
        assert_eq!(
            other
                .owned_completion()
                .drain(std::time::Duration::ZERO)
                .await,
            crate::agent::tool_completion::DrainOutcome::Complete
        );
    }

    /// Mimics the sandboxed shell: on cancel it finishes its own teardown and returns
    /// a normal result instead of being dropped.
    struct CooperativeTeardownTool;
    #[async_trait]
    impl Tool for CooperativeTeardownTool {
        fn name(&self) -> &str {
            "cooperative_teardown"
        }
        fn description(&self) -> &str {
            "fixture that tears down after cancel"
        }
        fn input_schema(&self) -> Value {
            json!({"type":"object"})
        }
        async fn call(&self, _: Value, ctx: &ToolContext) -> Result<crate::agent::tool::ToolResult> {
            while !ctx.cancel.load(Ordering::SeqCst) {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            tokio::time::sleep(std::time::Duration::from_millis(600)).await;
            Ok(crate::agent::tool::ToolResult::ok("torn down"))
        }
    }
    #[tokio::test]
    async fn cancelled_tool_that_tears_down_keeps_its_own_result() {
        let mut agent = tier_agent(
            CorrectionBudgetClient {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            None,
            crate::agent::harness::LayeredBudget::from_context_window(
                TIER_TEST_CONTEXT_WINDOW,
                TIER_TEST_MAX_TOKENS,
            ),
        );
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(CooperativeTeardownTool));
        agent.registry = Arc::new(registry);

        let cancel = Arc::new(AtomicBool::new(false));
        let mut ctx = tier_tool_context("cancel-grace".into());
        ctx.cancel = cancel.clone();
        ctx.bypass_permissions = true;
        let (event_tx, _event_rx) = mpsc::channel(64);

        let trigger = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            trigger.store(true, Ordering::SeqCst);
        });
        let blocks = agent
            .execute_single_tool(
                "t-cancel",
                "cooperative_teardown",
                &json!({}),
                &ctx,
                &event_tx,
                &cancel,
            )
            .await;
        let text = format!("{blocks:?}");
        assert!(text.contains("torn down"), "tool's own result must survive cancel: {text}");
        assert!(!text.contains("已被用户取消"), "cancel must not override a finished teardown: {text}");
    }

    /// A tool that ignores cancel entirely (no teardown). The loop must still stop
    /// waiting after the grace period and report the generic user-cancel result.
    struct NonTerminatingTool;
    #[async_trait]
    impl Tool for NonTerminatingTool {
        fn name(&self) -> &str {
            "non_terminating"
        }
        fn description(&self) -> &str {
            "fixture that never observes cancel"
        }
        fn input_schema(&self) -> Value {
            json!({"type":"object"})
        }
        async fn call(&self, _: Value, _ctx: &ToolContext) -> Result<crate::agent::tool::ToolResult> {
            tokio::time::sleep(std::time::Duration::from_secs(600)).await;
            Ok(crate::agent::tool::ToolResult::ok("never"))
        }
    }
    #[tokio::test]
    async fn uncooperative_tool_is_interrupted_after_grace() {
        let mut agent = tier_agent(
            CorrectionBudgetClient {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            None,
            crate::agent::harness::LayeredBudget::from_context_window(
                TIER_TEST_CONTEXT_WINDOW,
                TIER_TEST_MAX_TOKENS,
            ),
        );
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(NonTerminatingTool));
        agent.registry = Arc::new(registry);

        let cancel = Arc::new(AtomicBool::new(false));
        let mut ctx = tier_tool_context("cancel-interrupt".into());
        ctx.cancel = cancel.clone();
        ctx.bypass_permissions = true;
        let (event_tx, _event_rx) = mpsc::channel(64);

        let trigger = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            trigger.store(true, Ordering::SeqCst);
        });
        let started = std::time::Instant::now();
        let blocks = agent
            .execute_single_tool("t-interrupt", "non_terminating", &json!({}), &ctx, &event_tx, &cancel)
            .await;
        let elapsed = started.elapsed();
        let text = format!("{blocks:?}");
        assert!(text.contains("已被用户取消"), "uncooperative tool must be interrupted: {text}");
        assert!(
            elapsed >= std::time::Duration::from_secs(super::CANCEL_GRACE_SECS),
            "grace period must actually be waited before interrupting: {elapsed:?}"
        );
        assert!(
            elapsed < std::time::Duration::from_secs(super::CANCEL_GRACE_SECS + 5),
            "interrupt took {elapsed:?}: {text}"
        );
    }

    /// Real sandboxed shell, cancelled mid-run through the agent's execution path.
    /// The shell tears down its own AppContainer job and returns its cancel result; the
    /// watcher's generic "已被用户取消" must not replace it.
    #[cfg(windows)]
    #[tokio::test]
    async fn real_shell_cancel_keeps_shell_cancel_result_within_grace() {
        let temp = tempfile::tempdir().expect("create shell cancel root");
        let workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("create shell workspace");
        let mut agent = tier_agent(
            CorrectionBudgetClient {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            None,
            crate::agent::harness::LayeredBudget::from_context_window(
                TIER_TEST_CONTEXT_WINDOW,
                TIER_TEST_MAX_TOKENS,
            ),
        );
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(crate::tools::shell::ShellTool::default()));
        agent.registry = Arc::new(registry);

        let cancel = Arc::new(AtomicBool::new(false));
        let mut ctx = tier_tool_context("shell-cancel".into());
        ctx.workspace_root = workspace;
        ctx.cancel = cancel.clone();
        ctx.bypass_permissions = true;
        let (event_tx, _event_rx) = mpsc::channel(64);

        let trigger = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
            trigger.store(true, Ordering::SeqCst);
        });
        let started = std::time::Instant::now();
        let blocks = agent
            .execute_single_tool(
                "t-shell-cancel",
                "shell",
                &json!({
                    "command": "for /L %i in (1,1,200000000) do @rem",
                    "interpreter": "cmd",
                    "timeout": 120
                }),
                &ctx,
                &event_tx,
                &cancel,
            )
            .await;
        let elapsed = started.elapsed();
        let text = format!("{blocks:?}");
        assert!(text.contains("Command was cancelled"), "shell must report its own cancel: {text}");
        assert!(!text.contains("已被用户取消"), "watcher must not replace shell teardown: {text}");
        assert!(elapsed < std::time::Duration::from_secs(20), "cancel took {elapsed:?}: {text}");
    }

    /// Real sandboxed code_run, cancelled mid-run through the agent's execution path.
    /// Its own teardown returns "Sandbox command was cancelled"; the watcher's generic
    /// "已被用户取消" must not replace it.
    #[cfg(windows)]
    #[tokio::test]
    async fn real_code_run_cancel_keeps_its_own_result_within_grace() {
        let temp = tempfile::tempdir().expect("create code_run cancel root");
        let workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("create code_run workspace");
        let mut agent = tier_agent(
            CorrectionBudgetClient {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            None,
            crate::agent::harness::LayeredBudget::from_context_window(
                TIER_TEST_CONTEXT_WINDOW,
                TIER_TEST_MAX_TOKENS,
            ),
        );
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(crate::tools::code_run::CodeRunTool::default()));
        agent.registry = Arc::new(registry);

        let cancel = Arc::new(AtomicBool::new(false));
        let mut ctx = tier_tool_context("code-run-cancel".into());
        ctx.workspace_root = workspace;
        ctx.cancel = cancel.clone();
        ctx.bypass_permissions = true;
        let (event_tx, _event_rx) = mpsc::channel(64);

        let trigger = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
            trigger.store(true, Ordering::SeqCst);
        });
        let started = std::time::Instant::now();
        let blocks = agent
            .execute_single_tool(
                "t-code-run-cancel",
                "code_run",
                &json!({
                    "command": "for /L %i in (1,1,200000000) do @rem",
                    "timeout_secs": 120
                }),
                &ctx,
                &event_tx,
                &cancel,
            )
            .await;
        let elapsed = started.elapsed();
        let text = format!("{blocks:?}");
        assert!(text.contains("Sandbox command was cancelled"), "code_run must report its own cancel: {text}");
        assert!(!text.contains("已被用户取消"), "watcher must not replace code_run teardown: {text}");
        assert!(elapsed < std::time::Duration::from_secs(20), "cancel took {elapsed:?}: {text}");
    }

    #[async_trait]
    impl Tool for SchemaTool {
        fn name(&self) -> &str {
            "schema_tool"
        }

        fn description(&self) -> &str {
            "A tool used to validate schema-correction envelopes."
        }

        fn description_minimal(&self) -> Cow<'_, str> {
            Cow::Borrowed("validate schema correction")
        }

        fn input_schema(&self) -> Value {
            json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute path."
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["fast", "safe"],
                        "description": "Run mode."
                    }
                },
                "required": ["path", "mode"],
                "additionalProperties": false
            })
        }

        async fn call(
            &self,
            _input: Value,
            _ctx: &crate::agent::tool::ToolContext,
        ) -> Result<crate::agent::tool::ToolResult> {
            unreachable!("schema helper tool should not be executed in unit tests");
        }
    }

    // ── Test data helpers ─────────────────────────────────────────────────────

    fn make_text_msg(role: &str, text: &str) -> LlmMessage {
        LlmMessage {
            role: role.to_string(),
            content: MessageContent::Text(text.to_string()),
        }
    }

    const TIER_TEST_SYSTEM_PROMPT: &str = "adaptive context tier test";
    const TIER_TEST_CONTEXT_WINDOW: u32 = 8_192;
    const TIER_TEST_MAX_TOKENS: u32 = 512;

    struct TierRoutingSpyClient {
        semantic_calls: Arc<AtomicUsize>,
        main_calls: Arc<AtomicUsize>,
        semantic_prompts: Arc<std::sync::Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl crate::llm::LlmClient for TierRoutingSpyClient {
        async fn stream(
            &self,
            _req: LlmRequest,
            _tx: tokio::sync::mpsc::Sender<LlmChunk>,
        ) -> Result<()> {
            unreachable!("tier routing tests drive the non-streaming path")
        }

        async fn complete(&self, req: LlmRequest) -> Result<LlmResponse> {
            if req.system.is_none() {
                self.semantic_calls.fetch_add(1, Ordering::SeqCst);
                self.semantic_prompts.lock().unwrap().push(
                    req.messages
                        .iter()
                        .map(|message| message.content.as_text())
                        .collect::<Vec<_>>()
                        .join("\n"),
                );
                Ok(LlmResponse {
                    content: "tier rolling summary".into(),
                    tool_calls: vec![],
                    input_tokens: 37,
                    output_tokens: 11,
                })
            } else {
                self.main_calls.fetch_add(1, Ordering::SeqCst);
                Ok(LlmResponse {
                    content: "main response".into(),
                    tool_calls: vec![],
                    input_tokens: 17,
                    output_tokens: 5,
                })
            }
        }
    }

    struct RequestViewSpyClient {
        semantic_calls: Arc<AtomicUsize>,
        main_requests: Arc<std::sync::Mutex<Vec<LlmRequest>>>,
    }

    #[async_trait]
    impl crate::llm::LlmClient for RequestViewSpyClient {
        async fn stream(
            &self,
            _req: LlmRequest,
            _tx: tokio::sync::mpsc::Sender<LlmChunk>,
        ) -> Result<()> {
            unreachable!("request-view tests drive the non-streaming path")
        }

        async fn complete(&self, req: LlmRequest) -> Result<LlmResponse> {
            if req.system.is_none() {
                self.semantic_calls.fetch_add(1, Ordering::SeqCst);
                return Ok(LlmResponse {
                    content: "unexpected semantic summary".into(),
                    tool_calls: vec![],
                    input_tokens: 7,
                    output_tokens: 3,
                });
            }
            self.main_requests.lock().unwrap().push(req);
            Ok(LlmResponse {
                content: "main response".into(),
                tool_calls: vec![],
                input_tokens: 17,
                output_tokens: 5,
            })
        }
    }

    struct OverflowRecoverySpyClient {
        semantic_calls: Arc<AtomicUsize>,
        main_calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl crate::llm::LlmClient for OverflowRecoverySpyClient {
        async fn stream(
            &self,
            _req: LlmRequest,
            _tx: tokio::sync::mpsc::Sender<LlmChunk>,
        ) -> Result<()> {
            unreachable!("overflow tests drive the non-streaming path")
        }

        async fn complete(&self, req: LlmRequest) -> Result<LlmResponse> {
            if req.system.is_none() {
                self.semantic_calls.fetch_add(1, Ordering::SeqCst);
                return Ok(LlmResponse {
                    content: "overflow rolling summary".into(),
                    tool_calls: vec![],
                    input_tokens: 37,
                    output_tokens: 11,
                });
            }

            let call = self.main_calls.fetch_add(1, Ordering::SeqCst);
            if call == 0 {
                return Err(anyhow::anyhow!("context length exceeded"));
            }
            Ok(LlmResponse {
                content: "main response after overflow recovery".into(),
                tool_calls: vec![],
                input_tokens: 17,
                output_tokens: 5,
            })
        }
    }

    fn tier_history(
        budget: crate::agent::harness::LayeredBudget,
        target_percent: u32,
        expected_tier: crate::agent::harness::CompactionTier,
    ) -> Vec<LlmMessage> {
        let target = (u64::from(budget.total) * u64::from(target_percent) / 100) as usize;
        let mut history = Vec::new();
        for turn in 0..1_000 {
            let mut candidate = history.clone();
            candidate.push(make_text_msg("user", "current user request"));
            let request_view = build_request_messages(
                &candidate,
                &HashMap::new(),
                CTX_PRESERVE_RECENT_TURNS,
                CTX_KEEP_RECENT_TOOL_CARRIERS,
            );
            let estimated = crate::llm::estimate_request_input_tokens(
                &request_view,
                Some(TIER_TEST_SYSTEM_PROMPT),
                &[],
            );
            if estimated >= target {
                assert_eq!(
                    budget.classify(estimated.min(u32::MAX as usize) as u32),
                    expected_tier,
                    "history estimate {estimated} did not land in the requested tier"
                );
                return candidate;
            }
            let filler = format!("tier-message-{turn} {}", "payload ".repeat(96));
            history.push(make_text_msg("user", &filler));
            history.push(make_text_msg("assistant", &filler));
        }
        panic!("failed to calibrate test history to {expected_tier:?}");
    }

    fn tier_agent(
        client: impl crate::llm::LlmClient + 'static,
        db: Option<Arc<tokio::sync::Mutex<crate::store::Database>>>,
        budget: crate::agent::harness::LayeredBudget,
    ) -> AgentLoop {
        AgentLoop {
            client: Box::new(client),
            registry: Arc::new(ToolRegistry::new()),
            policy: Arc::new(PolicyGate::new(PathBuf::from("."))),
            system_prompt: TIER_TEST_SYSTEM_PROMPT.into(),
            model: "test-model".into(),
            max_tokens: TIER_TEST_MAX_TOKENS,
            context_window: TIER_TEST_CONTEXT_WINDOW,
            budget,
            fallback_models: vec![],
            db,
            plan_state: None,
            confirmation_responses: None,
            confirm_flags: confirm_flags_handle(false, false),
            vision_override: Some(false),
            vision_delegate: None,
            vision_model: String::new(),
            notification_rx: None,
            auto_compact_input_tokens_threshold: 0,
            enable_streaming: false,
            hooks: None,
            compaction_strategy: Arc::new(super::DefaultCompaction),
            memory_plugin: None,
            context_manager: None,
            memory_retrieval_prompt: None,
            loop_strategy: None,
            same_model_transient_retries: 3,
        }
    }

    fn tier_tool_context(session_id: String) -> ToolContext {
        ToolContext {
            session_id,
            workspace_root: PathBuf::from("."),
            bypass_permissions: false,
            settings: Arc::new(ToolSettings::default()),
            max_iterations: Some(1),
            memory_owner_id: "piscis".into(),
            pool_session_id: None,
            tool_use_id: None,
            cancel: Arc::new(AtomicBool::new(false)),
            loop_halt: None,
        }
    }

    async fn run_tier_agent(
        agent: AgentLoop,
        messages: Vec<LlmMessage>,
        session_id: String,
    ) -> Vec<AgentEvent> {
        let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(64);
        let cancel = Arc::new(AtomicBool::new(false));
        agent
            .run(messages, event_tx, cancel, tier_tool_context(session_id))
            .await
            .expect("tier AgentLoop run");
        let mut events = Vec::new();
        while let Ok(event) = event_rx.try_recv() {
            events.push(event);
        }
        events
    }

    #[tokio::test]
    async fn proactive_micro_tier_skips_semantic_summariser() {
        let budget = crate::agent::harness::LayeredBudget::from_context_window(
            TIER_TEST_CONTEXT_WINDOW,
            TIER_TEST_MAX_TOKENS,
        )
        .with_tier_percents(65, 80, 95);
        let messages = tier_history(budget, 70, crate::agent::harness::CompactionTier::Micro);
        let db = Arc::new(tokio::sync::Mutex::new(
            crate::store::Database::open_in_memory().expect("in-memory db"),
        ));
        let session_id = db
            .lock()
            .await
            .create_session(Some("micro tier"))
            .expect("session")
            .id;
        let semantic_calls = Arc::new(AtomicUsize::new(0));
        let main_calls = Arc::new(AtomicUsize::new(0));
        let agent = tier_agent(
            TierRoutingSpyClient {
                semantic_calls: semantic_calls.clone(),
                main_calls: main_calls.clone(),
                semantic_prompts: Arc::new(std::sync::Mutex::new(Vec::new())),
            },
            Some(db.clone()),
            budget,
        );

        let events = run_tier_agent(agent, messages, session_id.clone()).await;

        assert_eq!(semantic_calls.load(Ordering::SeqCst), 0);
        assert_eq!(main_calls.load(Ordering::SeqCst), 1);
        let reported_trigger = events
            .iter()
            .find_map(|event| match event {
                AgentEvent::ContextUsage {
                    trigger_threshold, ..
                } => Some(*trigger_threshold),
                _ => None,
            })
            .expect("ContextUsage event");
        assert_eq!(reported_trigger, budget.trigger_micro);
        let persisted = db
            .lock()
            .await
            .get_session_context_state(&session_id)
            .expect("context state")
            .expect("persisted session");
        assert!(persisted.rolling_summary.is_empty());
        assert_eq!(persisted.rolling_summary_version, 0);
    }

    #[tokio::test]
    async fn proactive_auto_tier_summarises_once_and_persists_rolling_summary() {
        let budget = crate::agent::harness::LayeredBudget::from_context_window(
            TIER_TEST_CONTEXT_WINDOW,
            TIER_TEST_MAX_TOKENS,
        );
        let messages = tier_history(budget, 87, crate::agent::harness::CompactionTier::Auto);
        let db = Arc::new(tokio::sync::Mutex::new(
            crate::store::Database::open_in_memory().expect("in-memory db"),
        ));
        let session_id = db
            .lock()
            .await
            .create_session(Some("auto tier"))
            .expect("session")
            .id;
        let semantic_calls = Arc::new(AtomicUsize::new(0));
        let main_calls = Arc::new(AtomicUsize::new(0));
        let agent = tier_agent(
            TierRoutingSpyClient {
                semantic_calls: semantic_calls.clone(),
                main_calls: main_calls.clone(),
                semantic_prompts: Arc::new(std::sync::Mutex::new(Vec::new())),
            },
            Some(db.clone()),
            budget,
        );

        let _events = run_tier_agent(agent, messages, session_id.clone()).await;

        assert_eq!(semantic_calls.load(Ordering::SeqCst), 1);
        assert_eq!(main_calls.load(Ordering::SeqCst), 1);
        let persisted = db
            .lock()
            .await
            .get_session_context_state(&session_id)
            .expect("context state")
            .expect("persisted session");
        assert_eq!(persisted.rolling_summary, "tier rolling summary");
        assert_eq!(persisted.rolling_summary_version, 1);
    }

    #[tokio::test]
    async fn proactive_full_tier_uses_single_aggressive_thirty_percent_pass() {
        let budget = crate::agent::harness::LayeredBudget::from_context_window(
            TIER_TEST_CONTEXT_WINDOW,
            TIER_TEST_MAX_TOKENS,
        );
        let messages = tier_history(budget, 97, crate::agent::harness::CompactionTier::Full);
        let overhead =
            crate::llm::estimate_request_overhead_tokens(Some(TIER_TEST_SYSTEM_PROMPT), &[]);
        let message_budget = (budget.total as usize).saturating_sub(overhead);
        let split_for = |keep_tokens: usize| {
            let mut accumulated = 0usize;
            let mut split_idx = messages.len().saturating_sub(6);
            for (index, message) in messages.iter().enumerate().rev() {
                accumulated += crate::llm::estimate_message_tokens(message);
                if accumulated >= keep_tokens && index > 0 {
                    split_idx = index;
                    break;
                }
            }
            split_idx
        };
        let normal_split = split_for((message_budget as f64 * SUMMARY_KEEP_RECENT_RATIO) as usize);
        let aggressive_split =
            split_for((message_budget as f64 * SUMMARY_KEEP_RECENT_RATIO * 0.5) as usize);
        assert!(aggressive_split > normal_split);
        let aggressive_only_marker = format!("tier-message-{}", normal_split / 2);

        let semantic_calls = Arc::new(AtomicUsize::new(0));
        let main_calls = Arc::new(AtomicUsize::new(0));
        let semantic_prompts = Arc::new(std::sync::Mutex::new(Vec::new()));
        let agent = tier_agent(
            TierRoutingSpyClient {
                semantic_calls: semantic_calls.clone(),
                main_calls: main_calls.clone(),
                semantic_prompts: semantic_prompts.clone(),
            },
            None,
            budget,
        );

        let _events = run_tier_agent(agent, messages, "tier-full".into()).await;

        assert_eq!(semantic_calls.load(Ordering::SeqCst), 1);
        assert_eq!(main_calls.load(Ordering::SeqCst), 1);
        let prompts = semantic_prompts.lock().unwrap();
        assert!(
            prompts[0].contains(&aggressive_only_marker),
            "30% pass must summarise marker {aggressive_only_marker} that a 60% pass keeps"
        );
    }

    #[tokio::test]
    async fn overflow_recovery_forces_semantic_compaction_below_proactive_threshold() {
        let budget = crate::agent::harness::LayeredBudget::with_total(100_000);
        let messages = (0..8)
            .map(|index| make_text_msg("user", &format!("overflow history {index}")))
            .collect::<Vec<_>>();
        let estimated = crate::llm::estimate_request_input_tokens(
            &messages,
            Some(TIER_TEST_SYSTEM_PROMPT),
            &[],
        );
        assert_eq!(
            budget.classify(estimated as u32),
            crate::agent::harness::CompactionTier::None
        );
        let semantic_calls = Arc::new(AtomicUsize::new(0));
        let main_calls = Arc::new(AtomicUsize::new(0));
        let db = Arc::new(tokio::sync::Mutex::new(
            crate::store::Database::open_in_memory().expect("in-memory db"),
        ));
        let session_id = db
            .lock()
            .await
            .create_session(Some("overflow recovery"))
            .expect("session")
            .id;
        let agent = tier_agent(
            OverflowRecoverySpyClient {
                semantic_calls: semantic_calls.clone(),
                main_calls: main_calls.clone(),
            },
            Some(db.clone()),
            budget,
        );

        let _events = run_tier_agent(agent, messages, session_id.clone()).await;

        assert_eq!(main_calls.load(Ordering::SeqCst), 2);
        assert_eq!(semantic_calls.load(Ordering::SeqCst), 1);
        let persisted = db
            .lock()
            .await
            .get_session_context_state(&session_id)
            .expect("context state")
            .expect("persisted session");
        assert_eq!(persisted.rolling_summary, "overflow rolling summary");
        assert_eq!(persisted.rolling_summary_version, 1);
    }

    #[tokio::test]
    async fn micro_agent_wire_request_demotes_historical_results_and_caps_recent_result() {
        let budget = crate::agent::harness::LayeredBudget::with_total(100_000)
            .with_tier_percents(1, 90, 95)
            .with_max_tool_result_tokens(1_000);
        let mut messages = Vec::new();
        let mut original_old = String::new();
        for turn in 0..12 {
            let id = format!("history-call-{turn}");
            messages.push(make_text_msg("user", &format!("historical request {turn}")));
            messages.push(LlmMessage {
                role: "assistant".into(),
                content: MessageContent::Blocks(vec![
                    ContentBlock::Text {
                        text: format!("running historical tool {turn}"),
                    },
                    ContentBlock::ToolUse {
                        id: id.clone(),
                        name: "shell".into(),
                        input: json!({"command": format!("echo {turn}")}),
                    },
                ]),
            });
            let content = if turn == 11 {
                format!("RECENT_OVERSIZED_RESULT {}", "R".repeat(12_000))
            } else {
                format!("HISTORICAL_FULL_RESULT_{turn} {}", "H".repeat(3_000))
            };
            if turn == 0 {
                original_old = content.clone();
            }
            messages.push(make_tool_result_msg(&id, &content));
        }
        messages.push(make_text_msg("user", "CURRENT_USER_REQUEST_MUST_SURVIVE"));

        let semantic_calls = Arc::new(AtomicUsize::new(0));
        let main_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
        let agent = tier_agent(
            RequestViewSpyClient {
                semantic_calls: semantic_calls.clone(),
                main_requests: main_requests.clone(),
            },
            None,
            budget,
        );

        let events = run_tier_agent(agent, messages, "micro-wire".into()).await;

        assert_eq!(semantic_calls.load(Ordering::SeqCst), 0);
        let requests = main_requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        let wire_messages = &requests[0].messages;
        let tool_result_content = |wanted_id: &str| {
            wire_messages
                .iter()
                .find_map(|message| match &message.content {
                    MessageContent::Blocks(blocks) => blocks.iter().find_map(|block| match block {
                        ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            ..
                        } if tool_use_id == wanted_id => Some(content.as_str()),
                        _ => None,
                    }),
                    MessageContent::Text(_) => None,
                })
        };
        let old = tool_result_content("history-call-0").expect("historical result on wire");
        assert_ne!(old, original_old);
        assert!(old.contains("[recall:history-call-0]"));
        let recent =
            tool_result_content("history-call-11").expect("recent oversized result on wire");
        assert!(recent.contains("chars removed"));
        assert!(wire_messages.iter().any(|message| {
            matches!(&message.content, MessageContent::Text(text) if text == "CURRENT_USER_REQUEST_MUST_SURVIVE")
        }));
        let reported_estimate = events
            .iter()
            .find_map(|event| match event {
                AgentEvent::ContextUsage {
                    estimated_input_tokens,
                    ..
                } => Some(*estimated_input_tokens),
                _ => None,
            })
            .expect("ContextUsage event");
        let wire_estimate = crate::llm::estimate_request_input_tokens(
            wire_messages,
            requests[0].system.as_deref(),
            &requests[0].tools,
        );
        assert_eq!(reported_estimate as usize, wire_estimate);
    }

    async fn capture_token_capped_wire_result(
        full_content: &str,
        token_cap: usize,
        include_image: bool,
    ) -> (String, bool) {
        let tool_use_id = "token-cap-call";
        let mut result_blocks = vec![ContentBlock::ToolResult {
            tool_use_id: tool_use_id.into(),
            content: full_content.into(),
            is_error: false,
        }];
        if include_image {
            result_blocks.push(ContentBlock::Image {
                source: crate::llm::ImageSource {
                    source_type: "base64".into(),
                    media_type: "image/png".into(),
                    data: "aGVsbG8=".into(),
                },
            });
        }
        let messages = vec![
            make_text_msg("user", "inspect a token-capped tool result"),
            LlmMessage {
                role: "assistant".into(),
                content: MessageContent::Blocks(vec![ContentBlock::ToolUse {
                    id: tool_use_id.into(),
                    name: "shell".into(),
                    input: json!({"command": "emit large output"}),
                }]),
            },
            LlmMessage {
                role: "user".into(),
                content: MessageContent::Blocks(result_blocks),
            },
            make_text_msg("user", "CURRENT_TOKEN_CAP_REQUEST_MUST_SURVIVE"),
        ];
        let semantic_calls = Arc::new(AtomicUsize::new(0));
        let main_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut budget = crate::agent::harness::LayeredBudget::with_total(100_000);
        // LayeredBudget is a public runtime contract; direct field values are
        // valid even when builder convenience methods apply a UI-oriented clamp.
        budget.max_tool_result_tokens = token_cap as u32;
        let agent = tier_agent(
            RequestViewSpyClient {
                semantic_calls: semantic_calls.clone(),
                main_requests: main_requests.clone(),
            },
            None,
            budget,
        );

        let _events = run_tier_agent(agent, messages, "wire-token-cap".into()).await;

        assert_eq!(semantic_calls.load(Ordering::SeqCst), 0);
        let requests = main_requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        let blocks = requests[0]
            .messages
            .iter()
            .find_map(|message| match &message.content {
                MessageContent::Blocks(blocks)
                    if blocks.iter().any(|block| {
                        matches!(block, ContentBlock::ToolResult { tool_use_id: id, .. } if id == tool_use_id)
                    }) => Some(blocks),
                _ => None,
            })
            .expect("tool-result carrier on the main wire request");
        let content = blocks
            .iter()
            .find_map(|block| match block {
                ContentBlock::ToolResult { content, .. } => Some(content.clone()),
                _ => None,
            })
            .expect("tool result content");
        let has_image = blocks
            .iter()
            .any(|block| matches!(block, ContentBlock::Image { .. }));
        (content, has_image)
    }

    #[tokio::test]
    async fn ascii_tool_result_on_real_wire_respects_token_cap() {
        let token_cap = 256usize;
        let full_content = "ASCII tool output ".repeat(2_000);
        assert!(crate::llm::estimate_tokens(&full_content) > token_cap);

        let (wire_content, _) =
            capture_token_capped_wire_result(&full_content, token_cap, false).await;

        assert!(wire_content.contains("chars removed"));
        assert!(
            crate::llm::estimate_tokens(&wire_content) <= token_cap,
            "ASCII wire result used {} tokens for cap {token_cap}",
            crate::llm::estimate_tokens(&wire_content)
        );
    }

    #[tokio::test]
    async fn cjk_tool_result_on_real_wire_respects_token_cap() {
        let token_cap = 256usize;
        let full_content = "中文工具结果输出".repeat(2_000);
        assert!(crate::llm::estimate_tokens(&full_content) > token_cap);

        let (wire_content, _) =
            capture_token_capped_wire_result(&full_content, token_cap, false).await;

        assert!(wire_content.contains("chars removed"));
        assert!(
            crate::llm::estimate_tokens(&wire_content) <= token_cap,
            "CJK wire result used {} tokens for cap {token_cap}",
            crate::llm::estimate_tokens(&wire_content)
        );
    }

    #[tokio::test]
    async fn mixed_tool_result_image_carrier_on_real_wire_respects_token_cap() {
        let token_cap = 256usize;
        let full_content = "mixed ASCII 中文结果 12345 ".repeat(2_000);
        assert!(crate::llm::estimate_tokens(&full_content) > token_cap);

        let (wire_content, has_image) =
            capture_token_capped_wire_result(&full_content, token_cap, true).await;

        assert!(wire_content.contains("chars removed"));
        assert!(
            crate::llm::estimate_tokens(&wire_content) <= token_cap,
            "mixed wire result used {} tokens for cap {token_cap}",
            crate::llm::estimate_tokens(&wire_content)
        );
        assert!(has_image, "ToolResult/Image carrier must remain intact");
    }

    #[tokio::test]
    async fn cancelled_streaming_response_keeps_partial_assistant_message() {
        let cancel = Arc::new(AtomicBool::new(false));
        let agent = AgentLoop {
            client: Box::new(StreamingCancelClient {
                cancel: cancel.clone(),
            }),
            registry: Arc::new(ToolRegistry::new()),
            policy: Arc::new(PolicyGate::new(PathBuf::from("."))),
            system_prompt: String::new(),
            model: "test-model".into(),
            max_tokens: 1024,
            context_window: 8192,
            budget: crate::agent::harness::LayeredBudget::from_context_window(8192, 1024),
            fallback_models: vec![],
            db: None,
            plan_state: None,
            confirmation_responses: None,
            confirm_flags: confirm_flags_handle(false, false),
            vision_override: Some(false),
            vision_delegate: None,
            vision_model: String::new(),
            notification_rx: None,
            auto_compact_input_tokens_threshold: 0,
            enable_streaming: true,
            hooks: None,
            compaction_strategy: Arc::new(super::DefaultCompaction),
            memory_plugin: None,
            context_manager: None,
            memory_retrieval_prompt: None,
            loop_strategy: None,
            same_model_transient_retries: 3,
        };
        let (event_tx, _event_rx) = tokio::sync::mpsc::channel(16);
        let ctx = ToolContext {
            session_id: "cancel-stream-test".into(),
            workspace_root: PathBuf::from("."),
            bypass_permissions: false,
            settings: Arc::new(ToolSettings::default()),
            max_iterations: Some(1),
            memory_owner_id: "piscis".into(),
            pool_session_id: None,
            tool_use_id: None,
            cancel: cancel.clone(),
            loop_halt: None,
        };

        let (messages, _, _) = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            agent.run(vec![make_text_msg("user", "hello")], event_tx, cancel, ctx),
        )
        .await
        .expect("agent run should observe cancellation")
        .expect("cancelled run should return cleanly");

        assert!(messages.iter().any(|message| {
            message.role == "assistant" && message.content.as_text() == "partial answer"
        }));
    }

    /// Fails the first `complete()` call with a `tool_args_invalid`-classified
    /// error, then succeeds with plain text on the second — simulates a model
    /// that truncates a tool call once and then answers normally after the
    /// P0-2 corrective retry.
    struct CorrectiveRetryClient {
        calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl crate::llm::LlmClient for CorrectiveRetryClient {
        async fn stream(
            &self,
            _req: LlmRequest,
            _tx: tokio::sync::mpsc::Sender<LlmChunk>,
        ) -> Result<()> {
            unreachable!("this test drives the non-streaming path")
        }

        async fn complete(&self, _req: LlmRequest) -> Result<LlmResponse> {
            let n = self.calls.fetch_add(1, Ordering::SeqCst);
            if n == 0 {
                Err(anyhow::anyhow!(
                    "tool_args_invalid: failed to parse arguments for tool \"file_write\" \
                     (id=call_1, args_len=5): EOF while parsing a string"
                ))
            } else {
                Ok(LlmResponse {
                    content: "已完成".into(),
                    tool_calls: vec![],
                    input_tokens: 5,
                    output_tokens: 5,
                })
            }
        }
    }

    #[tokio::test]
    async fn tool_args_invalid_gets_one_corrective_retry_then_succeeds() {
        let calls = Arc::new(AtomicUsize::new(0));
        let cancel = Arc::new(AtomicBool::new(false));
        let agent = AgentLoop {
            client: Box::new(CorrectiveRetryClient {
                calls: calls.clone(),
            }),
            registry: Arc::new(ToolRegistry::new()),
            policy: Arc::new(PolicyGate::new(PathBuf::from("."))),
            system_prompt: String::new(),
            model: "test-model".into(),
            max_tokens: 1024,
            context_window: 8192,
            budget: crate::agent::harness::LayeredBudget::from_context_window(8192, 1024),
            fallback_models: vec![],
            db: None,
            plan_state: None,
            confirmation_responses: None,
            confirm_flags: confirm_flags_handle(false, false),
            vision_override: Some(false),
            vision_delegate: None,
            vision_model: String::new(),
            notification_rx: None,
            auto_compact_input_tokens_threshold: 0,
            enable_streaming: false,
            hooks: None,
            compaction_strategy: Arc::new(super::DefaultCompaction),
            memory_plugin: None,
            context_manager: None,
            memory_retrieval_prompt: None,
            loop_strategy: None,
            same_model_transient_retries: 3,
        };
        let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(32);
        let ctx = ToolContext {
            session_id: "tool-args-invalid-test".into(),
            workspace_root: PathBuf::from("."),
            bypass_permissions: false,
            settings: Arc::new(ToolSettings::default()),
            max_iterations: Some(2),
            memory_owner_id: "piscis".into(),
            pool_session_id: None,
            tool_use_id: None,
            cancel: cancel.clone(),
            loop_halt: None,
        };

        let events_task = tokio::spawn(async move {
            let mut events = Vec::new();
            while let Some(ev) = event_rx.recv().await {
                events.push(ev);
            }
            events
        });

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            agent.run(
                vec![make_text_msg("user", "写个文件")],
                event_tx,
                cancel,
                ctx,
            ),
        )
        .await
        .expect("agent run should not hang");
        result.expect("run should succeed after exactly one corrective retry");

        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "expected the primary call plus exactly one corrective retry"
        );

        let events = events_task.await.expect("event collector should not panic");
        let notice = events.iter().find_map(|e| match e {
            AgentEvent::Notice { message, code, .. } => Some((message.clone(), code.clone())),
            _ => None,
        });
        let (notice_msg, notice_code) =
            notice.expect("expected a non-terminal Notice event for the corrective retry");
        assert_eq!(notice_code, Some("tool_args_invalid".to_string()));
        assert!(notice_msg.contains("tool_args_invalid"));

        // The run ultimately succeeded — must NOT have emitted a terminal
        // Error for the transient tool_args_invalid failure that was retried.
        assert!(!events.iter().any(|e| matches!(e, AgentEvent::Error { .. })));
    }

    /// Fails `complete()` for the primary model with a `model_unavailable`-
    /// classified error, and only succeeds when called with `fallback_model`.
    struct FallbackModelClient {
        fallback_model: String,
    }

    #[async_trait]
    impl crate::llm::LlmClient for FallbackModelClient {
        async fn stream(
            &self,
            _req: LlmRequest,
            _tx: tokio::sync::mpsc::Sender<LlmChunk>,
        ) -> Result<()> {
            unreachable!("this test drives the non-streaming path")
        }

        async fn complete(&self, req: LlmRequest) -> Result<LlmResponse> {
            if req.model == self.fallback_model {
                Ok(LlmResponse {
                    content: "已用备选模型完成".into(),
                    tool_calls: vec![],
                    input_tokens: 5,
                    output_tokens: 5,
                })
            } else {
                Err(anyhow::anyhow!(
                    "OpenAI API error 404 Not Found: {{\"error\":{{\"message\":\"Model \\\"{}\\\" is not supported by any configured account in this group\",\"type\":\"model_not_found\"}}}}",
                    req.model
                ))
            }
        }
    }

    #[tokio::test]
    async fn primary_model_unavailable_falls_back_and_discloses_switch() {
        let cancel = Arc::new(AtomicBool::new(false));
        let fallback_model = "fallback-model".to_string();
        let agent = AgentLoop {
            client: Box::new(FallbackModelClient {
                fallback_model: fallback_model.clone(),
            }),
            registry: Arc::new(ToolRegistry::new()),
            policy: Arc::new(PolicyGate::new(PathBuf::from("."))),
            system_prompt: String::new(),
            model: "primary-model".into(),
            max_tokens: 1024,
            context_window: 8192,
            budget: crate::agent::harness::LayeredBudget::from_context_window(8192, 1024),
            fallback_models: vec![fallback_model.clone()],
            db: None,
            plan_state: None,
            confirmation_responses: None,
            confirm_flags: confirm_flags_handle(false, false),
            vision_override: Some(false),
            vision_delegate: None,
            vision_model: String::new(),
            notification_rx: None,
            auto_compact_input_tokens_threshold: 0,
            enable_streaming: false,
            hooks: None,
            compaction_strategy: Arc::new(super::DefaultCompaction),
            memory_plugin: None,
            context_manager: None,
            memory_retrieval_prompt: None,
            loop_strategy: None,
            same_model_transient_retries: 3,
        };
        let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(32);
        let ctx = ToolContext {
            session_id: "model-fallback-test".into(),
            workspace_root: PathBuf::from("."),
            bypass_permissions: false,
            settings: Arc::new(ToolSettings::default()),
            max_iterations: Some(1),
            memory_owner_id: "piscis".into(),
            pool_session_id: None,
            tool_use_id: None,
            cancel: cancel.clone(),
            loop_halt: None,
        };

        let events_task = tokio::spawn(async move {
            let mut events = Vec::new();
            while let Some(ev) = event_rx.recv().await {
                events.push(ev);
            }
            events
        });

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            agent.run(vec![make_text_msg("user", "hi")], event_tx, cancel, ctx),
        )
        .await
        .expect("agent run should not hang");
        result.expect("run should succeed via the fallback model");

        let events = events_task.await.expect("event collector should not panic");
        let notice = events.iter().find_map(|e| match e {
            AgentEvent::Notice {
                message,
                code,
                details,
            } => Some((message.clone(), code.clone(), details.clone())),
            _ => None,
        });
        let (notice_msg, notice_code, details) =
            notice.expect("expected a Notice disclosing the model_fallback switch");
        assert_eq!(notice_code, Some("model_fallback".to_string()));
        assert!(notice_msg.contains("primary-model"));
        assert!(notice_msg.contains(&fallback_model));
        let details = details.expect("model_fallback notice should carry from/to details");
        assert_eq!(details["from"], "primary-model");
        assert_eq!(details["to"], fallback_model);

        // A successful fallback is not a terminal failure.
        assert!(!events.iter().any(|e| matches!(e, AgentEvent::Error { .. })));
    }

    /// Assistant message with a ToolUse block (mirrors real DB tool_calls_json).
    fn make_tool_call_msg(tool_name: &str, input_json: &str) -> LlmMessage {
        LlmMessage {
            role: "assistant".to_string(),
            content: MessageContent::Blocks(vec![
                ContentBlock::Text {
                    text: format!("正在调用 {}...", tool_name),
                },
                ContentBlock::ToolUse {
                    id: format!("call_{}", tool_name),
                    name: tool_name.to_string(),
                    input: serde_json::from_str(input_json).unwrap_or(serde_json::Value::Null),
                },
            ]),
        }
    }

    /// User message with a ToolResult block (mirrors real DB tool_results_json).
    fn make_tool_result_msg(tool_use_id: &str, content: &str) -> LlmMessage {
        LlmMessage {
            role: "user".to_string(),
            content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                tool_use_id: tool_use_id.to_string(),
                content: content.to_string(),
                is_error: false,
            }]),
        }
    }

    /// User message with a large ToolResult of exactly `size_chars` characters.
    fn make_large_tool_result(size_chars: usize) -> LlmMessage {
        let content = "x".repeat(size_chars);
        make_tool_result_msg("call_large", &content)
    }

    /// Simulate a realistic agent session with `n_tool_rounds` tool call rounds.
    /// Structure mirrors what the real AgentLoop produces in memory:
    ///   user text → [assistant(ToolUse) → user(ToolResult)] × n_rounds → assistant text
    fn make_realistic_session(n_tool_rounds: usize) -> Vec<LlmMessage> {
        let mut msgs = vec![make_text_msg(
            "user",
            "请帮我在Tribon中移动配件位置，将管道支撑从坐标(100,200,300)移动到(150,250,350)",
        )];
        for i in 0..n_tool_rounds {
            // assistant calls a tool (shell, file_read, com_invoke, etc.)
            let tool_names = [
                "shell",
                "file_read",
                "com_invoke",
                "plan_todo",
                "file_write",
            ];
            let tool_name = tool_names[i % tool_names.len()];
            let input = match tool_name {
                "shell" => format!(
                    r#"{{"command":"python tribon_move.py --id {} --x 150 --y 250 --z 350"}}"#,
                    i
                ),
                "file_read" => format!(r#"{{"path":"C:\\Tribon\\project\\part_{}.xml"}}"#, i),
                "com_invoke" => format!(
                    r#"{{"prog_id":"Tribon.Application","method":"MoveComponent","args":[{},150,250,350]}}"#,
                    i
                ),
                "plan_todo" => format!(
                    r#"{{"merge":true,"todos":[{{"id":"step-{}","content":"移动配件{}","status":"completed"}}]}}"#,
                    i, i
                ),
                _ => format!(
                    r#"{{"path":"C:\\output\\result_{}.txt","content":"done"}}"#,
                    i
                ),
            };
            msgs.push(make_tool_call_msg(tool_name, &input));

            // tool result (realistic size: 200-2000 chars)
            let result_size = 200 + (i * 37) % 1800;
            let result_content = format!(
                "工具 {} 执行结果 (迭代 {}):\n{}\n退出码: 0",
                tool_name,
                i,
                "a".repeat(result_size)
            );
            msgs.push(make_tool_result_msg(
                &format!("call_{}", tool_name),
                &result_content,
            ));
        }
        msgs.push(make_text_msg(
            "assistant",
            "配件移动完成。已将管道支撑从(100,200,300)成功移动到(150,250,350)。",
        ));
        msgs
    }

    #[test]
    fn schema_error_classifier_matches_structural_errors_only() {
        assert!(is_structural_schema_error("missing field `path`"));
        assert!(is_structural_schema_error(
            "invalid type: integer `1`, expected a string"
        ));
        assert!(is_structural_schema_error(
            "unknown field `extra`, expected one of `path`, `mode`"
        ));
        assert!(!is_structural_schema_error("permission denied"));
        assert!(!is_structural_schema_error("exit code 1"));
    }

    #[test]
    fn schema_correction_envelope_includes_full_schema_json() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(SchemaTool));

        let envelope =
            maybe_schema_correction_envelope(&registry, "schema_tool", "missing field `mode`")
                .expect("structural schema error should produce envelope");

        assert!(envelope.starts_with("[schema_correction tool=schema_tool]\n"));
        assert!(envelope.contains("\"required\":[\"path\",\"mode\"]"));
        assert!(envelope.contains("\"additionalProperties\":false"));
        assert!(envelope.ends_with("\n[/schema_correction]"));
    }

    #[test]
    fn schema_correction_envelope_skips_non_structural_and_unknown_tool() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(SchemaTool));

        assert!(
            maybe_schema_correction_envelope(&registry, "schema_tool", "permission denied")
                .is_none()
        );
        assert!(maybe_schema_correction_envelope(
            &registry,
            "missing_tool",
            "missing field `path`"
        )
        .is_none());
    }

    #[test]
    fn schema_correction_envelope_survives_tool_result_roundtrip() {
        let content = "[schema_correction tool=schema_tool]\n{\"type\":\"object\",\"required\":[\"path\"]}\n[/schema_correction]";
        let block = ContentBlock::ToolResult {
            tool_use_id: "call_schema_tool".to_string(),
            content: content.to_string(),
            is_error: true,
        };
        let json = serde_json::to_string(&block).expect("tool_result should serialize");
        let decoded: ContentBlock =
            serde_json::from_str(&json).expect("tool_result should deserialize");
        match decoded {
            ContentBlock::ToolResult {
                content: restored, ..
            } => assert_eq!(restored, content),
            other => panic!("unexpected block: {other:?}"),
        }
    }

    // ── T1: Level-1 — small result not trimmed ────────────────────────────────

    #[test]
    fn t1_small_tool_result_not_trimmed() {
        let original = "x".repeat(1_000);
        let mut msgs = vec![make_tool_result_msg("call_1", &original)];
        let changed = compact_trim_tool_results(&mut msgs, 50_000);
        assert!(
            !changed,
            "should not trim a 1000-char result with limit=50000"
        );
        if let MessageContent::Blocks(ref blocks) = msgs[0].content {
            if let ContentBlock::ToolResult { content, .. } = &blocks[0] {
                assert_eq!(*content, original, "content should be unchanged");
            }
        }
    }

    // ── T2: Level-1 — oversized result trimmed ────────────────────────────────

    #[test]
    fn t2_large_tool_result_trimmed() {
        let mut msgs = vec![make_large_tool_result(100_000)];
        let changed = compact_trim_tool_results(&mut msgs, 10_000);
        assert!(changed, "should trim a 100000-char result with limit=10000");
        if let MessageContent::Blocks(ref blocks) = msgs[0].content {
            if let ContentBlock::ToolResult { content, .. } = &blocks[0] {
                assert!(
                    content.contains("chars removed"),
                    "trimmed content should contain 'chars removed' marker"
                );
                // Verify head and tail are preserved
                let head_check: String = "x".repeat(CTX_TRIM_HEAD);
                assert!(content.starts_with(&head_check), "head should be preserved");
                let tail_check: String = "x".repeat(CTX_TRIM_TAIL);
                assert!(content.ends_with(&tail_check), "tail should be preserved");
            }
        }
    }

    // ── T3: Level-1 — assistant messages not trimmed ─────────────────────────

    #[test]
    fn t3_assistant_tool_use_not_trimmed() {
        // assistant ToolUse messages should never be touched by Level-1
        let large_input = serde_json::json!({"command": "x".repeat(50_000)});
        let mut msgs = vec![LlmMessage {
            role: "assistant".to_string(),
            content: MessageContent::Blocks(vec![ContentBlock::ToolUse {
                id: "call_big".to_string(),
                name: "shell".to_string(),
                input: large_input.clone(),
            }]),
        }];
        let changed = compact_trim_tool_results(&mut msgs, 1_000);
        assert!(
            !changed,
            "assistant ToolUse should never be trimmed by Level-1"
        );
        if let MessageContent::Blocks(ref blocks) = msgs[0].content {
            if let ContentBlock::ToolUse { input, .. } = &blocks[0] {
                assert_eq!(*input, large_input, "ToolUse input should be unchanged");
            }
        }
    }

    // ── T4: Level-1 — mixed messages, only oversized user results trimmed ─────

    #[test]
    fn t4_mixed_messages_only_oversized_trimmed() {
        let limit = 5_000;

        let small = make_tool_result_msg("c1", &"a".repeat(500));
        let medium = make_tool_result_msg("c2", &"b".repeat((limit - 1) * 4));
        let large = make_large_tool_result((limit + 2_500) * 4);
        let assistant = make_tool_call_msg("shell", r#"{"command":"ls"}"#);

        let mut msgs = vec![small, medium, large, assistant];
        let original_small = "a".repeat(500);
        let original_medium = "b".repeat((limit - 1) * 4);

        let changed = compact_trim_tool_results(&mut msgs, limit);
        assert!(
            changed,
            "should report change because large result was trimmed"
        );

        // small: unchanged
        if let MessageContent::Blocks(ref b) = msgs[0].content {
            if let ContentBlock::ToolResult { content, .. } = &b[0] {
                assert_eq!(*content, original_small);
            }
        }
        // medium: unchanged (just under threshold)
        if let MessageContent::Blocks(ref b) = msgs[1].content {
            if let ContentBlock::ToolResult { content, .. } = &b[0] {
                assert_eq!(*content, original_medium);
            }
        }
        // large: trimmed
        if let MessageContent::Blocks(ref b) = msgs[2].content {
            if let ContentBlock::ToolResult { content, .. } = &b[0] {
                assert!(
                    content.contains("chars removed"),
                    "large result should be trimmed"
                );
            }
        }
        // assistant: unchanged
        assert_eq!(msgs[3].role, "assistant");
    }

    // ── T5: Level-2 — too few messages returns None ───────────────────────────

    #[tokio::test]
    async fn t5_too_few_messages_returns_none() {
        let client = MockLlmClient::new("摘要内容");
        let msgs = vec![make_text_msg("user", "只有一条消息")];
        let result = compact_summarise(msgs, 100_000, &client, "test-model", 1024, None).await;
        assert!(result.is_none(), "single message should return None");
    }

    // ── T6: Level-2 — all messages fit in keep_chars, returns None ────────────

    #[tokio::test]
    async fn t6_all_fit_in_budget_returns_none() {
        let client = MockLlmClient::new("摘要内容");
        let msgs = vec![
            make_text_msg("user", "短消息1"),
            make_text_msg("assistant", "短回复1"),
            make_text_msg("user", "短消息2"),
        ];
        // keep_chars=100000 >> total size of 3 short messages
        let result = compact_summarise(msgs, 100_000, &client, "test-model", 1024, None).await;
        assert!(
            result.is_none(),
            "all messages fit in budget, should return None"
        );
    }

    // ── T7: Level-2 — plain text messages compacted correctly ────────────────

    #[tokio::test]
    async fn t7_plain_text_messages_compacted() {
        let client =
            MockLlmClient::new("用户要求[移动配件]，智能体已完成[查询位置]，当前状态[待执行移动]");
        // 20 messages × ~500 chars each ≈ 10000 chars total
        let mut msgs: Vec<LlmMessage> = (0..20)
            .map(|i| {
                let role = if i % 2 == 0 { "user" } else { "assistant" };
                make_text_msg(role, &format!("消息内容 {}: {}", i, "x".repeat(500)))
            })
            .collect();
        // Ensure alternating roles start with user
        msgs[0] = make_text_msg("user", &format!("用户请求: {}", "x".repeat(500)));

        // keep_chars=2000 forces compaction of older messages
        let result =
            compact_summarise(msgs.clone(), 2_000, &client, "test-model", 1024, None).await;
        assert!(
            result.is_some(),
            "should compact when messages exceed keep_chars"
        );

        let compacted = result.unwrap();
        assert!(
            compacted.messages.len() < msgs.len(),
            "compacted messages ({}) should be fewer than original ({})",
            compacted.messages.len(),
            msgs.len()
        );

        // First message should be the summary
        let first_content = compacted.messages[0].content.as_text();
        assert!(
            first_content.contains("[会话滚动摘要]"),
            "first message should contain [会话滚动摘要], got: {}",
            &first_content[..first_content.len().min(100)]
        );
    }

    // ── T8: Level-2 — realistic session with tool calls compacted ────────────

    #[tokio::test]
    async fn t8_realistic_tool_call_session_compacted() {
        let summary_text =
            "用户要求[移动Tribon配件]，智能体已完成[调用shell脚本、读取XML文件、调用COM接口]，当前状态[验证移动结果]";
        let client = MockLlmClient::new(summary_text);

        // 30 rounds of tool calls — mirrors the real crash scenario
        let msgs = make_realistic_session(30);
        let original_len = msgs.len();

        // keep_chars=5000 forces compaction of the bulk of the history
        let result = compact_summarise(msgs, 5_000, &client, "deepseek-chat", 4096, None).await;
        assert!(result.is_some(), "30-round session should be compacted");

        let compacted = result.unwrap();
        assert!(
            compacted.messages.len() < original_len,
            "compacted ({}) should be fewer than original ({})",
            compacted.messages.len(),
            original_len
        );

        // Summary message should contain tool names (not empty)
        let summary_msg = &compacted.messages[0];
        let summary_content = summary_msg.content.as_text();
        assert!(
            summary_content.contains("[会话滚动摘要]"),
            "first message should be summary"
        );
        assert!(
            !summary_content.trim().is_empty(),
            "summary content should not be empty"
        );

        // The history_text sent to LLM should have contained tool call info.
        // We verify this indirectly: the mock returns our fixed summary, which
        // means compact_summarise successfully called the LLM (didn't short-circuit
        // due to empty history_text).
        assert!(
            summary_content.contains(summary_text),
            "summary should contain the mock LLM response"
        );
    }

    // ── T9: Level-2 — LLM failure returns None ───────────────────────────────

    #[tokio::test]
    async fn t9_llm_failure_returns_none() {
        let client = FailingLlmClient;
        let msgs = make_realistic_session(20);
        let result = compact_summarise(msgs, 1_000, &client, "test-model", 1024, None).await;
        assert!(
            result.is_none(),
            "LLM failure should return None from compact_summarise"
        );
    }

    #[tokio::test]
    async fn t9b_merges_existing_rolling_summary() {
        let client = MockLlmClient::new(
            "用户目标[整理上下文]；已完成工作[合并旧摘要与新历史]；当前状态[继续执行]；关键结果[src-tauri/src/agent/loop_.rs]",
        );
        let msgs = make_realistic_session(12);
        let result = compact_summarise(
            msgs,
            1_500,
            &client,
            "test-model",
            1024,
            Some("用户目标[旧目标]；已完成工作[旧工作]；当前状态[旧状态]"),
        )
        .await
        .expect("merged compaction");

        assert!(result.summary.contains("合并旧摘要与新历史"));
        assert!(result.messages[0]
            .content
            .as_text()
            .contains("[会话滚动摘要]"));
    }

    // ── T10: estimate_message_tokens handles all content types ───────────────

    #[test]
    fn t10_estimate_message_tokens_all_types() {
        use crate::llm::estimate_message_tokens;

        // Plain text
        let text_msg = make_text_msg("user", &"a".repeat(400));
        let text_tokens = estimate_message_tokens(&text_msg);
        assert!(
            text_tokens > 0,
            "plain text should have non-zero token estimate"
        );
        // 400 ASCII chars ÷ 4 = 100 tokens (max(1) applies)
        assert!(
            (90..=110).contains(&text_tokens),
            "400 ASCII chars should estimate ~100 tokens, got {}",
            text_tokens
        );

        // ToolUse (assistant) — previously returned 0 with as_text()
        let tool_call = make_tool_call_msg("shell", r#"{"command":"python move.py --x 150"}"#);
        let tool_call_tokens = estimate_message_tokens(&tool_call);
        assert!(
            tool_call_tokens > 0,
            "ToolUse message should have non-zero token estimate, got {}",
            tool_call_tokens
        );

        // ToolResult (user) — previously returned 0 with as_text()
        let tool_result = make_tool_result_msg("call_1", &"result content ".repeat(50));
        let tool_result_tokens = estimate_message_tokens(&tool_result);
        assert!(
            tool_result_tokens > 0,
            "ToolResult message should have non-zero token estimate, got {}",
            tool_result_tokens
        );

        // Large ToolResult should estimate more tokens than small one
        let small_result = make_tool_result_msg("c1", "short");
        let large_result = make_large_tool_result(10_000);
        assert!(
            estimate_message_tokens(&large_result) > estimate_message_tokens(&small_result),
            "larger tool result should estimate more tokens"
        );
    }

    // ── T11: 154-round crash scenario — split_idx keeps enough tail messages ──

    #[tokio::test]
    async fn t11_154_round_crash_scenario_split_idx() {
        let client = MockLlmClient::new(
            "用户要求[监控路径合规性]，智能体已完成[检查前端和后端代码]，当前状态[待完成报告生成]",
        );

        // Reproduce the exact crash scenario from the logs:
        // deepseek-chat, max_tokens=4096 → budget=49000, keep_tokens=29400
        let msgs = make_realistic_session(76); // 76 rounds ≈ 154 messages
        let original_len = msgs.len();

        // keep_tokens matching the real crash: budget(49000) × 0.60 = 29400
        let keep_tokens = 29_400usize;
        let result =
            compact_summarise(msgs, keep_tokens, &client, "deepseek-chat", 4096, None).await;

        assert!(result.is_some(), "154-message session should be compacted");
        let compacted = result.unwrap();

        // Key regression check: must keep more than just 2 tail messages.
        // Before the fix, split_idx defaulted to len-2, leaving only 3 messages total.
        // After the fix, split_idx is computed from actual content sizes.
        let tail_count = compacted.messages.len() - 1; // subtract the summary message
        assert!(
            tail_count >= 6,
            "should keep at least 6 tail messages (3 tool rounds), got {} tail + 1 summary = {} total (original={})",
            tail_count,
            compacted.messages.len(),
            original_len
        );

        // Summary should be first
        assert!(
            compacted.messages[0]
                .content
                .as_text()
                .contains("[会话滚动摘要]"),
            "first message should be summary"
        );
    }

    fn tool_use_carrier(id: &str) -> LlmMessage {
        LlmMessage {
            role: "assistant".into(),
            content: MessageContent::Blocks(vec![ContentBlock::ToolUse {
                id: id.into(),
                name: "shell".into(),
                input: json!({"command": "echo atomic"}),
            }]),
        }
    }

    #[tokio::test]
    async fn compact_summarise_keeps_tool_use_and_result_atomic_at_split() {
        let latest_request = make_text_msg("user", "LATEST_USER_REQUEST_MUST_SURVIVE");
        let keep_tokens = crate::llm::estimate_message_tokens(&latest_request) + 1;
        let messages = vec![
            make_text_msg("user", "old history to summarise"),
            tool_use_carrier("atomic-call"),
            make_tool_result_msg("atomic-call", &"result ".repeat(200)),
            latest_request,
        ];
        let client = MockLlmClient::new("atomic rolling summary");

        let outcome = compact_summarise(messages, keep_tokens, &client, "test-model", 1_024, None)
            .await
            .expect("compaction outcome");

        assert!(matches!(
            &outcome.messages[1].content,
            MessageContent::Blocks(blocks)
                if blocks.iter().any(|block| matches!(block, ContentBlock::ToolUse { id, .. } if id == "atomic-call"))
        ));
        assert!(matches!(
            &outcome.messages[2].content,
            MessageContent::Blocks(blocks)
                if blocks.iter().any(|block| matches!(block, ContentBlock::ToolResult { tool_use_id, .. } if tool_use_id == "atomic-call"))
        ));
        assert!(matches!(
            &outcome.messages[3].content,
            MessageContent::Text(text) if text == "LATEST_USER_REQUEST_MUST_SURVIVE"
        ));
    }

    #[tokio::test]
    async fn compact_summarise_keeps_tool_result_image_carrier_atomic_at_split() {
        let latest_request = make_text_msg("user", "LATEST_MIXED_USER_REQUEST_MUST_SURVIVE");
        let keep_tokens = crate::llm::estimate_message_tokens(&latest_request) + 1;
        let mixed_result = LlmMessage {
            role: "user".into(),
            content: MessageContent::Blocks(vec![
                ContentBlock::ToolResult {
                    tool_use_id: "mixed-call".into(),
                    content: "mixed result ".repeat(200),
                    is_error: false,
                },
                ContentBlock::Image {
                    source: crate::llm::ImageSource {
                        source_type: "base64".into(),
                        media_type: "image/png".into(),
                        data: "aGVsbG8=".into(),
                    },
                },
            ]),
        };
        let messages = vec![
            make_text_msg("user", "old mixed history to summarise"),
            tool_use_carrier("mixed-call"),
            mixed_result,
            latest_request,
        ];
        let client = MockLlmClient::new("mixed atomic rolling summary");

        let outcome = compact_summarise(messages, keep_tokens, &client, "test-model", 1_024, None)
            .await
            .expect("compaction outcome");

        assert!(matches!(
            &outcome.messages[1].content,
            MessageContent::Blocks(blocks)
                if blocks.iter().any(|block| matches!(block, ContentBlock::ToolUse { id, .. } if id == "mixed-call"))
        ));
        assert!(matches!(
            &outcome.messages[2].content,
            MessageContent::Blocks(blocks)
                if blocks.iter().any(|block| matches!(block, ContentBlock::ToolResult { tool_use_id, .. } if tool_use_id == "mixed-call"))
                    && blocks.iter().any(|block| matches!(block, ContentBlock::Image { .. }))
        ));
        assert!(matches!(
            &outcome.messages[3].content,
            MessageContent::Text(text) if text == "LATEST_MIXED_USER_REQUEST_MUST_SURVIVE"
        ));
    }

    // ── Phase C: build_request_messages ───────────────────────────────────────

    fn user_text(text: &str) -> LlmMessage {
        LlmMessage {
            role: "user".into(),
            content: MessageContent::text(text),
        }
    }

    fn tool_result_carrier(id: &str, content: &str) -> LlmMessage {
        LlmMessage {
            role: "user".into(),
            content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                tool_use_id: id.to_string(),
                content: content.to_string(),
                is_error: false,
            }]),
        }
    }

    fn assistant_text(text: &str) -> LlmMessage {
        LlmMessage {
            role: "assistant".into(),
            content: MessageContent::text(text),
        }
    }

    #[test]
    fn build_request_messages_keeps_recent_turns_full() {
        // p5 two-boundary contract: with 10 short turns (one tool_result per
        // turn) both the turn-count boundary and the tool-carrier boundary
        // kick in. The effective cutoff is `min(turn, tool)`:
        //   * turn cutoff (keep 3 turns)          → index of turn 8's user
        //   * tool cutoff (keep 8 carriers)       → index of turn 3's carrier
        // so `min` = tool cutoff, and turns 1..=2's tool results get demoted
        // while turns 3..=10 stay full (tool-carrier boundary wins, as the
        // in-flight long-running work should).
        let big = "X".repeat(5_000);
        let mut messages = Vec::new();
        for turn in 1..=10 {
            messages.push(user_text(&format!("请求 {}", turn)));
            messages.push(assistant_text(&format!("assistant {}", turn)));
            messages.push(tool_result_carrier(&format!("call-{}", turn), &big));
        }
        let mut minimals = HashMap::new();
        for turn in 1..=10 {
            minimals.insert(format!("call-{}", turn), format!("receipt {}", turn));
        }
        let req = build_request_messages(
            &messages,
            &minimals,
            CTX_PRESERVE_RECENT_TURNS,
            CTX_KEEP_RECENT_TOOL_CARRIERS,
        );
        assert_eq!(req.len(), messages.len());

        // Turns 1 and 2 (idx 2 and 5) demoted — outside both boundaries.
        // p11: demoted receipts now carry a `[recall:<tool_use_id>]` suffix
        // so the agent can re-fetch the full content via recall_tool_result.
        for (idx, turn) in [(2usize, 1), (5, 2)] {
            if let MessageContent::Blocks(ref b) = req[idx].content {
                if let ContentBlock::ToolResult { content, .. } = &b[0] {
                    let expected = format!("receipt {} [recall:call-{}]", turn, turn);
                    assert_eq!(
                        content, &expected,
                        "turn {} (idx {}) should be demoted with recall hint",
                        turn, idx
                    );
                    continue;
                }
            }
            panic!("expected ToolResult at idx {}", idx);
        }

        // Turns 3..=10 preserved (within tool-carrier window of 8).
        for (idx, _turn) in (8..30).step_by(3).zip(3..=10) {
            if let MessageContent::Blocks(ref b) = req[idx].content {
                if let ContentBlock::ToolResult { content, .. } = &b[0] {
                    assert_eq!(
                        content.chars().count(),
                        5_000,
                        "tool at idx {} should stay full",
                        idx
                    );
                }
            }
        }
    }

    #[test]
    fn build_request_messages_tool_carrier_boundary_protects_single_long_turn() {
        // Single user turn, 12 tool-call iterations. CTX_KEEP_RECENT_TOOL_CARRIERS
        // is 8, but CTX_PRESERVE_RECENT_TURNS boundary alone would be 0
        // (only 1 user text boundary exists), so `min(0, tool_cutoff)` = 0
        // → nothing is demoted. This protects long autonomous workflows.
        let big = "Y".repeat(3_000);
        let mut messages = Vec::new();
        messages.push(user_text("开始长任务"));
        for i in 1..=12 {
            messages.push(assistant_text(&format!("iter {}", i)));
            messages.push(tool_result_carrier(&format!("call-{}", i), &big));
        }
        let mut minimals = HashMap::new();
        for i in 1..=12 {
            minimals.insert(format!("call-{}", i), format!("receipt {}", i));
        }
        let req = build_request_messages(
            &messages,
            &minimals,
            CTX_PRESERVE_RECENT_TURNS,
            CTX_KEEP_RECENT_TOOL_CARRIERS,
        );
        for i in 1..=12 {
            let idx = i * 2; // tool_result of iter i
            if let MessageContent::Blocks(ref b) = req[idx].content {
                if let ContentBlock::ToolResult { content, .. } = &b[0] {
                    assert_eq!(
                        content.chars().count(),
                        3_000,
                        "iter {} must stay full (single-turn protection)",
                        i
                    );
                }
            }
        }
    }

    #[test]
    fn build_request_messages_snaps_boundary_off_tool_use_result_pair() {
        // Construct: [user_text, assistant_with_tool_use, tool_result] × 10.
        // The raw boundary could fall on a `tool_result` carrier (a user
        // message whose only blocks are ToolResult). `snap_to_pair_boundary`
        // must step back over the preceding assistant `ToolUse` so the pair
        // is kept intact when it crosses the boundary.
        let big = "Z".repeat(2_000);
        let mut messages = Vec::new();
        for turn in 1..=10 {
            messages.push(user_text(&format!("Q{}", turn)));
            let tool_use_block = ContentBlock::ToolUse {
                id: format!("call-{}", turn),
                name: "shell".into(),
                input: serde_json::json!({"command": "echo"}),
            };
            messages.push(LlmMessage {
                role: "assistant".into(),
                content: MessageContent::Blocks(vec![
                    ContentBlock::Text {
                        text: format!("thinking {}", turn),
                    },
                    tool_use_block,
                ]),
            });
            messages.push(tool_result_carrier(&format!("call-{}", turn), &big));
        }
        let mut minimals = HashMap::new();
        for turn in 1..=10 {
            minimals.insert(format!("call-{}", turn), format!("r{}", turn));
        }
        let req = build_request_messages(
            &messages,
            &minimals,
            CTX_PRESERVE_RECENT_TURNS,
            CTX_KEEP_RECENT_TOOL_CARRIERS,
        );
        // For every assistant ToolUse that ended up retained with full
        // content, the matching ToolResult must also be retained. The
        // pair-boundary snap is what guarantees this.
        for i in 0..req.len() {
            if let MessageContent::Blocks(blocks) = &req[i].content {
                for b in blocks {
                    if let ContentBlock::ToolUse { id, .. } = b {
                        // Find matching tool_result in req.
                        let found = req.iter().any(|m| {
                            if let MessageContent::Blocks(bs) = &m.content {
                                bs.iter().any(|bb| {
                                    matches!(bb, ContentBlock::ToolResult { tool_use_id, .. }
                                             if tool_use_id == id)
                                })
                            } else {
                                false
                            }
                        });
                        assert!(found, "ToolUse {id} at idx {i} lost its ToolResult pair");
                    }
                }
            }
        }
    }

    #[test]
    fn build_request_messages_keeps_all_when_fewer_turns_than_window() {
        // With only 3 user turns and recent_full_turns=3, NOTHING should be
        // demoted — the whole session fits inside the recent window.
        let big = "X".repeat(5_000);
        let mut messages = Vec::new();
        for turn in 1..=3 {
            messages.push(user_text(&format!("请求 {}", turn)));
            messages.push(assistant_text(&format!("assistant {}", turn)));
            messages.push(tool_result_carrier(&format!("call-{}", turn), &big));
        }
        let mut minimals = HashMap::new();
        for turn in 1..=3 {
            minimals.insert(format!("call-{}", turn), format!("receipt {}", turn));
        }
        let req = build_request_messages(
            &messages,
            &minimals,
            CTX_PRESERVE_RECENT_TURNS,
            CTX_KEEP_RECENT_TOOL_CARRIERS,
        );
        for idx in [2usize, 5, 8] {
            if let MessageContent::Blocks(ref b) = req[idx].content {
                if let ContentBlock::ToolResult { content, .. } = &b[0] {
                    assert_eq!(
                        content.chars().count(),
                        5_000,
                        "turn at {} must stay full when total turns <= window",
                        idx
                    );
                }
            } else {
                panic!("expected Blocks at idx {}", idx);
            }
        }
    }

    #[test]
    fn build_request_messages_without_minimal_keeps_full() {
        // If the side-map has no entry for a tool_use_id, build_request_messages
        // must leave the full content intact rather than blanking it out.
        let msgs = vec![
            user_text("第 1 轮"),
            assistant_text("ok1"),
            tool_result_carrier("call-1", "LONG 1"),
            user_text("第 2 轮"),
            assistant_text("ok2"),
            tool_result_carrier("call-2", "LONG 2"),
            user_text("第 3 轮"),
            assistant_text("ok3"),
            tool_result_carrier("call-3", "LONG 3"),
            user_text("第 4 轮"),
            assistant_text("ok4"),
            tool_result_carrier("call-4", "LONG 4"),
        ];
        let minimals = HashMap::new(); // empty
        let req = build_request_messages(
            &msgs,
            &minimals,
            CTX_PRESERVE_RECENT_TURNS,
            CTX_KEEP_RECENT_TOOL_CARRIERS,
        );
        if let MessageContent::Blocks(ref b) = req[2].content {
            if let ContentBlock::ToolResult { content, .. } = &b[0] {
                assert_eq!(content, "LONG 1");
            }
        }
    }

    #[test]
    fn request_view_zero_tool_result_cap_disables_trimming() {
        let full = format!("UNTRIMMED {}", "x".repeat(20_000));
        let messages = vec![tool_result_carrier("no-cap", &full)];
        let request_view = build_request_view_messages(
            &messages,
            &HashMap::new(),
            CTX_PRESERVE_RECENT_TURNS,
            CTX_KEEP_RECENT_TOOL_CARRIERS,
            1_000,
            0,
        );

        assert!(matches!(
            &request_view[0].content,
            MessageContent::Blocks(blocks)
                if matches!(&blocks[0], ContentBlock::ToolResult { content, .. } if content == &full)
        ));
    }

    #[test]
    fn serialize_tool_results_with_receipts_injects_fields() {
        let blocks = [ContentBlock::ToolResult {
            tool_use_id: "call-1".into(),
            content: "full content".into(),
            is_error: false,
        }];
        let refs: Vec<&ContentBlock> = blocks.iter().collect();
        let mut minimals = HashMap::new();
        minimals.insert("call-1".to_string(), "receipt-1".to_string());
        let mut names = HashMap::new();
        names.insert("call-1".to_string(), "shell".to_string());
        let json = serialize_tool_results_with_receipts(&refs, Some(&minimals), Some(&names));
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v[0]["content_minimal"], "receipt-1");
        assert_eq!(v[0]["tool_name"], "shell");
        // Legacy fields preserved
        assert_eq!(v[0]["content"], "full content");
        assert_eq!(v[0]["tool_use_id"], "call-1");
    }

    /// Spy client that records the last prompt it saw, so we can assert that
    /// Level-2 summarisation receives the FULL content (not demoted minimal).
    struct SpyClient {
        last_prompt: std::sync::Mutex<String>,
        response: String,
    }

    #[async_trait]
    impl crate::llm::LlmClient for SpyClient {
        async fn stream(
            &self,
            _req: LlmRequest,
            _tx: tokio::sync::mpsc::Sender<LlmChunk>,
        ) -> Result<()> {
            Ok(())
        }

        async fn complete(&self, req: LlmRequest) -> Result<LlmResponse> {
            let joined = req
                .messages
                .iter()
                .map(|m| m.content.as_text())
                .collect::<Vec<_>>()
                .join("\n---\n");
            *self.last_prompt.lock().unwrap() = joined;
            Ok(LlmResponse {
                content: self.response.clone(),
                tool_calls: vec![],
                input_tokens: 123,
                output_tokens: 45,
            })
        }
    }

    #[tokio::test]
    async fn compact_summarise_uses_full_tool_result_content() {
        // Build a history where tool results carry distinctive full content.
        // After summarisation we assert the prompt contained that full text
        // (proof that the caller passed FULL, not the minimal receipt).
        let unique_marker = "FULL_CONTENT_MARKER_q7z9_long_tool_output";
        let big_content = format!("{} {}", unique_marker, "y".repeat(2000));
        let mut msgs = Vec::new();
        for turn in 1..=8 {
            msgs.push(user_text(&format!("请求 {}", turn)));
            msgs.push(assistant_text(&format!("回答 {}", turn)));
            msgs.push(tool_result_carrier(&format!("c{}", turn), &big_content));
        }
        let client = SpyClient {
            last_prompt: std::sync::Mutex::new(String::new()),
            response: "rolling summary output".into(),
        };
        // keep_tokens small enough to force summarisation of older turns.
        let result = compact_summarise(msgs, 500, &client, "test-model", 4096, None).await;
        assert!(result.is_some(), "summarise should have run");
        let prompt = client.last_prompt.lock().unwrap().clone();
        assert!(
            prompt.contains(unique_marker),
            "summariser prompt should include the full tool-result content"
        );
    }
}
