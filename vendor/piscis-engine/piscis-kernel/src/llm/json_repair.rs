//! Extremely conservative, best-effort repair for tool-call `arguments`
//! JSON that was truncated mid-stream (e.g. cut off exactly at a provider's
//! token/byte limit).
//!
//! This is deliberately narrow (P2-2, OpenAI 兼容链路鲁棒性计划): it only
//! re-balances brackets/braces and closes an unterminated string that were
//! left open by truncation. It never invents field values, never drops
//! content, and never guesses what a dangling `"key":` was supposed to
//! contain. If the repaired string still doesn't parse, or the input
//! doesn't look like a simple truncation (e.g. genuinely malformed JSON,
//! mismatched brackets), it returns `None` and the caller falls through to
//! the normal `tool_args_invalid` error path unchanged.

use serde_json::Value;

/// Attempt to repair a truncated JSON document. Only call this *after*
/// `serde_json::from_str` has already failed — a successful parse means
/// there's nothing to repair, and calling this on well-formed input that
/// merely has trailing garbage would incorrectly report a "successful"
/// repair with fabricated structure.
pub fn try_conservative_repair(input: &str) -> Option<Value> {
    if input.trim().is_empty() {
        return None;
    }

    let mut stack: Vec<char> = Vec::new();
    let mut in_string = false;
    let mut escape = false;

    for ch in input.chars() {
        if in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' | '[' => stack.push(ch),
            '}' => {
                if stack.pop() != Some('{') {
                    return None; // mismatched — not a simple truncation
                }
            }
            ']' => {
                if stack.pop() != Some('[') {
                    return None;
                }
            }
            _ => {}
        }
    }

    // A "clean" scan (balanced brackets, no open string) means the parse
    // failure wasn't caused by truncation — e.g. trailing garbage after an
    // otherwise complete value. Repairing that would require guessing, so
    // bail out instead of fabricating anything.
    if stack.is_empty() && !in_string {
        return None;
    }

    let mut repaired = input.trim_end().to_string();

    if in_string {
        // The cut happened mid-string-value: close it exactly where the
        // stream stopped, adding no characters to the value itself.
        repaired.push('"');
    } else {
        // The cut happened between JSON tokens (e.g. right after a comma,
        // before the next key). A trailing comma would make the
        // re-balanced JSON invalid (`{"a":1,}`), so drop it — this removes
        // punctuation only, never field content.
        while repaired.ends_with(',') {
            repaired.pop();
            repaired = repaired.trim_end().to_string();
        }
    }

    for open in stack.into_iter().rev() {
        repaired.push(match open {
            '{' => '}',
            '[' => ']',
            _ => unreachable!("stack only ever contains '{{' or '['"),
        });
    }

    // Final, authoritative check: if this doesn't parse, we don't know how
    // to fix it conservatively (e.g. a dangling `"key":` with no value) —
    // report failure rather than guessing further.
    serde_json::from_str::<Value>(&repaired).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn repairs_truncated_string_value() {
        let input = r#"{"path":"a.txt","content":"this got cut of"#;
        let repaired = try_conservative_repair(input).expect("should repair");
        assert_eq!(
            repaired,
            json!({"path": "a.txt", "content": "this got cut of"})
        );
    }

    #[test]
    fn repairs_truncated_nested_array() {
        let input = r#"{"items":["a","b","c"#;
        let repaired = try_conservative_repair(input).expect("should repair");
        assert_eq!(repaired, json!({"items": ["a", "b", "c"]}));
    }

    #[test]
    fn repairs_dangling_trailing_comma_before_close() {
        // Cut off right after a comma, before the next key ever started.
        let input = r#"{"a":1,"b":2,"#;
        let repaired = try_conservative_repair(input).expect("should repair");
        assert_eq!(repaired, json!({"a": 1, "b": 2}));
    }

    #[test]
    fn refuses_to_guess_a_dangling_key_with_no_value() {
        // Cut off right after a key's colon — repairing this would require
        // inventing a value, which is out of scope ("不做字段脑补").
        let input = r#"{"path":"a.txt","content":"#;
        assert!(try_conservative_repair(input).is_none());
    }

    #[test]
    fn refuses_mismatched_brackets() {
        // Not a truncation — a stray closing bracket with no matching open.
        let input = r#"{"a":1}}"#;
        assert!(try_conservative_repair(input).is_none());
    }

    #[test]
    fn refuses_when_nothing_looks_truncated() {
        // Balanced and no open string — this parse failure (if any) isn't a
        // simple truncation we know how to fix (e.g. trailing garbage).
        let input = r#"{"a":1} garbage"#;
        assert!(try_conservative_repair(input).is_none());
    }

    #[test]
    fn refuses_empty_input() {
        assert!(try_conservative_repair("").is_none());
        assert!(try_conservative_repair("   ").is_none());
    }

    #[test]
    fn does_not_fabricate_fields_beyond_what_was_present() {
        // Sanity check on the "no field invention" contract: every key that
        // appears in the repaired value must have been present verbatim in
        // the truncated input.
        let input = r#"{"query":"piscis kernel","limit":5,"filters":["rust"#;
        let repaired = try_conservative_repair(input).expect("should repair");
        assert_eq!(
            repaired,
            json!({"query": "piscis kernel", "limit": 5, "filters": ["rust"]})
        );
    }
}
