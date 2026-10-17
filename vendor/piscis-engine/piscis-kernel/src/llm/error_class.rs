//! Central classification of LLM/tool failures into a small, stable set of
//! machine-readable codes. Shared by:
//! - `agent::loop_` (classify bounded retry/fallback decisions and select the
//!   next fallback model), and
//! - `AgentEvent::Error.code` (surfaced to the frontend so it can render a
//!   category-specific message instead of a raw upstream string).
//!
//! Classification is heuristic — it works on the already-flattened error
//! `String` (produced by `anyhow`/`format!` call sites across the client
//! implementations), not on structured HTTP responses. This keeps it usable
//! from any call site without threading status codes through every error
//! path, at the cost of being best-effort rather than exhaustive.

use serde::{Deserialize, Serialize};

/// Stable machine-readable error codes shared across piscis-kernel,
/// DimWork and DimRouter/DimSDK (see plan doc "错误契约").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorClass {
    /// The requested model/deployment is not supported by the configured
    /// account/group, or does not exist in the provider's or DimRouter's
    /// catalogue. Eligible for automatic fallback to another model.
    ModelUnavailable,
    /// A tool call's `arguments` JSON was missing, truncated, or otherwise
    /// failed to parse. Must never be silently treated as `{}`.
    ToolArgsInvalid,
    /// The upstream response violated the expected protocol shape (empty
    /// `choices`, stream ended without `[DONE]`/`finish_reason`, undecodable
    /// body, etc.) rather than reporting a normal error.
    ProtocolError,
    /// Provider-side rate limiting. Eligible for automatic fallback.
    RateLimited,
    /// Authentication/authorization failure (bad/expired API key, etc.).
    /// Not eligible for fallback — switching models won't help.
    AuthFailed,
    /// Account billing/credit exhaustion. Neither retries nor model fallback restore funds.
    QuotaExceeded,
    /// Transient upstream failure (timeout, connection reset, 5xx,
    /// mid-stream disconnects). Should be retried with backoff on the same
    /// model, not switched away from.
    UpstreamTransient,
    /// Provider thinking mode demanded a `reasoning_content` replay that the
    /// client cannot supply. The client already retried with thinking off;
    /// if this still surfaces, the user must pick a non-thinking model.
    ReasoningReplayRequired,
    /// Doesn't match any known category.
    Unknown,
}

impl ErrorClass {
    /// The stable string code used in `AgentEvent::Error.code`, DimWork's
    /// `formatChatError`, and platform error envelopes.
    pub fn code(&self) -> &'static str {
        match self {
            Self::ModelUnavailable => "model_unavailable",
            Self::ToolArgsInvalid => "tool_args_invalid",
            Self::ProtocolError => "protocol_error",
            Self::RateLimited => "rate_limited",
            Self::AuthFailed => "auth_failed",
            Self::QuotaExceeded => "quota_exceeded",
            Self::UpstreamTransient => "upstream_transient",
            Self::ReasoningReplayRequired => "reasoning_replay_required",
            Self::Unknown => "unknown",
        }
    }

    /// Only temporary upstream failures may retry the same model.
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::UpstreamTransient)
    }

    /// True if a caller should try the next configured fallback model
    /// instead of retrying the same model or giving up outright.
    ///
    /// Intentionally excludes `UpstreamTransient` ("overloaded", timeouts,
    /// 502/503/529, mid-stream disconnects): those are retried with
    /// exponential backoff on the *same* model by the caller, since they
    /// say nothing about whether the model itself is usable.
    pub fn is_fallback_eligible(&self) -> bool {
        matches!(self, Self::ModelUnavailable | Self::RateLimited)
    }
}

/// Best-effort extraction of a leading HTTP status code from error strings
/// shaped like `"OpenAI API error 404 Not Found: {...}"` (see
/// `llm::openai::OpenAiClient`) or `"... error 429: ..."`.
fn extract_http_status(lower_msg: &str) -> Option<u16> {
    let idx = lower_msg.find("error ")?;
    let rest = &lower_msg[idx + "error ".len()..];
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.len() == 3 {
        digits.parse().ok()
    } else {
        None
    }
}

/// Classify a flattened error message string into an [`ErrorClass`].
///
/// This is intentionally conservative about `ModelUnavailable`: a bare 404
/// is not enough (many providers 404 for unrelated reasons), so we require
/// either an explicit model-not-found phrase or a 404 whose body also
/// mentions "model".
pub fn classify_error(msg: &str) -> ErrorClass {
    let lower = msg.to_lowercase();
    let status = extract_http_status(&lower);

    if lower.contains("auth_failed")
        || lower.contains("unauthorized")
        || lower.contains("invalid api key")
        || lower.contains("incorrect api key")
        || status == Some(401)
        || status == Some(403)
    {
        return ErrorClass::AuthFailed;
    }

    if status == Some(402)
        || lower.contains("insufficient_quota")
        || lower.contains("billing_hard_limit_reached")
        || lower.contains("insufficient_balance")
        || lower.contains("insufficient balance")
        || lower.contains("credit balance is too low")
        || lower.contains("credits exhausted")
        || lower.contains("payment_required")
    {
        return ErrorClass::QuotaExceeded;
    }

    if lower.contains("tool_args_invalid") {
        return ErrorClass::ToolArgsInvalid;
    }

    if crate::llm::openai::is_reasoning_replay_error(msg) {
        return ErrorClass::ReasoningReplayRequired;
    }

    // Explicit HTTP status takes precedence over incidental model/transient words.
    if status.is_some_and(|status| (500..=599).contains(&status)) {
        return ErrorClass::UpstreamTransient;
    }

    if lower.contains("model_not_found")
        || lower.contains("model not found")
        || lower.contains("model_unavailable")
        || lower.contains("does not exist")
        || lower.contains("not supported by any configured account")
        || lower.contains("not in the catalogue")
        || lower.contains("not in the catalog")
        || (status == Some(404) && lower.contains("model"))
    {
        return ErrorClass::ModelUnavailable;
    }

    if lower.contains("rate_limit") || lower.contains("rate limit") || status == Some(429) {
        return ErrorClass::RateLimited;
    }

    if lower.contains("protocol_error")
        || lower.contains("missing 'choices'")
        || lower.contains("returned empty choices")
        || lower.contains("json decode error")
        || lower.contains("response json decode error")
    {
        return ErrorClass::ProtocolError;
    }

    // Other HTTP failures are permanent/unknown here; incidental body text must
    // not turn a 400/404 into a transient network failure.
    if status.is_some() {
        return ErrorClass::Unknown;
    }

    if lower.contains("timeout")
        || lower.contains("connection")
        || lower.contains("overloaded")
        || lower.contains("error decoding response body")
        || lower.contains("incomplete message")
        || lower.contains("unexpected eof")
        || lower.contains("broken pipe")
    {
        return ErrorClass::UpstreamTransient;
    }

    ErrorClass::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permanent_auth_dominates_mixed_retry_and_fallback_words() {
        for msg in [
            "OpenAI API error 401 Unauthorized: connection timeout rate_limit model_not_found",
            "OpenAI API error 403 Forbidden: overloaded 503",
            "invalid API key: connection rejected",
            "OpenAI API error 429: auth_failed, retry after timeout",
            "OpenAI API error 500: incorrect api key",
        ] {
            let class = classify_error(msg);
            assert_eq!(class, ErrorClass::AuthFailed, "{msg}");
            assert!(!class.is_retryable());
            assert!(!class.is_fallback_eligible());
        }
    }

    #[test]
    fn account_exhaustion_dominates_rate_limits_and_transient_words() {
        for msg in [
            "OpenAI API error 429: insufficient_quota, timeout rate_limit",
            "OpenAI API error 402 Payment Required: overloaded",
            "OpenAI API error 503: billing_hard_limit_reached",
            "insufficient balance: connection unavailable",
            "credit balance is too low",
            "credits exhausted",
        ] {
            let class = classify_error(msg);
            assert_eq!(class, ErrorClass::QuotaExceeded, "{msg}");
            assert!(!class.is_retryable());
            assert!(!class.is_fallback_eligible());
        }
        // A periodic rate quota is not evidence of exhausted account funds.
        assert_eq!(
            classify_error("OpenAI API error 429: per-minute quota exceeded"),
            ErrorClass::RateLimited
        );
    }

    #[test]
    fn http_server_errors_retry_but_permanent_status_bodies_do_not() {
        for status in [500, 502, 503, 504, 529, 599] {
            let class = classify_error(&format!("OpenAI API error {status}: temporary failure"));
            assert_eq!(class, ErrorClass::UpstreamTransient);
            assert!(class.is_retryable());
            assert!(!class.is_fallback_eligible());
        }
        for msg in [
            "OpenAI API error 400 Bad Request: connection timeout happened earlier 503",
            "OpenAI API error 404: route missing, connection unavailable 502",
            "Unexpected identifier 502 in response",
        ] {
            assert_eq!(classify_error(msg), ErrorClass::Unknown, "{msg}");
            assert!(!classify_error(msg).is_retryable());
        }
        assert!(classify_error("CONNECTION RESET BY PEER").is_retryable());
    }

    #[test]
    fn quota_wire_code_is_additive_and_old_codes_round_trip() {
        let class = ErrorClass::QuotaExceeded;
        assert_eq!(class.code(), "quota_exceeded");
        assert_eq!(serde_json::to_string(&class).unwrap(), "\"quota_exceeded\"");
        assert_eq!(
            serde_json::from_str::<ErrorClass>("\"quota_exceeded\"").unwrap(),
            class
        );
        for class in [
            ErrorClass::AuthFailed,
            ErrorClass::RateLimited,
            ErrorClass::ToolArgsInvalid,
            ErrorClass::Unknown,
        ] {
            assert_eq!(
                serde_json::from_str::<ErrorClass>(&format!("\"{}\"", class.code())).unwrap(),
                class
            );
        }
    }

    #[test]
    fn classifies_group_404_as_model_unavailable() {
        // Real-world sample reported by a user via a compatible-API proxy group.
        let msg = r#"OpenAI API error 404 Not Found: {"error":{"message":"Model \"gpt-5.2\" is not supported by any configured account in this group","type":"model_not_found","param":null,"code":null}}"#;
        assert_eq!(classify_error(msg), ErrorClass::ModelUnavailable);
        assert!(classify_error(msg).is_fallback_eligible());
    }

    #[test]
    fn classifies_explicit_model_not_found_phrase() {
        assert_eq!(
            classify_error("upstream said: model_not_found"),
            ErrorClass::ModelUnavailable
        );
        assert_eq!(
            classify_error("Error: model not found for this account"),
            ErrorClass::ModelUnavailable
        );
    }

    #[test]
    fn classifies_does_not_exist_as_model_unavailable() {
        assert_eq!(
            classify_error("The model `gpt-9` does not exist"),
            ErrorClass::ModelUnavailable
        );
    }

    #[test]
    fn classifies_router_catalogue_miss() {
        assert_eq!(
            classify_error("requested model is not in the catalogue"),
            ErrorClass::ModelUnavailable
        );
    }

    #[test]
    fn bare_404_without_model_mention_is_not_model_unavailable() {
        // A 404 that has nothing to do with the model (e.g. wrong path) must
        // not be misclassified — otherwise callers would burn fallback
        // attempts for unrelated failures.
        let msg = "OpenAI API error 404 Not Found: {\"error\":\"route not found\"}";
        assert_ne!(classify_error(msg), ErrorClass::ModelUnavailable);
    }

    #[test]
    fn classifies_rate_limit_variants() {
        assert_eq!(
            classify_error("rate_limit_exceeded"),
            ErrorClass::RateLimited
        );
        assert_eq!(
            classify_error("You have hit the rate limit, slow down"),
            ErrorClass::RateLimited
        );
        assert_eq!(
            classify_error("OpenAI API error 429 Too Many Requests: {}"),
            ErrorClass::RateLimited
        );
        assert!(classify_error("rate_limit_exceeded").is_fallback_eligible());
    }

    #[test]
    fn classifies_auth_failures_as_not_fallback_eligible() {
        let class = classify_error("OpenAI API error 401 Unauthorized: invalid api key provided");
        assert_eq!(class, ErrorClass::AuthFailed);
        assert!(!class.is_fallback_eligible());
    }

    #[test]
    fn classifies_transient_upstream_errors() {
        for msg in [
            "request timeout after 120s",
            "OpenAI API error 503 Service Unavailable: overloaded",
            "error decoding response body: unexpected eof",
            "connection reset by peer",
        ] {
            let class = classify_error(msg);
            assert_eq!(class, ErrorClass::UpstreamTransient, "msg={msg}");
            assert!(!class.is_fallback_eligible(), "msg={msg}");
        }
    }

    #[test]
    fn classifies_protocol_errors() {
        assert_eq!(
            classify_error("OpenAI response returned empty choices"),
            ErrorClass::ProtocolError
        );
        assert_eq!(
            classify_error("OpenAI response missing 'choices' field"),
            ErrorClass::ProtocolError
        );
    }

    #[test]
    fn unknown_falls_through() {
        assert_eq!(
            classify_error("something totally unexpected"),
            ErrorClass::Unknown
        );
        assert!(!classify_error("something totally unexpected").is_fallback_eligible());
    }

    #[test]
    fn tool_args_invalid_takes_priority() {
        // Constructed by the tool-json handling (p0-tool-json); make sure the
        // marker itself round-trips even if the message also mentions other
        // keywords incidentally.
        assert_eq!(
            classify_error("tool_args_invalid: failed to parse arguments for file_write"),
            ErrorClass::ToolArgsInvalid
        );
    }
}
