//! Shared post-turn hooks: memory extract + background skill review.

use piscis_kernel::llm::{self, LlmMessage};
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::Mutex;

pub async fn run_post_turn_hooks(
    app: &AppHandle,
    project_db: Arc<Mutex<piscis_kernel::store::db::Database>>,
    session_id: String,
    messages: Vec<LlmMessage>,
    provider: String,
    api_key: String,
    base_url: Option<String>,
    model: String,
    max_tokens: u32,
    budget: crate::runtime::task_budget::BudgetScope,
) {
    if messages.is_empty() {
        return;
    }
    let Ok(global_ctx) = crate::commands::skill_evolution_ctx::SkillEvolutionCtx::open(app) else {
        return;
    };
    let mem_client = budget.guard(llm::build_client_with_timeout(
        &provider,
        &api_key,
        base_url.as_deref(),
        90,
    ));
    crate::commands::memory_extract::auto_extract_memories(
        project_db.clone(),
        session_id.clone(),
        messages.clone(),
        mem_client,
        model.clone(),
        max_tokens,
        "piscis".to_string(),
    )
    .await;
    crate::commands::skill_review::run_background_skill_review(
        global_ctx,
        project_db,
        session_id,
        messages,
        provider,
        api_key,
        base_url,
        model,
        max_tokens,
        "piscis".to_string(),
        budget,
    )
    .await;
}

/// Load recent session rows as simple user/assistant text messages for review.
pub fn messages_from_db_rows(rows: &[piscis_kernel::store::db::ChatMessage]) -> Vec<LlmMessage> {
    rows.iter()
        .filter(|m| m.role == "user" || m.role == "assistant")
        .map(|m| LlmMessage {
            role: m.role.clone(),
            content: llm::MessageContent::text(&m.content),
        })
        .collect()
}

/// Automatically distill 0–2 source-anchored repository engineering notes (`repo_notes`)
/// after a turn that modified files or diagnosed non-trivial codebase behavior.
pub async fn auto_extract_repo_notes(
    workspace_root: String,
    messages: Vec<LlmMessage>,
    provider: String,
    api_key: String,
    base_url: Option<String>,
    model: String,
    max_tokens: u32,
    budget: crate::runtime::task_budget::BudgetScope,
) {
    let root = std::path::PathBuf::from(workspace_root.trim());
    if !root.is_dir() || messages.is_empty() {
        return;
    }

    let assistant_text: String = messages
        .iter()
        .filter(|m| m.role == "assistant")
        .map(|m| m.content.as_text())
        .collect::<Vec<_>>()
        .join("\n");
    // Only run when the turn involved substantive engineering analysis or code paths.
    if assistant_text.chars().count() < 260
        || (!assistant_text.contains(".rs")
            && !assistant_text.contains(".ts")
            && !assistant_text.contains(".tsx")
            && !assistant_text.contains(".py")
            && !assistant_text.contains(".go"))
    {
        return;
    }

    let relevant: Vec<_> = messages
        .iter()
        .filter(|m| m.role == "user" || m.role == "assistant")
        .collect();
    let start = relevant.len().saturating_sub(8);
    let conv_excerpt: String = relevant[start..]
        .iter()
        .map(|m| {
            let text = m.content.as_text();
            let truncated: String = text.chars().take(700).collect();
            format!("{}: {}", m.role, truncated)
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let prompt = format!(
        "Analyze the following coding session turn. If and ONLY if the assistant discovered or enforced \
         a non-obvious repository invariant, bug pitfall, or cross-boundary dataflow contract tied to a \
         specific source file in this workspace, extract at most 2 notes as a JSON array.\n\
         If nothing durable or non-obvious was established, output exactly: []\n\n\
         JSON schema (array of 0..2 objects):\n\
         [{{\"scope_kind\":\"module|file|symbol\",\"scope_target\":\"src-tauri/commands\",\"kind\":\"invariant|pitfall|dataflow|decision\",\"title\":\"Short title\",\"body\":\"One-paragraph technical rule\",\"path\":\"repo/relative/file.rs\",\"start_line\":10,\"end_line\":40,\"symbol\":\"optional_fn_name\"}}]\n\n\
         Conversation:\n{conv_excerpt}\n\nJSON output:"
    );

    let client = budget.guard(llm::build_client_with_timeout(
        &provider,
        &api_key,
        base_url.as_deref(),
        60,
    ));
    let req = llm::LlmRequest {
        messages: vec![LlmMessage {
            role: "user".into(),
            content: llm::MessageContent::text(&prompt),
        }],
        system: Some(
            "You extract concise, source-anchored engineering invariants from coding sessions. Output valid JSON array only.".into(),
        ),
        tools: vec![],
        model,
        max_tokens: max_tokens.min(600),
        stream: false,
        vision_override: None,
    };

    let Ok(resp) = client.complete(req).await else {
        return;
    };
    let raw = resp.content.trim();
    let Some(start_bracket) = raw.find('[') else {
        return;
    };
    let Some(end_bracket) = raw.rfind(']') else {
        return;
    };
    if end_bracket <= start_bracket {
        return;
    }
    let slice = &raw[start_bracket..=end_bracket];
    let Ok(items) = serde_json::from_str::<Vec<serde_json::Value>>(slice) else {
        return;
    };

    let _ = tokio::task::spawn_blocking(move || {
        for item in items.into_iter().take(2) {
            let Some(path) = item.get("path").and_then(|v| v.as_str()) else {
                continue;
            };
            let norm_path = path.trim().trim_start_matches('/').replace('\\', "/");
            if norm_path.is_empty() || !root.join(&norm_path).is_file() {
                continue;
            }
            let title = item
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let body = item
                .get("body")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if title.is_empty() || body.is_empty() {
                continue;
            }
            let scope_kind = item
                .get("scope_kind")
                .and_then(|v| v.as_str())
                .unwrap_or("module")
                .to_string();
            let scope_target = item
                .get("scope_target")
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| crate::commands::graph::logical_module(&norm_path));
            let kind = item
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("invariant")
                .to_string();
            let start_line = item
                .get("start_line")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);
            let end_line = item
                .get("end_line")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);
            let symbol = item
                .get("symbol")
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());

            let _ = crate::commands::graph_db::record_repo_note(
                &root,
                crate::commands::graph_db::RepoNoteInput {
                    scope_kind,
                    scope_target,
                    kind,
                    title,
                    body,
                    citations: vec![crate::commands::graph_db::NoteCitation {
                        path: norm_path,
                        start_line,
                        end_line,
                        symbol,
                        hash: String::new(),
                    }],
                    source: "agent_auto".to_string(),
                },
            );
        }
    })
    .await;
}
