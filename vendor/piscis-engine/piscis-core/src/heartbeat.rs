use crate::models::{KoiTodo, PoolMessage, PoolSession};
use crate::project_state::{
    assess_project_state, contains_delegated_piscis_mention, contains_piscis_mention,
    extract_project_status_signal, ProjectAssessment, ProjectDecision,
};

#[derive(Debug, Clone)]
pub struct PoolAttention {
    pub pool_id: String,
    pub pool_name: String,
    pub latest_message_id: i64,
    pub session_id: String,
    pub summary: String,
    pub assessment: ProjectAssessment,
    /// Truncated org_spec text for convergence checks (data injection, not flow rules).
    pub org_spec_excerpt: Option<String>,
}

/// True when the host assessment says Piscis should coordinate, not merely ack.
pub fn assessment_requires_coordination(assessment: &ProjectAssessment) -> bool {
    !matches!(assessment.decision, ProjectDecision::Continue)
        || assessment.needs_review_count > 0
        || assessment.blocked_todo_count > 0
}

/// Detect heartbeat turns that ended without observable pool coordination.
pub fn is_heartbeat_ack_only(response: &str) -> bool {
    let trimmed = response.trim();
    if trimmed.is_empty() {
        return true;
    }
    let lower = trimmed.to_lowercase();
    let short = trimmed.chars().count() <= 240;
    short
        && (lower.contains("heartbeat_ok")
            || lower.contains("无需干预")
            || lower.contains("no intervention")
            || lower.contains("no change"))
}

/// State-derived supervisor note when a heartbeat turn did not touch the pool.
pub fn build_heartbeat_coordination_gap_notice(attention: &PoolAttention) -> String {
    let reasons = if attention.assessment.attention_reasons.is_empty() {
        attention.assessment.summary.clone()
    } else {
        attention.assessment.attention_reasons.join("; ")
    };
    format!(
        "[Heartbeat patrol · state trigger]\n\
         Decision: {:?}\n\
         Assessment: {}\n\
         Attention: {}\n\
         This note was posted because the heartbeat turn did not produce any new Piscis pool activity. \
         Compare org_spec against board evidence and coordinate via pool_org (post_status / create_todo / assign_koi) as appropriate.",
        attention.assessment.decision,
        attention.assessment.summary,
        reasons,
    )
}

fn preview_chars(content: &str, max_chars: usize) -> String {
    if content.chars().count() <= max_chars {
        return content.to_string();
    }
    format!("{}...", content.chars().take(max_chars).collect::<String>())
}

fn is_attention_event(msg: &PoolMessage, koi_ids: &[String]) -> bool {
    if msg.sender_id == "piscis" {
        return false;
    }
    let from_known_koi = koi_ids.iter().any(|id| id == &msg.sender_id);
    if contains_piscis_mention(&msg.content) {
        return true;
    }
    if from_known_koi && extract_project_status_signal(&msg.content).is_some() {
        return true;
    }
    matches!(
        msg.event_type.as_deref(),
        Some(
            "task_completed"
                | "task_failed"
                | "task_claimed"
                | "task_blocked"
                | "task_cancelled"
                | "protocol_reminder"
                | "task_progress"
        )
    )
}

pub fn pool_piscis_session_id(pool_id: &str) -> String {
    format!("piscis_pool_{}", pool_id)
}

pub fn build_pool_heartbeat_message(base_prompt: &str, attention: &PoolAttention) -> String {
    let assessment = &attention.assessment;
    let mut lines = vec![
        base_prompt.to_string(),
        String::new(),
        "## Heartbeat Inbox".to_string(),
        attention.summary.clone(),
        String::new(),
        "## Current Project State".to_string(),
        "- The fields below are a host-generated snapshot of observable state, not a final project judgment.".to_string(),
        format!("- Decision: {:?}", assessment.decision),
        format!("- Active todos: {}", assessment.active_todo_count),
        format!("- Blocked todos: {}", assessment.blocked_todo_count),
        format!("- Needs-review todos: {}", assessment.needs_review_count),
        format!("- Integration-ready branches: {}", assessment.integration_ready_count),
        format!("- Dependency-blocked todos: {}", assessment.dependency_blocked_count),
        format!("- task_failed events: {}", assessment.task_failed_count),
        format!("- Follow-up signals: {}", assessment.follow_up_signal_count),
        format!("- Assessment: {}", assessment.summary),
        String::new(),
        "## Attention Reasons".to_string(),
    ];
    if assessment.attention_reasons.is_empty() {
        lines.push("- No explicit attention reason was derived; inspect the latest pool state before acting.".to_string());
    } else {
        lines.extend(
            assessment
                .attention_reasons
                .iter()
                .map(|reason| format!("- {}", reason)),
        );
    }
    if let Some(org_spec) = attention.org_spec_excerpt.as_deref() {
        lines.extend([
            String::new(),
            "## Project org_spec (convergence reference)".to_string(),
            "- Compare ALL phases/milestones/deliverables in this text against pool_org evidence — not just whether todos are done.".to_string(),
            org_spec.to_string(),
        ]);
    }
    lines.extend([String::new(), "## Guidance".to_string()]);

    match assessment.decision {
        ProjectDecision::Continue => {
            lines.push(
                "The project snapshot still shows active coordination pressure. Inspect pool_org(get_messages), the task board, and the org_spec before deciding whether Piscis should act now or continue waiting."
                    .to_string(),
            );
            if assessment.active_todo_count == 0
                && (assessment.follow_up_signal_count > 0 || assessment.task_failed_count > 0)
            {
                lines.push(
                    "There is coordination pressure without a clear active owner. Treat this as an attention gap to investigate, not as permission to pick a specific next actor automatically."
                        .to_string(),
                );
            }
            lines.push(
                "Do not assume a fixed reviewer or handoff target. Use the current pool evidence and org_spec to decide the smallest effective coordination action."
                    .to_string(),
            );
        }
        ProjectDecision::SupervisorDecisionRequired => {
            lines.push(
                "Workers look locally finished; the task board alone is NOT proof the project converged under org_spec."
                    .to_string(),
            );
            lines.push(
                "Mandatory before HEARTBEAT_OK: pool_org(action=\"read\") for this pool, then get_todos + get_messages. Compare all org_spec requirements (phases, milestones, roles, deliverables — whatever the spec text says) against board evidence."
                    .to_string(),
            );
            lines.push(
                "If org_spec still has uncovered work (e.g. early milestones done but later ones never decomposed into todos): pool_org(post_status) stating the gap, then create_todo / assign_koi for the next concrete steps. Do NOT reply HEARTBEAT_OK while the spec is unfinished."
                    .to_string(),
            );
            lines.push(
                "If you decide the user should weigh in before the project moves forward, call `app_control` with action='notify_user' (level='warning' or 'info', include pool_id) to surface a toast in the main UI. Do this only when user input genuinely helps, not as a routine status update."
                    .to_string(),
            );
        }
        ProjectDecision::EscalateToHuman => {
            lines.push(
                "The project reached a state that should NOT be resolved by further autonomous retries or automatic routing."
                    .to_string(),
            );
            lines.push(
                "Treat this as a human-escalation state: inspect the failure evidence, summarize what became impossible or unsafe to decide automatically, and stop short of inventing a new worker plan unless the user explicitly approves it."
                    .to_string(),
            );
            lines.push(
                "Your role here is to surface the situation clearly for the user, not to silently convert an unrecoverable failure into a normal project continuation."
                    .to_string(),
            );
            lines.push(
                "Raise a user-visible toast via `app_control` with action='notify_user', level='critical', duration_ms=0, and include pool_id plus a 1-2 sentence summary of why human judgment is needed. The system may have auto-posted a baseline toast already; your call adds the diagnostic summary."
                    .to_string(),
            );
        }
        ProjectDecision::ReadyForPiscisReview => {
            lines.push(
                "The snapshot is compatible with Piscis review, but HEARTBEAT_OK is forbidden as the only action when needs_review todos exist."
                    .to_string(),
            );
            if assessment.integration_ready_count > 0 {
                lines.push(format!(
                    "{} branch(es) are done on the board but not merged. Prefer pool_org(merge_branches, branch=<one branch>) incrementally after reviewing get_messages/get_todos — do not wait for a final batch merge of every koi/* branch.",
                    assessment.integration_ready_count
                ));
            }
            lines.push(
                "Mandatory closeout: call pool_org(action=\"get_messages\") and pool_org(action=\"get_todos\") for this pool, inspect the reported deliverables, then choose exactly one outcome: merge_branches if the work is acceptable, resume_todo/replace_todo/assign_koi if rework is needed, or post_status plus notify_user when human review is required."
                    .to_string(),
            );
            lines.push(
                "Do not say \"no change\", \"无需干预\", or HEARTBEAT_OK until you have taken a concrete closeout action or written a clear pool_org(post_status) explanation of why no autonomous action is safe."
                    .to_string(),
            );
            lines.push(
                "During heartbeat, do NOT archive the project automatically. Leave the pool active until the user explicitly asks to archive it."
                    .to_string(),
            );
            lines.push("If you discover unresolved work, keep the project open and coordinate the next step explicitly.".to_string());
        }
    }

    lines.push(String::new());
    lines.push(
        "Do not treat a quiet board as success. Read org_spec via pool_org(read), then act or document why HEARTBEAT_OK is justified."
            .to_string(),
    );
    lines.join("\n")
}

pub fn collect_pool_attention(
    pool: &PoolSession,
    messages: &[PoolMessage],
    todos: &[KoiTodo],
    koi_ids: &[String],
    last_seen_message_id: i64,
) -> Option<PoolAttention> {
    let latest_message_id = messages
        .last()
        .map(|m| m.id)
        .unwrap_or(last_seen_message_id);
    let new_attention_messages: Vec<&PoolMessage> = messages
        .iter()
        .filter(|m| m.id > last_seen_message_id && is_attention_event(m, koi_ids))
        .collect();

    let assessment = assess_project_state(messages, todos, koi_ids);
    let has_historic_piscis_route = messages
        .iter()
        .any(|msg| contains_piscis_mention(&msg.content));

    let has_state_attention = !assessment.attention_reasons.is_empty();
    if new_attention_messages.is_empty()
        && assessment.decision == ProjectDecision::EscalateToHuman
        && has_historic_piscis_route
    {
        return None;
    }
    // Do not suppress supervisor attention after a prior @piscis mention: the board
    // may be quiet while org_spec is still unfinished (e.g. only early milestones done).
    if new_attention_messages.is_empty()
        && assessment.decision == ProjectDecision::Continue
        && !has_state_attention
    {
        return None;
    }
    let mut lines = vec![
        format!("Pool: {} ({})", pool.name, pool.id),
        format!("Status: {}", pool.status),
        format!("Recent attention events: {}", new_attention_messages.len()),
        format!("Assessment: {}", assessment.summary),
    ];
    if let Some(project_dir) = pool.project_dir.as_deref() {
        lines.push(format!("Project dir: {}", project_dir));
    }
    lines.push("Recent pool events:".to_string());
    for msg in new_attention_messages.iter().rev().take(6).rev() {
        let event = msg.event_type.as_deref().unwrap_or(&msg.msg_type);
        lines.push(format!(
            "- #{} [{}] {}: {}",
            msg.id,
            event,
            msg.sender_id,
            preview_chars(&msg.content.replace('\n', " "), 240)
        ));
    }

    let org_spec_excerpt = {
        let trimmed = pool.org_spec.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(preview_chars(trimmed, 4000))
        }
    };

    Some(PoolAttention {
        pool_id: pool.id.clone(),
        pool_name: pool.name.clone(),
        latest_message_id,
        session_id: pool_piscis_session_id(&pool.id),
        summary: lines.join("\n"),
        assessment,
        org_spec_excerpt,
    })
}

/// Build attention for an explicit `@!Piscis` mention without relying on the
/// periodic heartbeat cursor. Uses a `last_seen` one message before the
/// triggering mention so the mention is always included in the inbox summary.
pub fn build_forced_mention_attention(
    pool: &PoolSession,
    messages: &[PoolMessage],
    todos: &[KoiTodo],
    koi_ids: &[String],
) -> Option<PoolAttention> {
    let trigger = messages
        .iter()
        .rev()
        .find(|msg| contains_delegated_piscis_mention(&msg.content))?;
    collect_pool_attention(pool, messages, todos, koi_ids, trigger.id.saturating_sub(1))
}
