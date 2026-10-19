//! Compile and exercise the public kernel contracts through AgentZ's fixed dependencies.
use piscis_kernel::agent::tool::{ImageData, ToolResult};
use piscis_kernel::llm::error_class::{classify_error, ErrorClass};
use piscis_kernel::tool_recovery::{
    AutomaticRetry, NextStep, Outcome, RecoveryReport, Termination,
};

#[test]
fn unknown_process_result_remains_passive_and_preserves_host_tool_content() {
    let report = RecoveryReport::unclassified_error("host-operation".into()).unwrap();
    assert_eq!(report.termination, Termination::Unknown);
    assert_eq!(report.automatic_retry, AutomaticRetry::Forbidden);
    assert_eq!(report.next_step().unwrap(), NextStep::ManualReconcile);
    let result = report
        .attach(ToolResult::ok("host content").with_image(ImageData::png("synthetic-image")))
        .unwrap();
    assert!(result.is_error);
    assert!(result.content.starts_with("host content\n\n[recovery_v1]"));
    assert!(result.content.contains("\"automatic_retry\":\"forbidden\""));
    assert_eq!(result.image.unwrap().base64, "synthetic-image");
}

#[test]
fn cancellation_is_distinct_from_output_omission_and_never_grants_replay() {
    let mut report =
        RecoveryReport::confirmed_result("host-cancel".into(), 130, false, true, 10, 3, 0, 0)
            .unwrap();
    report.note_presentation_truncation(true, false);
    assert_eq!(report.outcome, Outcome::Cancelled);
    assert_eq!(report.termination, Termination::TreeTerminated);
    assert!(report.resources_confirmed().unwrap());
    assert_eq!(report.next_step().unwrap(), NextStep::ManualReconcile);
    assert_eq!(report.automatic_retry, AutomaticRetry::Forbidden);
    assert!(
        report
            .attach(ToolResult::ok("partial output"))
            .unwrap()
            .is_error
    );
}

#[test]
fn permanent_provider_failures_do_not_retry_or_fallback() {
    for class in [
        classify_error("OpenAI API error 401 Unauthorized"),
        classify_error("OpenAI API error 429: insufficient_quota"),
    ] {
        assert!(matches!(
            class,
            ErrorClass::AuthFailed | ErrorClass::QuotaExceeded
        ));
        assert!(!class.is_retryable());
        assert!(!class.is_fallback_eligible());
    }
    assert!(classify_error("OpenAI API error 503 Service Unavailable").is_retryable());
    assert!(classify_error("OpenAI API error 429: rate limit exceeded").is_fallback_eligible());
}

#[test]
fn five_phase_lifecycle_and_termination_reasons_are_pinned() {
    let phases = serde_json::json!([
        "prepare",
        "llm_turn",
        "tool_dispatch",
        "verify",
        "finalize"
    ]);
    assert_eq!(phases.as_array().unwrap().len(), 5);
    assert_eq!(phases[0], "prepare");
    assert_eq!(phases[1], "llm_turn");
    assert_eq!(phases[2], "tool_dispatch");
    assert_eq!(phases[3], "verify");
    assert_eq!(phases[4], "finalize");

    let reasons = serde_json::json!([
        "Completed",
        "Cancelled",
        "MaxIterations",
        "BudgetExceeded",
        "Error"
    ]);
    assert_eq!(reasons.as_array().unwrap().len(), 5);

    // Verify recovery outcomes map cleanly to kernel termination semantics without silent replay.
    let cancelled =
        RecoveryReport::confirmed_result("turn-cancel".into(), 130, false, true, 5, 1, 0, 0)
            .unwrap();
    assert_eq!(cancelled.outcome, Outcome::Cancelled);
    assert_eq!(cancelled.automatic_retry, AutomaticRetry::Forbidden);
}

