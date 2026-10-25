/// OpenAI-compatible API client (Chat Completions, streaming SSE)
use super::{
    ContentBlock, LlmChunk, LlmClient, LlmMessage, LlmRequest, LlmResponse, MessageContent,
    ToolCall,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use std::error::Error as StdError;
use tokio::sync::mpsc::Sender;

pub struct OpenAiClient {
    api_key: String,
    base_url: String,
    http: Client,
    options: super::ClientOptions,
}

/// Returns true if the model name indicates vision/multimodal capability.
/// Conservative: only well-known vision models are listed.
/// Unknown models → no vision (safe default to avoid 400 errors).
/// NOTE: The authoritative vision validation is done at config save time
/// via `validate_vision_model` in the chat commands layer.
pub fn model_supports_vision(model: &str) -> bool {
    let m = model.to_lowercase();
    // OpenAI vision-capable models
    m.contains("gpt-4o")
        || m.contains("gpt-4-vision")
        || m.contains("gpt-4-turbo")
        || m.contains("o3")
        || m.contains("o4")
        // Qwen — qwen-vl, qwen*-vl, qwen3.6-plus models support vision;
        // plain "qwen3" text models (e.g. qwen3-235b-a22b) do NOT.
        || m.contains("qwen-vl")
        || m.contains("qwen3-vl")
        || m.contains("qwen2.5-vl")
        || m.contains("qvq")
        || m.contains("qwen3.6-plus")
        || m.contains("qwen3-plus")
        || m.contains("qwen-omni")
        // Claude 3+ (all support vision)
        || m.contains("claude-3")
        || m.contains("claude-4")
        || m.contains("claude-sonnet")
        || m.contains("claude-haiku")
        || m.contains("claude-opus")
        // Gemini
        || m.contains("gemini")
        // MiniMax / Kimi with vision
        || m.contains("abab6.5")
}

/// Parse the `tool_calls` array of a non-streaming Chat Completions response
/// message. Mirrors `parse_streamed_tool_call`'s "never fabricate `{}`"
/// contract: a tool call whose `arguments` fails to parse gets one
/// conservative repair attempt (P2-2, `json_repair`); if that also fails,
/// it aborts the whole response with a classifiable `tool_args_invalid`
/// error instead of returning a partially-fabricated `LlmResponse` that
/// looks successful.
fn parse_tool_calls(message: &Value) -> Result<Vec<ToolCall>> {
    let mut tool_calls = Vec::new();
    let Some(tcs) = message["tool_calls"].as_array() else {
        return Ok(tool_calls);
    };
    for tc in tcs {
        let id = tc["id"].as_str().unwrap_or("").to_string();
        let name = tc["function"]["name"].as_str().unwrap_or("").to_string();
        if name.trim().is_empty() {
            tracing::warn!("dropping tool call with empty name (id={})", id);
            continue;
        }
        let id = if id.is_empty() {
            format!("call_synth_{}", tool_calls.len())
        } else {
            id
        };
        let args_str = tc["function"]["arguments"].as_str().unwrap_or("");
        match serde_json::from_str::<Value>(args_str) {
            Ok(input) => tool_calls.push(ToolCall { id, name, input }),
            Err(parse_err) => {
                let args_len = args_str.len();
                // P2-2: one conservative repair attempt before giving up.
                if let Some(input) = super::json_repair::try_conservative_repair(args_str) {
                    tracing::warn!(
                        "tool_args_invalid recovered via conservative JSON repair: tool={} id={} args_len={} parse_error={}",
                        name,
                        id,
                        args_len,
                        parse_err
                    );
                    tool_calls.push(ToolCall { id, name, input });
                    continue;
                }
                tracing::warn!(
                    "tool_args_invalid: tool={} id={} args_len={} parse_error={} args_preview={:?}",
                    name,
                    id,
                    args_len,
                    parse_err,
                    args_str.chars().take(200).collect::<String>()
                );
                return Err(anyhow!(
                    "tool_args_invalid: failed to parse arguments for tool \"{}\" (id={}, args_len={}): {}",
                    name,
                    id,
                    args_len,
                    parse_err
                ));
            }
        }
    }
    Ok(tool_calls)
}

fn is_dashscope_qwen_endpoint(base_url: &str, model: &str) -> bool {
    let url = base_url.to_lowercase();
    let model = model.to_lowercase();
    url.contains("dashscope.aliyuncs.com") && (model.contains("qwen") || model.contains("qvq"))
}

fn thinking_blocked_models() -> &'static std::sync::Mutex<std::collections::HashSet<String>> {
    static SET: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> =
        std::sync::OnceLock::new();
    SET.get_or_init(Default::default)
}

pub(crate) fn is_reasoning_replay_error(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("reasoning_content")
        && (lower.contains("passed back") || lower.contains("must be") || lower.contains("missing"))
}

fn has_assistant_tool_calls(body: &Value) -> bool {
    body["messages"].as_array().is_some_and(|msgs| {
        msgs.iter().any(|m| {
            m["role"] == "assistant"
                && m.get("tool_calls")
                    .and_then(|t| t.as_array())
                    .is_some_and(|t| !t.is_empty())
        })
    })
}

fn is_deepseek_thinking_model(model: &str) -> bool {
    let model = model.to_lowercase();
    // Also covers gateway-prefixed names (e.g. `deepseek-ai/DeepSeek-R1`) and
    // `*-think*` variants, whose reasoning_content we cannot replay.
    model.contains("deepseek")
        && (model.contains("v4")
            || model.ends_with("deepseek-flash")
            || model.contains("reasoner")
            || model.contains("think")
            || model.contains("-r1"))
}

/// Parse a single accumulated tool-call's `arguments` buffer into a chunk to
/// forward to the caller. Never silently substitutes `{}` on a parse
/// failure (see plan "OpenAI 兼容链路鲁棒性" P0-2) — truncated/malformed
/// `arguments` JSON (e.g. cut off mid-stream, or genuinely empty because the
/// model never sent any) must surface as an explicit, classifiable error so
/// the agent loop never executes a tool call with fabricated input.
fn parse_streamed_tool_call(id: String, name: String, args_buf: String) -> LlmChunk {
    match serde_json::from_str::<Value>(&args_buf) {
        Ok(input) => LlmChunk::ToolUse { id, name, input },
        Err(parse_err) => {
            let args_len = args_buf.len();
            // P2-2: one conservative repair attempt (rebalance brackets /
            // close an unterminated string) before giving up. Never
            // fabricates field content — see `llm::json_repair`.
            if let Some(input) = super::json_repair::try_conservative_repair(&args_buf) {
                tracing::warn!(
                    "tool_args_invalid recovered via conservative JSON repair: tool={} id={} args_len={} parse_error={}",
                    name,
                    id,
                    args_len,
                    parse_err
                );
                return LlmChunk::ToolUse { id, name, input };
            }
            let preview: String = args_buf.chars().take(200).collect();
            tracing::warn!(
                "tool_args_invalid: tool={} id={} args_len={} parse_error={} args_preview={:?}",
                name,
                id,
                args_len,
                parse_err,
                preview
            );
            LlmChunk::Error(format!(
                "tool_args_invalid: failed to parse arguments for tool \"{}\" (id={}, args_len={}): {}",
                name, id, args_len, parse_err
            ))
        }
    }
}

/// Drain all buffered streamed tool calls, sending a `ToolUse` chunk for
/// each one that parses cleanly and an `Error` chunk (never a silent `{}`)
/// for each one that doesn't.
async fn emit_buffered_tool_calls(
    tool_bufs: &mut std::collections::HashMap<usize, (String, String, String)>,
    tx: &Sender<LlmChunk>,
) {
    // Deterministic order (HashMap drain is random).
    let mut entries: Vec<_> = tool_bufs.drain().collect();
    entries.sort_by_key(|(idx, _)| *idx);
    for (idx, (id, name, args_buf)) in entries {
        if name.trim().is_empty() {
            tracing::warn!(
                "dropping streamed tool call with empty name (index={} id={} args_len={})",
                idx,
                id,
                args_buf.len()
            );
            continue;
        }
        let id = if id.is_empty() {
            format!("call_synth_{idx}")
        } else {
            id
        };
        let _ = tx.send(parse_streamed_tool_call(id, name, args_buf)).await;
    }
}

/// Build the `protocol_error` message for a stream that ended (no more SSE
/// bytes, connection closed) while one or more tool calls were still
/// buffered — i.e. it never reached `[DONE]` or a `finish_reason` that would
/// have triggered a drain. Dropping these silently would look like the
/// model simply chose not to call any tool, hiding a real failure.
fn undrained_tool_bufs_message(
    tool_bufs: &std::collections::HashMap<usize, (String, String, String)>,
) -> String {
    let pending: Vec<String> = tool_bufs
        .values()
        .map(|(id, name, args_buf)| format!("{}(id={}, args_len={})", name, id, args_buf.len()))
        .collect();
    format!(
        "protocol_error: stream ended with {} undrained tool call(s) still buffered [{}] — \
         the connection likely closed mid tool-call",
        tool_bufs.len(),
        pending.join(", ")
    )
}

impl OpenAiClient {
    #[allow(dead_code)]
    pub fn new(api_key: &str, base_url: &str) -> Self {
        Self::with_timeout(api_key, base_url, 120)
    }

    pub fn with_timeout(api_key: &str, base_url: &str, read_timeout_secs: u32) -> Self {
        // Configurable read timeout: prevents indefinite hang when the server accepts the
        // connection but stops sending data mid-stream (common with DeepSeek under load).
        let secs = read_timeout_secs.max(30) as u64;
        let http = Client::builder()
            .read_timeout(std::time::Duration::from_secs(secs))
            .build()
            .unwrap_or_default();
        Self {
            api_key: api_key.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            http,
            options: super::ClientOptions::default(),
        }
    }

    pub fn with_options(mut self, options: super::ClientOptions) -> Self {
        self.options = options;
        self
    }

    fn thinking_block_key(&self, model: &str) -> String {
        format!("{}|{}", self.base_url, model.to_lowercase())
    }

    fn is_thinking_blocked(&self, model: &str) -> bool {
        thinking_blocked_models()
            .lock()
            .map(|set| set.contains(&self.thinking_block_key(model)))
            .unwrap_or(false)
    }

    fn block_thinking(&self, model: &str) {
        if let Ok(mut set) = thinking_blocked_models().lock() {
            set.insert(self.thinking_block_key(model));
        }
    }

    /// POST `/chat/completions`. If the provider rejects the request because
    /// thinking mode wants a `reasoning_content` replay, thinking is turned
    /// off for this endpoint+model (process-wide) and the request is resent
    /// once — the model cannot fix this itself, the history shape is ours.
    async fn post_chat(&self, req: &LlmRequest) -> Result<reqwest::Response> {
        let url = format!("{}/chat/completions", self.base_url);
        let mut downgraded = false;
        loop {
            let body = self.build_body(req);
            let response = self
                .http
                .post(&url)
                .bearer_auth(&self.api_key)
                .json(&body)
                .send()
                .await
                .map_err(|e| Self::request_send_error(&url, e))?;
            if response.status().is_success() {
                return Ok(response);
            }
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            Self::log_400_diagnostic(status, &body, &req.model, &url, &text);
            if !downgraded
                && status == reqwest::StatusCode::BAD_REQUEST
                && is_reasoning_replay_error(&text)
            {
                tracing::warn!(
                    model = %req.model,
                    "provider requires reasoning_content replay; disabling thinking and retrying once"
                );
                self.block_thinking(&req.model);
                downgraded = true;
                continue;
            }
            return Err(Self::api_error(&req.model, status, &text));
        }
    }

    /// Build the error for a non-2xx HTTP response and, in the same place,
    /// emit the structured log line consumers can grep/alert on (P1-3,
    /// observability): `model`, `http_status`, `error_code`. The returned
    /// `anyhow::Error`'s `Display` text is unchanged (`"OpenAI API error
    /// {status}: {body}"`) so `error_class::classify_error` and existing
    /// callers keep working on the flattened string.
    fn api_error(model: &str, status: reqwest::StatusCode, text: &str) -> anyhow::Error {
        let msg = format!("OpenAI API error {}: {}", status, text);
        let class = crate::llm::error_class::classify_error(&msg);
        tracing::warn!(
            model = model,
            http_status = status.as_u16(),
            error_code = class.code(),
            "OpenAI-compatible API returned an error response"
        );
        anyhow!(msg)
    }

    fn request_send_error(url: &str, err: reqwest::Error) -> anyhow::Error {
        let mut flags = Vec::new();
        if err.is_timeout() {
            flags.push("timeout");
        }
        if err.is_connect() {
            flags.push("connect");
        }
        if err.is_request() {
            flags.push("request");
        }
        if err.is_body() {
            flags.push("body");
        }

        let mut sources = Vec::new();
        let mut source = err.source();
        while let Some(current) = source {
            sources.push(current.to_string());
            source = current.source();
        }

        anyhow!(
            "OpenAI-compatible request failed before HTTP response: url={} flags={} error={} sources=[{}] debug={:?}",
            url,
            if flags.is_empty() { "none".to_string() } else { flags.join(",") },
            err,
            sources.join(" | "),
            err
        )
    }

    /// Log diagnostic information when an OpenAI-compatible API returns a 400 error.
    /// Dumps the message sequence summary (role + content type) to help identify
    /// content-format issues (e.g. DashScope "Unexpected item type in content").
    fn log_400_diagnostic(
        status: reqwest::StatusCode,
        body: &Value,
        model: &str,
        url: &str,
        response_text: &str,
    ) {
        if status.as_u16() != 400 {
            return;
        }
        let msg_summary: Vec<String> = body["messages"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .enumerate()
                    .map(|(i, m)| {
                        let role = m["role"].as_str().unwrap_or("?");
                        let content_kind = if m["content"].is_string() {
                            "string".to_string()
                        } else if let Some(a) = m["content"].as_array() {
                            let types: Vec<&str> =
                                a.iter().filter_map(|v| v["type"].as_str()).collect();
                            format!("array[{}]", types.join(","))
                        } else if m["content"].is_null() {
                            "null".to_string()
                        } else {
                            "other".to_string()
                        };
                        let has_tc = m.get("tool_calls").is_some();
                        format!("[{i}] {role} content={content_kind} tc={has_tc}")
                    })
                    .collect()
            })
            .unwrap_or_default();
        tracing::error!(
            "OpenAI API 400 — model={} url={}\n  messages: {}\n  response: {}",
            model,
            url,
            msg_summary.join(" → "),
            response_text,
        );
    }

    /// Convert an Image block to a safe text placeholder.
    fn image_placeholder(is_latest: bool) -> ContentBlock {
        let msg = if is_latest {
            "[图片/截图已捕获 — 如需查看请使用 browser screenshot 工具重新截图]".to_string()
        } else {
            "[历史截图已省略 — 仅保留最近一轮截图以节省上下文]".to_string()
        };
        ContentBlock::Text { text: msg }
    }

    /// Preprocess messages: strip or downgrade Image blocks according to vision support.
    ///
    /// Rules:
    /// - Non-vision model: replace ALL Image blocks with text placeholders.
    /// - Vision model: keep Image blocks only from the LAST assistant/tool turn;
    ///   replace all older Image blocks with text placeholders.
    fn strip_images(&self, messages: &[LlmMessage], vision: bool) -> Vec<LlmMessage> {
        if !vision {
            // Strip all images
            return messages
                .iter()
                .map(|m| {
                    let content = match &m.content {
                        MessageContent::Blocks(blocks) => {
                            let new_blocks: Vec<ContentBlock> = blocks
                                .iter()
                                .map(|b| {
                                    if matches!(b, ContentBlock::Image { .. }) {
                                        Self::image_placeholder(false)
                                    } else {
                                        b.clone()
                                    }
                                })
                                .collect();
                            MessageContent::Blocks(new_blocks)
                        }
                        other => other.clone(),
                    };
                    LlmMessage {
                        role: m.role.clone(),
                        content,
                    }
                })
                .collect();
        }

        // Vision model: find index of the LAST message containing an Image block
        let last_image_msg = messages.iter().rposition(|m| {
            if let MessageContent::Blocks(blocks) = &m.content {
                blocks
                    .iter()
                    .any(|b| matches!(b, ContentBlock::Image { .. }))
            } else {
                false
            }
        });

        messages
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let is_latest_image_msg = last_image_msg == Some(i);
                let content = match &m.content {
                    MessageContent::Blocks(blocks) => {
                        let new_blocks: Vec<ContentBlock> = blocks
                            .iter()
                            .map(|b| {
                                if matches!(b, ContentBlock::Image { .. }) {
                                    if is_latest_image_msg {
                                        b.clone() // Keep the latest image for vision models
                                    } else {
                                        Self::image_placeholder(false) // Replace older images
                                    }
                                } else {
                                    b.clone()
                                }
                            })
                            .collect();
                        MessageContent::Blocks(new_blocks)
                    }
                    other => other.clone(),
                };
                LlmMessage {
                    role: m.role.clone(),
                    content,
                }
            })
            .collect()
    }

    fn convert_messages(&self, messages: &[LlmMessage], vision: bool) -> Vec<Value> {
        // Pre-pass: build a set of indices that are "safe" to include.
        // A tool_calls message is only safe if ALL its tool_call_ids are satisfied by
        // immediately following tool-result messages. A tool_result message is only safe
        // if it is preceded by a tool_calls message that contains its id.
        // We do this by scanning forward and marking unsafe indices to skip.
        let n = messages.len();
        let mut skip = vec![false; n];

        let mut i = 0;
        while i < n {
            let m = &messages[i];
            // Check if this is an assistant message with tool_calls
            let tool_call_ids: Vec<String> = if let MessageContent::Blocks(blocks) = &m.content {
                blocks
                    .iter()
                    .filter_map(|b| {
                        if let ContentBlock::ToolUse { id, .. } = b {
                            Some(id.clone())
                        } else {
                            None
                        }
                    })
                    .collect()
            } else {
                vec![]
            };

            if !tool_call_ids.is_empty() {
                // Collect the tool_call_ids that are satisfied by immediately following messages
                let mut satisfied: std::collections::HashSet<String> =
                    std::collections::HashSet::new();
                let mut j = i + 1;
                while j < n {
                    if let MessageContent::Blocks(blocks) = &messages[j].content {
                        let has_result = blocks
                            .iter()
                            .any(|b| matches!(b, ContentBlock::ToolResult { .. }));
                        if has_result {
                            for b in blocks {
                                if let ContentBlock::ToolResult { tool_use_id, .. } = b {
                                    satisfied.insert(tool_use_id.clone());
                                }
                            }
                            j += 1;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                // If any tool_call_id is not satisfied, skip this entire tool_calls+results block
                let all_satisfied = tool_call_ids.iter().all(|id| satisfied.contains(id));
                if !all_satisfied {
                    tracing::warn!(
                        "Skipping tool_calls message with unsatisfied ids {:?} (satisfied: {:?})",
                        tool_call_ids,
                        satisfied
                    );
                    skip[i] = true;
                    // Also skip the immediately following tool-result messages for this block
                    let mut k = i + 1;
                    while k < n {
                        if let MessageContent::Blocks(blocks) = &messages[k].content {
                            if blocks
                                .iter()
                                .any(|b| matches!(b, ContentBlock::ToolResult { .. }))
                            {
                                skip[k] = true;
                                k += 1;
                                continue;
                            }
                        }
                        break;
                    }
                }
            }
            i += 1;
        }

        // Debug: log the pre-pass skip decisions
        for (idx, m) in messages.iter().enumerate() {
            let summary = match &m.content {
                MessageContent::Text(t) => format!("text({} chars)", t.len()),
                MessageContent::Blocks(blocks) => {
                    let uses: Vec<_> = blocks
                        .iter()
                        .filter_map(|b| {
                            if let ContentBlock::ToolUse { id, name, .. } = b {
                                Some(format!("use({name}/{id})"))
                            } else {
                                None
                            }
                        })
                        .collect();
                    let results: Vec<_> = blocks
                        .iter()
                        .filter_map(|b| {
                            if let ContentBlock::ToolResult { tool_use_id, .. } = b {
                                Some(format!("result({tool_use_id})"))
                            } else {
                                None
                            }
                        })
                        .collect();
                    let texts: usize = blocks
                        .iter()
                        .filter(|b| matches!(b, ContentBlock::Text { .. }))
                        .count();
                    format!("blocks[uses={uses:?} results={results:?} texts={texts}]")
                }
            };
            tracing::debug!(
                "convert_messages pre-pass [{idx}] role={} skip={} content={}",
                m.role,
                skip[idx],
                summary
            );
        }

        let mut result: Vec<Value> = Vec::new();
        // Images from tool results that need to be appended as a separate user message
        // right after the tool messages (OpenAI format requires this).
        let mut pending_vision: Vec<Value> = Vec::new();
        // tool_call ids emitted by an assistant message that still await a reply.
        // Replaces the old "previous message must be the tool_calls one" check,
        // which dropped the 2nd..Nth result of a parallel tool-call batch and made
        // strict providers (DeepSeek) reject the request.
        let mut pending_tool_ids: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        for (idx, m) in messages.iter().enumerate() {
            if skip[idx] {
                tracing::debug!("convert_messages [{idx}] SKIPPED (pre-pass)");
                continue;
            }

            // Flush any pending vision images before starting a new non-tool message
            // (so they appear immediately after the last tool message).
            let carries_tool_result = matches!(
                &m.content,
                MessageContent::Blocks(b) if b.iter().any(|x| matches!(x, ContentBlock::ToolResult { .. }))
            );
            // Do not split a batch of tool replies with the vision user message.
            if !pending_vision.is_empty() && m.role != "tool" && !carries_tool_result {
                tracing::debug!(
                    "convert_messages [{idx}] flushing {} pending_vision images before role={}",
                    pending_vision.len(),
                    m.role
                );
                // Some API providers (e.g. DashScope) reject content arrays
                // that contain only image items without a leading text item.
                // Prepend a short text placeholder to keep the array valid.
                let mut flushed = std::mem::take(&mut pending_vision);
                if !flushed.iter().any(|v| v["type"] == "text") {
                    flushed.insert(
                        0,
                        json!({"type": "text", "text": "[Tool-generated image(s)]"}),
                    );
                }
                result.push(json!({
                    "role": "user",
                    "content": flushed
                }));
            }

            // Defense: skip orphaned tool-result messages that have no preceding tool_calls.
            // These can appear when context is truncated mid-turn.
            if let MessageContent::Blocks(blocks) = &m.content {
                let has_tool_result = blocks
                    .iter()
                    .any(|b| matches!(b, ContentBlock::ToolResult { .. }));
                if has_tool_result {
                    let answers_pending = blocks.iter().any(|b| {
                        matches!(b, ContentBlock::ToolResult { tool_use_id, .. }
                            if pending_tool_ids.contains(tool_use_id))
                    });
                    if !answers_pending {
                        tracing::warn!(
                            "convert_messages [{idx}] SKIP orphaned tool_result (no pending tool_call id)"
                        );
                        continue;
                    }
                }
            }

            match &m.content {
                MessageContent::Text(t) => {
                    result.push(json!({"role": m.role, "content": t}));
                }
                MessageContent::Blocks(blocks) => {
                    let has_tool_use = blocks
                        .iter()
                        .any(|b| matches!(b, ContentBlock::ToolUse { .. }));
                    let has_tool_result = blocks
                        .iter()
                        .any(|b| matches!(b, ContentBlock::ToolResult { .. }));

                    if has_tool_result {
                        // OpenAI: each ToolResult → separate "tool" role message (content must be string).
                        // Image blocks are collected and will become a "user" message right after.
                        for block in blocks {
                            match block {
                                ContentBlock::ToolResult {
                                    tool_use_id,
                                    content,
                                    ..
                                } => {
                                    pending_tool_ids.remove(tool_use_id);
                                    result.push(json!({
                                        "role": "tool",
                                        "tool_call_id": tool_use_id,
                                        "content": content
                                    }));
                                }
                                ContentBlock::Image { source } if vision => {
                                    pending_vision.push(json!({
                                        "type": "image_url",
                                        "image_url": {
                                            "url": format!("data:{};base64,{}", source.media_type, source.data)
                                        }
                                    }));
                                }
                                ContentBlock::Image { .. } => {
                                    // Non-vision model — image already replaced by strip_images(),
                                    // this branch is a safety fallback; simply skip.
                                }
                                ContentBlock::Text { text } if !text.is_empty() => {
                                    // Text mixed into a tool-result block would break the OpenAI
                                    // message ordering contract — drop it and log a warning.
                                    let preview: String = text.chars().take(80).collect();
                                    tracing::warn!(
                                        "convert_messages: dropping Text block inside tool-result message to avoid API error (text={:?})",
                                        preview
                                    );
                                }
                                _ => {}
                            }
                        }
                    } else if has_tool_use {
                        let mut text_content = String::new();
                        let mut tool_calls: Vec<Value> = Vec::new();

                        for block in blocks {
                            match block {
                                ContentBlock::Text { text } => text_content.push_str(text),
                                ContentBlock::ToolUse { id, name, input } => {
                                    // Providers (e.g. mimo) reject tool_calls with an
                                    // empty function name (400 "missing a function name").
                                    // Such calls come from malformed model output; drop
                                    // them (their orphaned results are dropped below).
                                    if name.trim().is_empty() {
                                        tracing::warn!(
                                            "convert_messages: dropping tool_call with empty name (id={})",
                                            id
                                        );
                                        continue;
                                    }
                                    pending_tool_ids.insert(id.clone());
                                    tool_calls.push(json!({
                                        "id": id,
                                        "type": "function",
                                        "function": {
                                            "name": name,
                                            "arguments": serde_json::to_string(input)
                                                .unwrap_or_else(|_| "{}".to_string())
                                        }
                                    }));
                                }
                                // Images inside a ToolUse message are unusual; skip silently.
                                _ => {}
                            }
                        }

                        if tool_calls.is_empty() {
                            // Every call was dropped as invalid; keep only the text.
                            if !text_content.is_empty() {
                                result.push(json!({
                                    "role": "assistant",
                                    "content": text_content
                                }));
                            }
                        } else {
                            let mut msg = json!({
                                "role": "assistant",
                                "tool_calls": tool_calls,
                                "content": Value::Null
                            });
                            if !text_content.is_empty() {
                                msg["content"] = json!(text_content);
                            }
                            result.push(msg);
                        }
                    } else {
                        // Regular user/assistant message — may contain text + images.
                        let mut parts: Vec<Value> = Vec::new();
                        for b in blocks {
                            match b {
                                ContentBlock::Text { text } if !text.is_empty() => {
                                    parts.push(json!({"type": "text", "text": text}));
                                }
                                ContentBlock::Image { source } if vision => {
                                    parts.push(json!({
                                        "type": "image_url",
                                        "image_url": {
                                            "url": format!("data:{};base64,{}", source.media_type, source.data)
                                        }
                                    }));
                                }
                                // Non-vision model: Image already replaced upstream; skip here.
                                _ => {}
                            }
                        }

                        if parts.is_empty() {
                            continue;
                        }

                        // Collapse single-text to plain string (cleaner API payload)
                        if parts.len() == 1 {
                            if let Some(text) = parts[0]["text"].as_str() {
                                result.push(json!({"role": m.role, "content": text}));
                                continue;
                            }
                        }
                        // Some providers reject content arrays with only image items.
                        // Prepend a short text placeholder if no text item exists.
                        if !parts.is_empty() && !parts.iter().any(|v| v["type"] == "text") {
                            parts.insert(0, json!({"type": "text", "text": "[Image(s)]"}));
                        }
                        result.push(json!({"role": m.role, "content": parts}));
                    }
                }
            }
        }

        // Flush any remaining pending vision images
        if !pending_vision.is_empty() {
            // Same as above: prepend text placeholder for providers that
            // reject content arrays containing only image items.
            if !pending_vision.iter().any(|v| v["type"] == "text") {
                pending_vision.insert(
                    0,
                    json!({"type": "text", "text": "[Tool-generated image(s)]"}),
                );
            }
            result.push(json!({
                "role": "user",
                "content": std::mem::take(&mut pending_vision)
            }));
        }

        // Debug: log the final message sequence sent to the API
        let seq: Vec<String> = result
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let role = v["role"].as_str().unwrap_or("?");
                let detail = if let Some(tcs) = v["tool_calls"].as_array() {
                    let ids: Vec<_> = tcs.iter().filter_map(|tc| tc["id"].as_str()).collect();
                    format!("tool_calls{ids:?}")
                } else if v["tool_call_id"].is_string() {
                    format!("tool_call_id={}", v["tool_call_id"].as_str().unwrap_or("?"))
                } else {
                    let content_len = v["content"].as_str().map(|s| s.len()).unwrap_or(0);
                    format!("content({content_len} chars)")
                };
                format!("[{i}]{role}:{detail}")
            })
            .collect();
        tracing::debug!("convert_messages final sequence: {}", seq.join(" → "));

        result
    }

    fn build_body(&self, req: &LlmRequest) -> Value {
        let vision = req
            .vision_override
            .unwrap_or_else(|| model_supports_vision(&req.model));
        tracing::info!(
            "build_body: model={} vision_override={:?} vision={}",
            req.model,
            req.vision_override,
            vision
        );
        let stripped = self.strip_images(&req.messages, vision);
        let messages = self.convert_messages(&stripped, vision);
        let mut body = json!({
            "model": req.model,
            "max_tokens": req.max_tokens,
            "messages": messages,
            "stream": req.stream,
        });

        if let Some(sys) = &req.system {
            // Prepend system message
            if let Some(arr) = body["messages"].as_array_mut() {
                arr.insert(0, json!({"role": "system", "content": sys}));
            }
        }

        // ── Defensive sanitization of content arrays ─────────────────────────
        // 1. If a content array has NO image items, collapse to a plain string.
        //    Some providers (DashScope text-only models) reject content arrays
        //    entirely, even [{type: "text", text: "..."}].
        // 2. If a content array has images but no text item, prepend a text
        //    placeholder (DashScope requires at least one text item).
        if let Some(arr) = body["messages"].as_array_mut() {
            for msg in arr.iter_mut() {
                let role_str = msg["role"].as_str().unwrap_or("?").to_string();
                let needs_fix = msg.get("content").and_then(|c| c.as_array()).is_some();
                if !needs_fix {
                    continue;
                }
                let items = msg["content"].as_array().unwrap();
                let has_image = items.iter().any(|v| v["type"] == "image_url");

                if !has_image {
                    // No images — collapse to plain text string
                    let text: String = items
                        .iter()
                        .filter_map(|v| {
                            if v["type"] == "text" {
                                v["text"].as_str()
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    if !text.is_empty() {
                        tracing::debug!(
                            "build_body: sanitization — collapsing text-only content array to string (role={})",
                            role_str
                        );
                        msg["content"] = json!(text);
                    }
                } else {
                    // Has images — ensure a text item exists
                    let has_text = items.iter().any(|v| v["type"] == "text");
                    if !has_text {
                        tracing::warn!(
                            "build_body: sanitization — prepending text placeholder to image-only content array (role={})",
                            role_str
                        );
                        if let Some(arr) = msg["content"].as_array_mut() {
                            arr.insert(0, json!({"type": "text", "text": "[Image(s)]"}));
                        }
                    }
                }
            }
        }

        if !req.tools.is_empty() {
            let tools: Vec<Value> = req
                .tools
                .iter()
                .map(|t| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.input_schema,
                        }
                    })
                })
                .collect();
            body["tools"] = json!(tools);
            body["tool_choice"] = json!("auto");
        }

        if let Some(t) = self.options.temperature {
            body["temperature"] = json!(t);
        }
        if let Some(p) = self.options.top_p {
            body["top_p"] = json!(p);
        }
        let thinking_blocked = self.is_thinking_blocked(&req.model);
        // Reasoning traces are not persisted, so once the history holds an
        // assistant tool call the provider would demand a `reasoning_content`
        // we cannot replay. Thinking stays usable only before that point.
        let thinking_on = self.options.thinking == Some(true)
            && !thinking_blocked
            && !has_assistant_tool_calls(&body);

        if thinking_blocked && !is_dashscope_qwen_endpoint(&self.base_url, &req.model) {
            body["thinking"] = json!({ "type": "disabled" });
        }

        if is_dashscope_qwen_endpoint(&self.base_url, &req.model) {
            // DashScope Qwen thinking mode requires assistant
            // `reasoning_content` to be passed back in every later request.
            // OpenPiscis's persisted message model stores user-visible content
            // and tool calls, not hidden reasoning traces, so leaving thinking
            // enabled breaks resumed/IM conversations with a 400. Keep the
            // OpenAI-compatible payload stateless until reasoning traces are a
            // first-class persisted field.
            body["enable_thinking"] = json!(thinking_on);
        }
        if is_deepseek_thinking_model(&req.model) {
            // DeepSeek's newer thinking models default thinking on and require
            // `reasoning_content` to be replayed after tool calls. We do not
            // persist hidden reasoning traces yet, so disable thinking to keep
            // multi-turn IM/headless conversations compatible with our stored
            // OpenAI-style message history.
            body["thinking"] = json!({ "type": if thinking_on { "enabled" } else { "disabled" } });
        }

        body
    }
}

#[async_trait]
impl LlmClient for OpenAiClient {
    async fn stream(&self, req: LlmRequest, tx: Sender<LlmChunk>) -> Result<()> {
        let mut req_stream = req.clone();
        req_stream.stream = true;
        let response = self.post_chat(&req_stream).await?;

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        // tool call accumulation: index -> (id, name, args_buf)
        let mut tool_bufs: std::collections::HashMap<usize, (String, String, String)> =
            std::collections::HashMap::new();
        let mut input_tokens = 0u32;
        let mut output_tokens = 0u32;
        // P2-1: distinguishes "the stream ended having produced nothing at
        // all" (almost certainly a dropped connection before the response
        // even started) from "the stream ended after delivering real
        // content but without an explicit [DONE]" (some OpenAI-compatible
        // providers omit it once everything has already been sent) — only
        // the former is flagged as `protocol_error`.
        let mut any_output_sent = false;

        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => {
                    // Network-level errors mid-stream (server closed connection, incomplete
                    // chunk, etc.) — propagate so the caller can retry with backoff.
                    return Err(anyhow::anyhow!("error decoding response body: {}", e));
                }
            };
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim().to_string();
                buffer = buffer[pos + 1..].to_string();

                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" {
                        // Drain any tool calls that arrived before [DONE]
                        emit_buffered_tool_calls(&mut tool_bufs, &tx).await;
                        let _ = tx
                            .send(LlmChunk::Done {
                                input_tokens,
                                output_tokens,
                            })
                            .await;
                        return Ok(());
                    }
                    if let Ok(val) = serde_json::from_str::<Value>(data) {
                        // Usage
                        if let Some(usage) = val.get("usage") {
                            input_tokens = usage["prompt_tokens"].as_u64().unwrap_or(0) as u32;
                            output_tokens = usage["completion_tokens"].as_u64().unwrap_or(0) as u32;
                        }

                        if let Some(choices) = val["choices"].as_array() {
                            for choice in choices {
                                let delta = &choice["delta"];

                                // Text delta
                                if let Some(text) = delta["content"].as_str() {
                                    if !text.is_empty() {
                                        any_output_sent = true;
                                        let _ =
                                            tx.send(LlmChunk::TextDelta(text.to_string())).await;
                                    }
                                }

                                // Tool calls
                                if let Some(tool_calls) = delta["tool_calls"].as_array() {
                                    for tc in tool_calls {
                                        any_output_sent = true;
                                        let idx = tc["index"].as_u64().unwrap_or(0) as usize;
                                        let entry = tool_bufs
                                            .entry(idx)
                                            .or_insert_with(|| (String::new(), String::new(), String::new()));
                                        // id/name may arrive in a later delta than the first.
                                        if entry.0.is_empty() {
                                            if let Some(id) = tc["id"].as_str() {
                                                entry.0 = id.to_string();
                                            }
                                        }
                                        if entry.1.is_empty() {
                                            if let Some(n) = tc["function"]["name"].as_str() {
                                                entry.1 = n.to_string();
                                            }
                                        }
                                        if let Some(args) = tc["function"]["arguments"].as_str() {
                                            entry.2.push_str(args);
                                        }
                                    }
                                }

                                // Finish reason
                                if let Some("tool_calls") = choice["finish_reason"].as_str() {
                                    emit_buffered_tool_calls(&mut tool_bufs, &tx).await;
                                }
                            }
                        }
                    }
                }
            }
        }

        // The HTTP body ended without a `[DONE]` sentinel or a `finish_reason`
        // that would have drained `tool_bufs`. Silently returning `Ok(())` here
        // would look like the model produced a normal, tool-free response —
        // hiding a real mid-stream failure. Surface it explicitly instead.
        if !tool_bufs.is_empty() {
            let msg = undrained_tool_bufs_message(&tool_bufs);
            tracing::warn!("{}", msg);
            let _ = tx.send(LlmChunk::Error(msg.clone())).await;
            return Err(anyhow!(msg));
        }

        // The connection closed before delivering a single text delta, tool
        // call, or [DONE] — the model never got a chance to say anything.
        // This is functionally the streaming equivalent of the non-streaming
        // path's "OpenAI response returned empty choices" check below.
        if !any_output_sent {
            let msg = "protocol_error: stream ended with no text, no tool calls, and no [DONE] sentinel — connection likely closed before the response started".to_string();
            tracing::warn!("{}", msg);
            let _ = tx.send(LlmChunk::Error(msg.clone())).await;
            return Err(anyhow!(msg));
        }

        Ok(())
    }

    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse> {
        let mut req_no_stream = req.clone();
        req_no_stream.stream = false;
        let response = self.post_chat(&req_no_stream).await?;

        let body = response.bytes().await?;
        let val: Value = serde_json::from_slice(&body).map_err(|e| {
            let preview: String = String::from_utf8_lossy(&body).chars().take(200).collect();
            anyhow!(
                "OpenAI response JSON decode error: {} (body preview: {})",
                e,
                preview
            )
        })?;
        let choices = val["choices"]
            .as_array()
            .ok_or_else(|| anyhow!("OpenAI response missing 'choices' field"))?;
        if choices.is_empty() {
            return Err(anyhow!("OpenAI response returned empty choices"));
        }
        let message = &choices[0]["message"];
        let text = message["content"].as_str().unwrap_or("").to_string();

        let tool_calls = parse_tool_calls(message)?;

        Ok(LlmResponse {
            content: text,
            tool_calls,
            input_tokens: val["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            output_tokens: val["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parallel_tool_results_in_separate_messages_are_all_kept() {
        let client = OpenAiClient::new("k", "https://api.deepseek.com/v1");
        let msg = |role: &str, blocks: Vec<ContentBlock>| LlmMessage {
            role: role.to_string(),
            content: MessageContent::Blocks(blocks),
        };
        let use_block = |id: &str| ContentBlock::ToolUse {
            id: id.to_string(),
            name: "shell".to_string(),
            input: json!({}),
        };
        let result_block = |id: &str| ContentBlock::ToolResult {
            tool_use_id: id.to_string(),
            content: "ok".to_string(),
            is_error: false,
        };
        let messages = vec![
            msg("assistant", vec![use_block("a"), use_block("b")]),
            msg("user", vec![result_block("a")]),
            msg("user", vec![result_block("b")]),
        ];
        let out = client.convert_messages(&messages, false);
        let tool_ids: Vec<&str> = out
            .iter()
            .filter(|v| v["role"] == "tool")
            .filter_map(|v| v["tool_call_id"].as_str())
            .collect();
        assert_eq!(tool_ids, vec!["a", "b"]);
    }

    fn request_for_model(model: &str) -> LlmRequest {
        LlmRequest {
            messages: vec![LlmMessage {
                role: "user".to_string(),
                content: MessageContent::text("hello"),
            }],
            system: None,
            tools: Vec::new(),
            model: model.to_string(),
            max_tokens: 128,
            stream: false,
            vision_override: Some(false),
        }
    }

    #[test]
    fn dashscope_qwen_disables_thinking_mode() {
        let client = OpenAiClient::new(
            "test-key",
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
        );
        let body = client.build_body(&request_for_model("qwen3.6-plus"));

        assert_eq!(body["enable_thinking"], Value::Bool(false));
    }

    #[test]
    fn non_dashscope_openai_payload_does_not_add_qwen_flag() {
        let client = OpenAiClient::new("test-key", "https://api.openai.com/v1");
        let body = client.build_body(&request_for_model("gpt-4o"));

        assert!(body.get("enable_thinking").is_none());
    }

    #[test]
    fn deepseek_disables_thinking_mode() {
        let client = OpenAiClient::new("test-key", "https://api.deepseek.com/v1");
        let body = client.build_body(&request_for_model("deepseek-v4-flash"));

        assert_eq!(body["thinking"], json!({ "type": "disabled" }));
    }

    #[test]
    fn deepseek_current_aliases_disable_unpersisted_reasoning_on_tool_replay() {
        let client = OpenAiClient::new("test-key", "https://api.deepseek.com/v1");
        for model in ["deepseek-flash", "gateway/deepseek-flash"] {
            let body = client.build_body(&request_for_model(model));
            assert_eq!(body["thinking"], json!({ "type": "disabled" }));
            assert!(body.get("enable_thinking").is_none());
        }
        assert!(!is_deepseek_thinking_model("deepseek-chat"));
        assert!(!is_deepseek_thinking_model("other-flash"));
    }

    #[test]
    fn client_options_apply_sampling_and_thinking() {
        let client = OpenAiClient::new("test-key", "https://api.deepseek.com/v1").with_options(
            crate::llm::ClientOptions {
                temperature: Some(0.3),
                top_p: Some(0.9),
                thinking: Some(true),
            },
        );
        let body = client.build_body(&request_for_model("deepseek-v4-flash"));

        assert_eq!(body["temperature"], json!(0.3f32));
        assert_eq!(body["top_p"], json!(0.9f32));
        assert_eq!(body["thinking"], json!({ "type": "enabled" }));
    }

    #[test]
    fn ordinary_deepseek_chat_does_not_add_thinking_flag() {
        let client = OpenAiClient::new("test-key", "https://api.deepseek.com/v1");
        let body = client.build_body(&request_for_model("deepseek-chat"));

        assert!(body.get("thinking").is_none());
    }

    fn request_with_tool_history(model: &str) -> LlmRequest {
        let mut req = request_for_model(model);
        req.messages = vec![
            LlmMessage {
                role: "user".into(),
                content: MessageContent::text("hi"),
            },
            LlmMessage {
                role: "assistant".into(),
                content: MessageContent::Blocks(vec![ContentBlock::ToolUse {
                    id: "call_1".into(),
                    name: "file_read".into(),
                    input: json!({"path": "a.txt"}),
                }]),
            },
            LlmMessage {
                role: "user".into(),
                content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                    tool_use_id: "call_1".into(),
                    content: "ok".into(),
                    is_error: false,
                }]),
            },
        ];
        req
    }

    #[test]
    fn thinking_disabled_once_history_has_tool_calls() {
        let client = OpenAiClient::new("test-key", "https://api.deepseek.com/v1").with_options(
            crate::llm::ClientOptions {
                thinking: Some(true),
                ..Default::default()
            },
        );
        let body = client.build_body(&request_with_tool_history("deepseek-v4-flash"));
        assert_eq!(body["thinking"], json!({ "type": "disabled" }));
    }

    #[test]
    fn blocked_model_gets_explicit_thinking_disabled() {
        let client = OpenAiClient::new("test-key", "https://gw.example.com/v1");
        client.block_thinking("kimi-k2-thinking-blocktest");
        let body = client.build_body(&request_for_model("kimi-k2-thinking-blocktest"));
        assert_eq!(body["thinking"], json!({ "type": "disabled" }));
        let other = client.build_body(&request_for_model("gpt-4o"));
        assert!(other.get("thinking").is_none());
    }

    #[test]
    fn detects_reasoning_replay_error() {
        assert!(is_reasoning_replay_error(
            r#"{"error":{"message":"The `reasoning_content` in the thinking mode must be passed back to the API."}}"#
        ));
        assert!(!is_reasoning_replay_error("invalid api key"));
    }

    #[test]
    fn streamed_tool_call_valid_json_yields_tool_use() {
        let chunk = parse_streamed_tool_call(
            "call_1".into(),
            "file_write".into(),
            r#"{"path":"a.txt","content":"hi"}"#.into(),
        );
        match chunk {
            LlmChunk::ToolUse { id, name, input } => {
                assert_eq!(id, "call_1");
                assert_eq!(name, "file_write");
                assert_eq!(input["path"], "a.txt");
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }

    #[test]
    fn streamed_tool_call_unrepairable_truncation_yields_error_not_empty_object() {
        // Cut off right after a key's colon, with no value at all — the
        // P2-2 conservative repair explicitly refuses to guess a value here
        // ("不做字段脑补"), so this must still surface as tool_args_invalid,
        // and must NOT become `{}`.
        let chunk = parse_streamed_tool_call(
            "call_2".into(),
            "file_write".into(),
            r#"{"path":"a.txt","content":"#.into(),
        );
        match chunk {
            LlmChunk::Error(msg) => {
                assert!(msg.contains("tool_args_invalid"), "msg={msg}");
                assert!(msg.contains("file_write"), "msg={msg}");
                assert!(msg.contains("call_2"), "msg={msg}");
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn streamed_tool_call_repairable_truncation_recovers_via_json_repair() {
        // P2-2: a string value cut off mid-stream is exactly the case the
        // conservative repair handles — it must recover with the verbatim
        // (truncated) content, not error out and not invent anything extra.
        let chunk = parse_streamed_tool_call(
            "call_2".into(),
            "file_write".into(),
            r#"{"path":"a.txt","content":"this got cut of"#.into(),
        );
        match chunk {
            LlmChunk::ToolUse { id, name, input } => {
                assert_eq!(id, "call_2");
                assert_eq!(name, "file_write");
                assert_eq!(input["path"], "a.txt");
                assert_eq!(input["content"], "this got cut of");
            }
            other => panic!("expected repaired ToolUse, got {other:?}"),
        }
    }

    #[test]
    fn streamed_tool_call_empty_args_yields_error_not_empty_object() {
        // A tool call whose arguments never arrived at all (e.g. the model
        // emitted the call but the connection dropped before any argument
        // deltas) must be flagged, not treated as "no arguments" == `{}`.
        let chunk = parse_streamed_tool_call("call_3".into(), "search".into(), String::new());
        assert!(matches!(chunk, LlmChunk::Error(ref m) if m.contains("tool_args_invalid")));
    }

    #[tokio::test]
    async fn emit_buffered_tool_calls_sends_error_for_bad_and_tooluse_for_good() {
        let mut bufs = std::collections::HashMap::new();
        bufs.insert(
            0usize,
            (
                "good".to_string(),
                "search".to_string(),
                r#"{"query":"x"}"#.to_string(),
            ),
        );
        bufs.insert(
            1usize,
            (
                "bad".to_string(),
                "file_write".to_string(),
                "{broken".to_string(),
            ),
        );

        let (tx, mut rx) = tokio::sync::mpsc::channel(4);
        emit_buffered_tool_calls(&mut bufs, &tx).await;
        drop(tx);

        let mut saw_tool_use = false;
        let mut saw_error = false;
        while let Some(chunk) = rx.recv().await {
            match chunk {
                LlmChunk::ToolUse { name, .. } if name == "search" => saw_tool_use = true,
                LlmChunk::Error(msg) if msg.contains("tool_args_invalid") => saw_error = true,
                other => panic!("unexpected chunk: {other:?}"),
            }
        }
        assert!(saw_tool_use && saw_error);
        assert!(bufs.is_empty());
    }

    #[test]
    fn undrained_tool_bufs_message_names_pending_calls() {
        let mut bufs = std::collections::HashMap::new();
        bufs.insert(
            0usize,
            (
                "call_x".to_string(),
                "file_write".to_string(),
                "{\"path\":".to_string(),
            ),
        );
        let msg = undrained_tool_bufs_message(&bufs);
        assert!(msg.contains("protocol_error"));
        assert!(msg.contains("file_write"));
        assert!(msg.contains("call_x"));
    }

    #[test]
    fn parse_tool_calls_valid_json() {
        let message = json!({
            "tool_calls": [{
                "id": "call_1",
                "function": { "name": "search", "arguments": "{\"query\":\"piscis\"}" }
            }]
        });
        let calls = parse_tool_calls(&message).expect("should parse");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "search");
        assert_eq!(calls[0].input["query"], "piscis");
    }

    #[test]
    fn parse_tool_calls_unrepairable_truncation_errs_instead_of_defaulting() {
        // Cut off right after a colon — repair explicitly refuses to guess
        // the missing value ("不做字段脑补"), so this must still error.
        let message = json!({
            "tool_calls": [{
                "id": "call_2",
                "function": { "name": "file_write", "arguments": "{\"path\":\"a.txt\",\"content\":" }
            }]
        });
        let err = parse_tool_calls(&message).expect_err("should error, not default to {}");
        let msg = err.to_string();
        assert!(msg.contains("tool_args_invalid"), "msg={msg}");
        assert!(msg.contains("file_write"), "msg={msg}");
    }

    #[test]
    fn parse_tool_calls_repairable_truncation_recovers_via_json_repair() {
        // P2-2: a string value cut off mid-stream is repaired, not rejected.
        let message = json!({
            "tool_calls": [{
                "id": "call_2",
                "function": { "name": "file_write", "arguments": "{\"path\":\"a.txt\",\"content\":\"cut of" }
            }]
        });
        let calls = parse_tool_calls(&message).expect("should recover via repair");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].input["path"], "a.txt");
        assert_eq!(calls[0].input["content"], "cut of");
    }

    #[test]
    fn parse_tool_calls_empty_arguments_errs() {
        let message = json!({
            "tool_calls": [{
                "id": "call_3",
                "function": { "name": "search" }
            }]
        });
        let err = parse_tool_calls(&message).expect_err("missing arguments must error");
        assert!(err.to_string().contains("tool_args_invalid"));
    }

    #[test]
    fn parse_tool_calls_drops_empty_name_and_synthesizes_id() {
        let message = json!({
            "tool_calls": [
                {"id": "a", "function": {"name": "", "arguments": "{}"}},
                {"function": {"name": "search", "arguments": "{}"}}
            ]
        });
        let calls = parse_tool_calls(&message).expect("ok");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "search");
        assert!(!calls[0].id.is_empty());
    }

    #[tokio::test]
    async fn emit_buffered_drops_empty_name_and_orders_by_index() {
        let mut bufs = std::collections::HashMap::new();
        bufs.insert(1usize, ("b".into(), "t2".into(), "{}".into()));
        bufs.insert(0usize, ("a".into(), "t1".into(), "{}".into()));
        bufs.insert(2usize, ("c".into(), "".into(), "{}".into()));
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        emit_buffered_tool_calls(&mut bufs, &tx).await;
        drop(tx);
        let mut names = Vec::new();
        while let Some(c) = rx.recv().await {
            if let LlmChunk::ToolUse { name, .. } = c {
                names.push(name);
            }
        }
        assert_eq!(names, vec!["t1", "t2"]);
    }

    #[test]
    fn parse_tool_calls_no_tool_calls_field_returns_empty() {
        let message = json!({ "content": "just text" });
        let calls = parse_tool_calls(&message).expect("should not error");
        assert!(calls.is_empty());
    }

    #[test]
    fn qwen_text_only_models_are_not_vision() {
        // Regression: qwen3.7-max is a text-only model; must NOT be treated as vision.
        // Sending image content arrays to it causes DashScope 400 errors.
        assert!(!model_supports_vision("qwen3.7-max"));
        assert!(!model_supports_vision("qwen3-max"));
        assert!(!model_supports_vision("qwen3-235b-a22b"));
        assert!(!model_supports_vision("qwen-plus"));
        // But VL variants should be detected
        assert!(model_supports_vision("qwen3-vl-plus"));
        assert!(model_supports_vision("qwen-vl-max"));
        assert!(model_supports_vision("qwen2.5-vl-72b"));
    }
}
