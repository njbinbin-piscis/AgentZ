//! Host-side context assembly for AgentZ chat turns.
//!
//! Previously AgentZ handed the kernel the latest ~500 DB rows as plain text and
//! relied solely on the kernel's in-loop compaction. This module mirrors
//! openpiscis's host pre-assembly so the *first* request of a (resumed) session
//! is already controlled:
//!
//! 1. Reconstruct each DB row into a real [`LlmMessage`] — honouring
//!    `tool_calls_json` (assistant tool_use) and `tool_results_json`
//!    (tool-result carriers) — so the kernel's Level-1 demotion has structured
//!    blocks to work with, and user-turn boundaries stay detectable.
//! 2. Prepend the persisted rolling summary and the p6 state frame.
//! 3. Window by turn age (full → trimmed) and budget-trim from newest.
//!
//! It is built only on kernel *primitives* (token estimators, message builders,
//! window constants). Compaction *policy* therefore lives here in the host —
//! the direction we want long-term, where hosts assemble their own strategy.

use piscis_kernel::agent::compaction::{
    CTX_COMPACT_AFTER, CTX_FULL_TURNS, CTX_TRIM_HEAD, CTX_TRIM_TAIL,
};
use piscis_kernel::agent::message_utils::rolling_summary_message;
use piscis_kernel::agent::state_frame::{state_frame_message, StateFrame};
use piscis_kernel::llm::{estimate_message_tokens, ContentBlock, LlmMessage, MessageContent};
use piscis_kernel::store::db::ChatMessage;

/// A conversation turn: one real user message and its following agent messages.
struct Turn {
    user: ChatMessage,
    agents: Vec<ChatMessage>,
}

/// A real user turn starts at a `user` row that is *not* a tool-result carrier.
fn is_real_user(m: &ChatMessage) -> bool {
    m.role == "user" && m.tool_results_json.is_none()
}

/// Reconstruct a DB row into the same `LlmMessage` shape the agent loop keeps
/// in memory. Mirrors the kernel's internal reconstruction so the request view
/// is consistent across host assembly and in-loop compaction.
fn reconstruct(msg: &ChatMessage) -> LlmMessage {
    if let Some(json) = msg.tool_results_json.as_deref() {
        let blocks = serde_json::from_str::<Vec<ContentBlock>>(json).unwrap_or_default();
        return LlmMessage {
            role: msg.role.clone(),
            content: MessageContent::Blocks(blocks),
        };
    }
    if let Some(json) = msg.tool_calls_json.as_deref() {
        let mut blocks: Vec<ContentBlock> = Vec::new();
        if !msg.content.is_empty() {
            blocks.push(ContentBlock::Text {
                text: msg.content.clone(),
            });
        }
        if let Ok(calls) = serde_json::from_str::<Vec<ContentBlock>>(json) {
            blocks.extend(calls);
        }
        let content = if blocks.is_empty() {
            MessageContent::text(&msg.content)
        } else {
            MessageContent::Blocks(blocks)
        };
        return LlmMessage {
            role: msg.role.clone(),
            content,
        };
    }
    LlmMessage {
        role: msg.role.clone(),
        content: MessageContent::text(&msg.content),
    }
}

/// Head+tail trim the content of any `ToolResult` blocks in a message — the
/// host-side Level-1 demotion for middle-aged turns.
fn demote_tool_results(msg: &mut LlmMessage) {
    if let MessageContent::Blocks(blocks) = &mut msg.content {
        for block in blocks.iter_mut() {
            if let ContentBlock::ToolResult { content, .. } = block {
                if content.chars().count() > CTX_TRIM_HEAD + CTX_TRIM_TAIL + 32 {
                    let head: String = content.chars().take(CTX_TRIM_HEAD).collect();
                    let tail: String = {
                        let all: Vec<char> = content.chars().collect();
                        all[all.len() - CTX_TRIM_TAIL..].iter().collect()
                    };
                    *content = format!("{head}\n…[trimmed]…\n{tail}");
                }
            }
        }
    }
}

fn split_turns(history: &[ChatMessage]) -> Vec<Turn> {
    let mut turns: Vec<Turn> = Vec::new();
    let mut current: Option<Turn> = None;
    for msg in history {
        if is_real_user(msg) {
            if let Some(turn) = current.take() {
                turns.push(turn);
            }
            current = Some(Turn {
                user: msg.clone(),
                agents: Vec::new(),
            });
        } else if let Some(turn) = current.as_mut() {
            turn.agents.push(msg.clone());
        }
        // Rows before the first real user message (rare) are dropped.
    }
    if let Some(turn) = current {
        turns.push(turn);
    }
    turns
}

/// Tokens of conversation history a new turn may carry.
///
/// The system prompt and the tool definitions are sent with every request,
/// so their measured size comes off the input budget first. A fixed
/// allowance for them let a session with many MCP tools assemble a first
/// request larger than the whole budget. A twentieth of the budget is held
/// back for what is added after assembly: an attachment, or a reminder
/// prepended to the user's message.
///
/// The history is not cut down to the kernel's compaction trigger here.
/// Turns dropped by this module are gone without a summary, whereas a request
/// that reaches the trigger is summarised by the kernel before it is sent.
pub fn turn_history_budget(total_input_budget: usize, static_overhead_tokens: usize) -> usize {
    total_input_budget
        .saturating_sub(static_overhead_tokens)
        .saturating_sub(total_input_budget / 20)
}

/// Assemble the LLM context window from DB history + persisted summary/frame.
pub fn build_context_messages(
    history: &[ChatMessage],
    budget: usize,
    rolling_summary: Option<&str>,
    state_frame: Option<&StateFrame>,
) -> Vec<LlmMessage> {
    let summary = rolling_summary.map(str::trim).filter(|s| !s.is_empty());

    let turns = split_turns(history);
    let total = turns.len();
    // Once a summary exists, older turns are represented by it — only keep the
    // most recent `CTX_COMPACT_AFTER` turns verbatim.
    let slice: &[Turn] = if summary.is_some() && total > CTX_COMPACT_AFTER {
        &turns[total - CTX_COMPACT_AFTER..]
    } else {
        &turns[..]
    };

    // Reserve the summary's and the state frame's token cost up front so the
    // budget accounts for them.
    let mut token_est = summary
        .map(|s| estimate_message_tokens(&rolling_summary_message(s)))
        .unwrap_or(0)
        + state_frame
            .map(|frame| estimate_message_tokens(&state_frame_message(frame)))
            .unwrap_or(0);

    // Build turn groups newest → oldest, then reverse to chronological order.
    let mut groups: Vec<Vec<LlmMessage>> = Vec::new();
    for (age, turn) in slice.iter().rev().enumerate() {
        // The newest turn is the message the user just sent; it is kept even
        // when the summary and the frame have used up the budget.
        if token_est >= budget && !groups.is_empty() {
            break;
        }
        let mut msgs: Vec<LlmMessage> = vec![LlmMessage {
            role: "user".into(),
            content: MessageContent::text(&turn.user.content),
        }];
        for agent in &turn.agents {
            let mut m = reconstruct(agent);
            if age >= CTX_FULL_TURNS {
                // Middle-aged turn: demote bulky tool results.
                demote_tool_results(&mut m);
            }
            msgs.push(m);
        }
        let group_tokens: usize = msgs.iter().map(estimate_message_tokens).sum();
        // Always keep at least the newest turn even if it alone exceeds budget.
        if token_est + group_tokens > budget && !groups.is_empty() {
            break;
        }
        token_est += group_tokens;
        groups.push(msgs);
    }
    groups.reverse();

    let mut out: Vec<LlmMessage> = Vec::new();
    if let Some(s) = summary {
        out.push(rolling_summary_message(s));
    }
    if let Some(frame) = state_frame {
        // Right after the summary (if any), else at the very top.
        let at = if summary.is_some() {
            1.min(out.len())
        } else {
            0
        };
        out.insert(at, state_frame_message(frame));
    }
    for group in groups {
        out.extend(group);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use piscis_kernel::agent::compaction_strategy::{
        CompactionGuard, CompactionOutcomeKind, CompactionPins, CompactionRequest,
        CompactionStrategy, CompactionTrigger,
    };
    use piscis_kernel::agent::harness::LayeredBudget;
    use piscis_kernel::agent::loop_::DefaultCompaction;
    use piscis_kernel::llm::{self, LlmChunk, LlmClient, LlmRequest, LlmResponse};
    use std::collections::HashMap;

    const WINDOW: u32 = 128_000;
    const MAX_TOKENS: u32 = 8_192;

    fn row(role: &str, content: String) -> ChatMessage {
        serde_json::from_value(serde_json::json!({
            "id": "row",
            "session_id": "session",
            "role": role,
            "content": content,
            "created_at": "2026-01-01T00:00:00Z",
        }))
        .expect("chat message row")
    }

    /// `turns` finished turns of about 1500 tokens each, then the message
    /// the user has just sent.
    fn session(turns: usize) -> Vec<ChatMessage> {
        let mut rows = Vec::new();
        for turn in 0..turns {
            rows.push(row("user", format!("request-{turn} {}", "u".repeat(400))));
            rows.push(row("assistant", format!("answer-{turn} {}", "a".repeat(5_600))));
        }
        rows.push(row("user", "CURRENT_REQUEST".to_string()));
        rows
    }

    /// A system prompt of about 50k tokens: many MCP tool descriptions.
    fn heavy_system_prompt() -> (String, usize) {
        let prompt = "s".repeat(50_000 * 4);
        let overhead = llm::estimate_request_overhead_tokens(Some(&prompt), &[]);
        assert!((50_000..50_100).contains(&overhead), "{overhead}");
        (prompt, overhead)
    }

    struct StubSummariser;

    #[async_trait::async_trait]
    impl LlmClient for StubSummariser {
        async fn stream(
            &self,
            _req: LlmRequest,
            _tx: tokio::sync::mpsc::Sender<LlmChunk>,
        ) -> anyhow::Result<()> {
            unreachable!("the summariser is called through complete")
        }

        async fn complete(&self, _req: LlmRequest) -> anyhow::Result<LlmResponse> {
            Ok(LlmResponse {
                content: "summary of earlier turns".into(),
                tool_calls: vec![],
                input_tokens: 1,
                output_tokens: 1,
            })
        }
    }

    #[test]
    fn history_budget_is_what_the_measured_overhead_leaves() {
        assert_eq!(turn_history_budget(100_000, 3_000), 92_000);
        assert_eq!(turn_history_budget(100_000, 50_000), 45_000);
        assert_eq!(turn_history_budget(100_000, 99_000), 0);
        assert_eq!(turn_history_budget(100_000, 150_000), 0);
    }

    #[test]
    fn heavy_static_overhead_no_longer_assembles_a_request_over_the_budget() {
        let total = llm::compute_total_input_budget(WINDOW, MAX_TOKENS);
        let (system_prompt, overhead) = heavy_system_prompt();
        let rows = session(120);

        // The fixed 3000-token allowance: history alone fills the budget, so
        // the request is half as large again as the budget.
        let before = build_context_messages(
            &rows,
            llm::compute_context_budget(WINDOW, MAX_TOKENS),
            None,
            None,
        );
        let before_request = llm::estimate_request_input_tokens(&before, Some(&system_prompt), &[]);
        assert!(before_request > total + 40_000, "{before_request} vs {total}");

        let messages =
            build_context_messages(&rows, turn_history_budget(total, overhead), None, None);
        let request = llm::estimate_request_input_tokens(&messages, Some(&system_prompt), &[]);
        assert!(request <= total, "{request} vs {total}");
        // The budget is used, not just respected.
        assert!(request > total * 9 / 10, "{request} vs {total}");
        assert_eq!(
            messages.last().map(|m| m.content.as_text()).as_deref(),
            Some("CURRENT_REQUEST")
        );
    }

    #[test]
    fn the_newest_turn_is_kept_even_when_the_overhead_leaves_no_budget() {
        let rows = session(5);
        let messages = build_context_messages(&rows, turn_history_budget(100_000, 120_000), None, None);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content.as_text(), "CURRENT_REQUEST");
    }

    /// With 50k tokens of static overhead, the first request the kernel
    /// sends in a new turn is below the micro trigger: the host hands over
    /// what fits the budget, and the kernel's pass before the first model
    /// call summarises it down in one go.
    #[tokio::test]
    async fn first_request_of_a_turn_with_heavy_overhead_is_below_the_micro_trigger() {
        let budget = LayeredBudget::from_context_window(WINDOW, MAX_TOKENS);
        let total = budget.total as usize;
        let (system_prompt, overhead) = heavy_system_prompt();
        let messages =
            build_context_messages(&session(120), turn_history_budget(total, overhead), None, None);
        let assembled = llm::estimate_request_input_tokens(&messages, Some(&system_prompt), &[]);
        assert!(assembled >= budget.trigger_auto as usize, "{assembled}");

        let pins = CompactionPins {
            user_request: "CURRENT_REQUEST".into(),
            ..Default::default()
        };
        let result = DefaultCompaction
            .compact(CompactionRequest {
                trigger: CompactionTrigger::Proactive,
                budget,
                messages,
                rolling_summary: "",
                system_prompt: &system_prompt,
                model: "test-model",
                max_tokens: MAX_TOKENS,
                context_window: WINDOW,
                tool_defs: &[],
                tool_minimals: &HashMap::new(),
                session_id: "host-assembly",
                cumulative_input_tokens: 0,
                next_auto_compact_threshold: i64::MAX,
                threshold_step: 0,
                client: &StubSummariser,
                guard: CompactionGuard::default(),
                pins: &pins,
            })
            .await;

        assert!(result.changed);
        assert_eq!(result.report.expect("report").outcome, CompactionOutcomeKind::Ok);
        let first_request =
            llm::estimate_request_input_tokens(&result.messages, Some(&system_prompt), &[]);
        assert!(
            first_request < budget.trigger_micro as usize,
            "{first_request} vs trigger {}",
            budget.trigger_micro
        );
        assert_eq!(
            result.messages.last().map(|m| m.content.as_text()).as_deref(),
            Some("CURRENT_REQUEST")
        );
    }
}
