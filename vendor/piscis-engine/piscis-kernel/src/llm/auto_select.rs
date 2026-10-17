//! Client-side `auto`: pick one concrete catalogue id before the agent loop.
//!
//! The gateway never sees `"auto"`. On the first user turn of a question a
//! cheap live model names an id from the candidate set (billed to the caller).
//! Tool-loop continuations reuse the session pin. The pin is replaced only
//! when the current model cannot take the attached modality.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use serde_json::Value;

use super::{LlmClient, LlmMessage, LlmRequest, MessageContent};

pub const AUTO_SENTINEL: &str = "auto";
const USER_EXCERPT_CHARS: usize = 1_500;

#[derive(Debug, Clone, Default)]
pub struct Modality {
    pub need_vision: bool,
    pub need_audio: bool,
    pub need_video: bool,
}

#[derive(Debug, Clone)]
pub struct ModelCandidate {
    pub id: String,
    pub supports_vision: bool,
    pub supports_audio: bool,
}

#[derive(Debug, Default)]
pub struct SessionPins {
    /// session_id → pinned catalogue id
    inner: HashMap<String, String>,
}

impl SessionPins {
    pub fn get(&self, session_id: &str) -> Option<&str> {
        if session_id.is_empty() {
            return None;
        }
        self.inner.get(session_id).map(String::as_str)
    }

    pub fn set(&mut self, session_id: &str, model_id: &str) {
        if session_id.is_empty() || model_id.is_empty() {
            return;
        }
        self.inner
            .insert(session_id.to_string(), model_id.to_string());
    }
}

fn global_pins() -> &'static Mutex<SessionPins> {
    static PINS: OnceLock<Mutex<SessionPins>> = OnceLock::new();
    PINS.get_or_init(|| Mutex::new(SessionPins::default()))
}

pub fn with_session_pins<R>(f: impl FnOnce(&mut SessionPins) -> R) -> R {
    let mut pins = global_pins().lock().unwrap_or_else(|e| e.into_inner());
    f(&mut pins)
}

pub async fn classify_model(
    client: &dyn LlmClient,
    picker_model: &str,
    candidates: &[ModelCandidate],
    messages: &[LlmMessage],
) -> Option<String> {
    if picker_model.trim().is_empty() || candidates.is_empty() {
        return None;
    }
    let allowed: Vec<String> = candidates
        .iter()
        .filter(|c| !is_reserved_catalogue_id(&c.id))
        .map(|c| c.id.clone())
        .collect();
    let prompt = classifier_messages(candidates, messages);
    let req = LlmRequest {
        messages: vec![
            LlmMessage {
                role: "system".into(),
                content: MessageContent::text(
                    "You choose which catalogue model should answer this request. Use the catalogue facts. Do not invent ids.",
                ),
            },
            LlmMessage {
                role: "user".into(),
                content: MessageContent::text(prompt),
            },
        ],
        system: None,
        tools: vec![],
        model: picker_model.to_string(),
        max_tokens: 48,
        stream: false,
        vision_override: Some(false),
    };
    let result = client.complete(req).await.ok()?;
    parse_model_choice(&result.content, &allowed)
}

pub fn is_auto_alias(model: &str) -> bool {
    let key = model.trim().to_ascii_lowercase();
    key.is_empty()
        || key == AUTO_SENTINEL
        || key == "__auto__"
        || key == "default"
        || key == "tier:auto"
        || key.starts_with("auto:")
        || key.starts_with("tier:")
}

pub fn is_reserved_catalogue_id(id: &str) -> bool {
    let key = id.trim().to_ascii_lowercase();
    key == AUTO_SENTINEL
        || key == "__auto__"
        || key == "default"
        || key == "tier:auto"
        || key.starts_with("auto:")
        || key.starts_with("tier:")
}

pub fn is_first_user_turn(messages: &[LlmMessage]) -> bool {
    let last_user = messages
        .iter()
        .rposition(|m| m.role == "user")
        .unwrap_or(usize::MAX);
    if last_user == usize::MAX {
        return true;
    }
    !messages[last_user + 1..]
        .iter()
        .any(|m| matches!(m.role.as_str(), "assistant" | "tool" | "function"))
}

pub fn inspect_modality(messages: &[LlmMessage]) -> Modality {
    let mut modality = Modality::default();
    for message in messages {
        match &message.content {
            MessageContent::Text(text) if text.starts_with("data:image") => {
                modality.need_vision = true;
            }
            MessageContent::Blocks(blocks) => {
                for block in blocks {
                    if matches!(block, super::ContentBlock::Image { .. }) {
                        modality.need_vision = true;
                    }
                }
            }
            _ => {}
        }
    }
    modality
}

pub fn supports_vision_heuristic(id: &str) -> bool {
    let lower = id.to_ascii_lowercase();
    lower.contains("gpt-4o")
        || lower.contains("gpt-4.1")
        || lower.contains("gpt-5")
        || lower.contains("claude")
        || lower.contains("gemini")
        || lower.contains("qwen-vl")
        || lower.contains("qwen2.5-vl")
        || lower.contains("qwen3-vl")
        || lower.contains("vision")
        || lower.contains("-vl")
        || lower.contains("4o-mini")
}

pub fn cheapest_score(id: &str) -> i32 {
    let lower = id.to_ascii_lowercase();
    let mut score = 50;
    for (token, delta) in [
        ("nano", -30),
        ("mini", -25),
        ("flash", -22),
        ("lite", -20),
        ("turbo", -18),
        ("haiku", -18),
        ("small", -15),
        ("fast", -12),
        ("pro", 10),
        ("plus", 8),
        ("max", 12),
        ("opus", 16),
        ("ultra", 18),
    ] {
        if lower.contains(token) {
            score += delta;
        }
    }
    score
}

pub fn model_fits(candidate: &ModelCandidate, modality: &Modality) -> bool {
    if modality.need_vision && !candidate.supports_vision {
        return false;
    }
    if modality.need_audio && !candidate.supports_audio {
        return false;
    }
    if modality.need_video && !candidate.supports_vision {
        return false;
    }
    true
}

pub fn cheapest_fitting(candidates: &[ModelCandidate], modality: &Modality) -> Option<String> {
    let mut fitting: Vec<&ModelCandidate> = candidates
        .iter()
        .filter(|c| !is_reserved_catalogue_id(&c.id) && model_fits(c, modality))
        .collect();
    if fitting.is_empty() {
        fitting = candidates
            .iter()
            .filter(|c| !is_reserved_catalogue_id(&c.id))
            .collect();
    }
    fitting.sort_by_key(|c| (cheapest_score(&c.id), c.id.clone()));
    fitting.first().map(|c| c.id.clone())
}

pub fn parse_model_choice(text: &str, allowed: &[String]) -> Option<String> {
    if text.trim().is_empty() || allowed.is_empty() {
        return None;
    }
    let allowed_set: std::collections::HashSet<&str> = allowed.iter().map(String::as_str).collect();
    for blob in extract_json_objects(text) {
        let model = blob
            .get("model")
            .or_else(|| blob.get("id"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if allowed_set.contains(model) {
            return Some(model.to_string());
        }
    }
    let mut ordered = allowed.to_vec();
    ordered.sort_by_key(|id| std::cmp::Reverse(id.len()));
    for id in ordered {
        if text.contains(&id) {
            return Some(id);
        }
    }
    None
}

fn extract_json_objects(blob: &str) -> Vec<Value> {
    let mut found = Vec::new();
    if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(blob.trim()) {
        found.push(Value::Object(map));
    }
    let bytes = blob.as_bytes();
    let mut start = None;
    for (i, ch) in bytes.iter().enumerate() {
        if *ch == b'{' {
            start = Some(i);
        } else if *ch == b'}' {
            if let Some(s) = start {
                if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&blob[s..=i]) {
                    found.push(Value::Object(map));
                }
                start = None;
            }
        }
    }
    found
}

pub fn user_excerpt(messages: &[LlmMessage]) -> String {
    for message in messages.iter().rev() {
        if message.role != "user" {
            continue;
        }
        let mut excerpt = message.content.as_text();
        if excerpt.chars().count() > USER_EXCERPT_CHARS {
            excerpt = excerpt.chars().take(USER_EXCERPT_CHARS).collect::<String>() + "…";
        }
        return excerpt;
    }
    String::new()
}

pub fn classifier_messages(candidates: &[ModelCandidate], messages: &[LlmMessage]) -> String {
    let cards: Vec<String> = candidates
        .iter()
        .map(|c| {
            format!(
                "id={} vision={} audio={}",
                c.id,
                if c.supports_vision { "yes" } else { "no" },
                if c.supports_audio { "yes" } else { "no" }
            )
        })
        .collect();
    let excerpt = user_excerpt(messages);
    format!(
        "Live catalogue (each line is one model you may name):\n{}\n\nUser request:\n{}\n\nReply with JSON only: {{\"model\":\"<id>\"}}. The id must be one of the catalogue ids above.",
        cards.join("\n"),
        if excerpt.is_empty() { "(empty)" } else { &excerpt }
    )
}

/// Resolve `auto` (or keep a named id) using an optional classifier result.
pub fn resolve_selection(
    requested: &str,
    session_id: &str,
    pins: &mut SessionPins,
    candidates: &[ModelCandidate],
    first_turn: bool,
    modality: &Modality,
    classified: Option<&str>,
) -> String {
    if !is_auto_alias(requested) {
        return requested.trim().to_string();
    }
    let allowed: Vec<String> = candidates
        .iter()
        .filter(|c| !is_reserved_catalogue_id(&c.id))
        .map(|c| c.id.clone())
        .collect();

    if !first_turn {
        if let Some(pin) = pins.get(session_id).map(str::to_string) {
            if let Some(candidate) = candidates.iter().find(|c| c.id == pin) {
                if model_fits(candidate, modality) {
                    return pin;
                }
            }
        }
    }

    if let Some(picked) = classified.filter(|id| allowed.iter().any(|a| a == *id)) {
        if let Some(candidate) = candidates.iter().find(|c| c.id == picked) {
            if model_fits(candidate, modality) {
                pins.set(session_id, picked);
                return picked.to_string();
            }
        }
    }

    let fallback = cheapest_fitting(candidates, modality).unwrap_or_else(|| {
        allowed
            .first()
            .cloned()
            .unwrap_or_else(|| requested.trim().to_string())
    });
    pins.set(session_id, &fallback);
    fallback
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::ContentBlock;

    fn cand(id: &str, vision: bool) -> ModelCandidate {
        ModelCandidate {
            id: id.to_string(),
            supports_vision: vision,
            supports_audio: false,
        }
    }

    #[test]
    fn first_user_turn_is_only_the_latest_question() {
        let user = LlmMessage {
            role: "user".into(),
            content: MessageContent::text("hi"),
        };
        let assistant = LlmMessage {
            role: "assistant".into(),
            content: MessageContent::text("ok"),
        };
        assert!(is_first_user_turn(&[user.clone()]));
        assert!(!is_first_user_turn(&[user.clone(), assistant.clone()]));
        assert!(is_first_user_turn(&[
            user.clone(),
            assistant,
            LlmMessage {
                role: "user".into(),
                content: MessageContent::text("second"),
            }
        ]));
    }

    #[test]
    fn parse_choice_stays_inside_the_catalogue() {
        let allowed = vec!["qwen-max".into(), "deepseek-chat".into()];
        assert_eq!(
            parse_model_choice(r#"{"model":"qwen-max"}"#, &allowed).as_deref(),
            Some("qwen-max")
        );
        assert_eq!(
            parse_model_choice("use deepseek-chat please", &allowed).as_deref(),
            Some("deepseek-chat")
        );
        assert_eq!(parse_model_choice(r#"{"model":"gpt-9"}"#, &allowed), None);
    }

    #[test]
    fn tool_loop_keeps_the_pin_unless_modality_breaks_it() {
        let candidates = vec![cand("qwen-max", false), cand("gpt-4o-mini", true)];
        let mut pins = SessionPins::default();
        pins.set("s1", "qwen-max");
        let text = Modality::default();
        assert_eq!(
            resolve_selection("auto", "s1", &mut pins, &candidates, false, &text, None),
            "qwen-max"
        );
        let vision = Modality {
            need_vision: true,
            ..Modality::default()
        };
        assert_eq!(
            resolve_selection("auto", "s1", &mut pins, &candidates, false, &vision, None),
            "gpt-4o-mini"
        );
    }

    #[test]
    fn named_request_is_not_rewritten() {
        let mut pins = SessionPins::default();
        let candidates = vec![cand("qwen-max", false)];
        assert_eq!(
            resolve_selection(
                "deepseek-chat",
                "s1",
                &mut pins,
                &candidates,
                true,
                &Modality::default(),
                Some("qwen-max")
            ),
            "deepseek-chat"
        );
    }

    #[test]
    fn inspect_detects_image_blocks() {
        let messages = vec![LlmMessage {
            role: "user".into(),
            content: MessageContent::Blocks(vec![
                ContentBlock::Text {
                    text: "看图".into(),
                },
                ContentBlock::Image {
                    source: crate::llm::ImageSource {
                        source_type: "base64".into(),
                        media_type: "image/png".into(),
                        data: "xx".into(),
                    },
                },
            ]),
        }];
        assert!(inspect_modality(&messages).need_vision);
    }
}
