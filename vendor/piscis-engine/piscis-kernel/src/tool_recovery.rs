//! Versioned observations for tool/process recovery, not execution permissions.
//! Reports never authorize retries or infer cleanup from a plain tool error.
use serde::{Deserialize, Serialize};

pub const REPORT_VERSION: u32 = 1;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Completed,
    ToolFailed,
    TimedOut,
    Cancelled,
    ElevationNoResponse,
    ElevationDenied,
    ElevationUnsupported,
    SetupFailed,
    WorkerFailed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Termination {
    Unknown,
    NotStarted,
    Exited,
    TreeTerminated,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Evidence {
    Unknown,
    Confirmed,
    Failed,
    NotApplicable,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cleanup {
    pub stdout_drained: Evidence,
    pub stderr_drained: Evidence,
    pub access_restored: Evidence,
}
impl Cleanup {
    pub fn unknown() -> Self {
        Self {
            stdout_drained: Evidence::Unknown,
            stderr_drained: Evidence::Unknown,
            access_restored: Evidence::Unknown,
        }
    }
    fn confirmed(&self) -> bool {
        [
            self.stdout_drained,
            self.stderr_drained,
            self.access_restored,
        ]
        .iter()
        .all(|s| matches!(s, Evidence::Confirmed | Evidence::NotApplicable))
    }
}
/// Truncation is orthogonal to exit/timeout/cancellation; never overwrites them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum OutputCapture {
    Unknown,
    Complete {
        captured_bytes: u64,
    },
    Truncated {
        captured_bytes: u64,
        total_bytes: u64,
    },
}
impl OutputCapture {
    fn validate(&self) -> Result<(), String> {
        if let Self::Truncated {
            captured_bytes,
            total_bytes,
        } = self
        {
            if captured_bytes >= total_bytes {
                return Err("invalid output truncation observation".into());
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomaticRetry {
    Forbidden,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NextStep {
    NotRequired,
    ReviewOutput,
    ManualReconcile,
}
/// Trusted runner observations only. No command, arguments, raw errors, output,
/// secret or arbitrary path is included; operation identity is host supplied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryReport {
    pub version: u32,
    pub operation_id: String,
    pub outcome: Outcome,
    pub exit_code: Option<i32>,
    pub termination: Termination,
    pub cleanup: Cleanup,
    pub stdout: OutputCapture,
    pub stderr: OutputCapture,
    /// Presentation trimming is separate from raw pipe capture byte counts.
    #[serde(default)]
    pub stdout_presentation_truncated: bool,
    #[serde(default)]
    pub stderr_presentation_truncated: bool,
    pub automatic_retry: AutomaticRetry,
}
impl RecoveryReport {
    pub fn note_presentation_truncation(&mut self, stdout: bool, stderr: bool) {
        // Monotonic: a later formatter cannot undo an earlier omission.
        self.stdout_presentation_truncated |= stdout;
        self.stderr_presentation_truncated |= stderr;
    }
    /// Use only after a runner confirms tree termination, both pipe joins and
    /// access restoration. Counts are raw bytes, never lossy UTF-8 string length.
    #[allow(clippy::too_many_arguments)]
    pub fn confirmed_result(
        operation_id: String,
        exit_code: i32,
        timed_out: bool,
        cancelled: bool,
        stdout_total_bytes: u64,
        stdout_retained_bytes: u64,
        stderr_total_bytes: u64,
        stderr_retained_bytes: u64,
    ) -> Result<Self, String> {
        if timed_out && cancelled {
            return Err("contradictory process timeout/cancellation observation".into());
        }
        fn capture(total: u64, retained: u64) -> Result<OutputCapture, String> {
            if retained > total {
                return Err("retained output exceeds observed raw bytes".into());
            }
            Ok(if retained == total {
                OutputCapture::Complete {
                    captured_bytes: retained,
                }
            } else {
                OutputCapture::Truncated {
                    captured_bytes: retained,
                    total_bytes: total,
                }
            })
        }
        let outcome = if timed_out {
            Outcome::TimedOut
        } else if cancelled {
            Outcome::Cancelled
        } else if exit_code == 0 {
            Outcome::Completed
        } else {
            Outcome::ToolFailed
        };
        Self::from_observation(
            operation_id,
            outcome,
            Some(exit_code),
            Termination::TreeTerminated,
            Cleanup {
                stdout_drained: Evidence::Confirmed,
                stderr_drained: Evidence::Confirmed,
                access_restored: Evidence::Confirmed,
            },
            capture(stdout_total_bytes, stdout_retained_bytes)?,
            capture(stderr_total_bytes, stderr_retained_bytes)?,
        )
    }
    pub fn unknown_failure(operation_id: String) -> Result<Self, String> {
        Self::unclassified_error(operation_id)
    }
    /// Preflight refusal before ANY process/resource setup; not a UAC response.
    pub fn refused_before_launch(operation_id: String) -> Result<Self, String> {
        Self::refused(operation_id, Outcome::SetupFailed)
    }
    pub fn refused_elevation_before_launch(operation_id: String) -> Result<Self, String> {
        Self::refused(operation_id, Outcome::ElevationUnsupported)
    }
    fn refused(operation_id: String, outcome: Outcome) -> Result<Self, String> {
        Self::from_observation(
            operation_id,
            outcome,
            None,
            Termination::NotStarted,
            Cleanup {
                stdout_drained: Evidence::NotApplicable,
                stderr_drained: Evidence::NotApplicable,
                access_restored: Evidence::NotApplicable,
            },
            OutputCapture::Complete { captured_bytes: 0 },
            OutputCapture::Complete { captured_bytes: 0 },
        )
    }
    /// Passive metadata footer in the existing tool-content/event channel.
    /// Never parse this text as execution authority or an automatic retry permit.
    pub fn attach(
        self,
        mut result: crate::agent::tool::ToolResult,
    ) -> Result<crate::agent::tool::ToolResult, String> {
        self.validate()?;
        let json = serde_json::to_string(&self).map_err(|e| e.to_string())?;
        result.content.push_str("\n\n[recovery_v1]\n");
        result.content.push_str(&json);
        result.content.push_str("\n[/recovery_v1]");
        if self.outcome != Outcome::Completed {
            result.is_error = true;
        }
        Ok(result)
    }
    pub fn from_observation(
        operation_id: String,
        outcome: Outcome,
        exit_code: Option<i32>,
        termination: Termination,
        cleanup: Cleanup,
        stdout: OutputCapture,
        stderr: OutputCapture,
    ) -> Result<Self, String> {
        let report = Self {
            version: REPORT_VERSION,
            operation_id,
            outcome,
            exit_code,
            termination,
            cleanup,
            stdout,
            stderr,
            stdout_presentation_truncated: false,
            stderr_presentation_truncated: false,
            automatic_retry: AutomaticRetry::Forbidden,
        };
        report.validate()?;
        Ok(report)
    }
    /// A plain Err has no process or cleanup proof. Do not parse error strings
    /// for UAC/timeout classification or claim a child never launched.
    pub fn unclassified_error(operation_id: String) -> Result<Self, String> {
        Self::from_observation(
            operation_id,
            Outcome::WorkerFailed,
            None,
            Termination::Unknown,
            Cleanup::unknown(),
            OutputCapture::Unknown,
            OutputCapture::Unknown,
        )
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != REPORT_VERSION {
            return Err("unsupported tool recovery report version".into());
        }
        if self.operation_id.trim().is_empty()
            || self.operation_id.trim() != self.operation_id
            || self.operation_id.len() > 256
            || self.operation_id.chars().any(char::is_control)
        {
            return Err("invalid tool recovery operation identity".into());
        }
        self.stdout.validate()?;
        self.stderr.validate()?;
        match self.outcome {
            Outcome::Completed if self.exit_code != Some(0) => {
                return Err("completed process requires zero exit code".into())
            }
            Outcome::ToolFailed if self.exit_code.is_none() || self.exit_code == Some(0) => {
                return Err("tool failure requires a nonzero exit code".into())
            }
            Outcome::Completed | Outcome::ToolFailed
                if matches!(
                    self.termination,
                    Termination::NotStarted | Termination::Unknown
                ) =>
            {
                return Err("exit outcome requires observed process termination".into())
            }
            _ => {}
        }
        if self.termination == Termination::NotStarted {
            if self.exit_code.is_some()
                || !matches!(
                    self.stdout,
                    OutputCapture::Unknown | OutputCapture::Complete { captured_bytes: 0 }
                )
                || !matches!(
                    self.stderr,
                    OutputCapture::Unknown | OutputCapture::Complete { captured_bytes: 0 }
                )
            {
                return Err("unstarted process cannot have exit or captured output".into());
            }
        }
        Ok(())
    }
    /// Exited means only the root exited; it is not a process-tree proof.
    pub fn resources_confirmed(&self) -> Result<bool, String> {
        self.validate()?;
        Ok(matches!(
            self.termination,
            Termination::NotStarted | Termination::TreeTerminated
        ) && self.cleanup.confirmed())
    }
    pub fn next_step(&self) -> Result<NextStep, String> {
        if !self.resources_confirmed()? || self.outcome != Outcome::Completed {
            return Ok(NextStep::ManualReconcile);
        }
        if self.stdout_presentation_truncated
            || self.stderr_presentation_truncated
            || !matches!(self.stdout, OutputCapture::Complete { .. })
            || !matches!(self.stderr, OutputCapture::Complete { .. })
        {
            return Ok(NextStep::ReviewOutput);
        }
        Ok(NextStep::NotRequired)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn clean() -> Cleanup {
        Cleanup {
            stdout_drained: Evidence::Confirmed,
            stderr_drained: Evidence::Confirmed,
            access_restored: Evidence::Confirmed,
        }
    }
    fn report(outcome: Outcome, exit: Option<i32>) -> RecoveryReport {
        RecoveryReport::from_observation(
            "host-operation".into(),
            outcome,
            exit,
            Termination::TreeTerminated,
            clean(),
            OutputCapture::Complete { captured_bytes: 5 },
            OutputCapture::Complete { captured_bytes: 0 },
        )
        .unwrap()
    }
    #[test]
    fn clean_success_nonzero_timeout_and_cancel_are_distinct_without_retry() {
        for (outcome, exit, next) in [
            (Outcome::Completed, Some(0), NextStep::NotRequired),
            (Outcome::ToolFailed, Some(2), NextStep::ManualReconcile),
            (Outcome::TimedOut, Some(124), NextStep::ManualReconcile),
            (Outcome::Cancelled, Some(130), NextStep::ManualReconcile),
        ] {
            let r = report(outcome, exit);
            assert_eq!(r.next_step().unwrap(), next);
            assert!(r.resources_confirmed().unwrap());
            assert_eq!(r.automatic_retry, AutomaticRetry::Forbidden);
            assert_eq!(
                serde_json::from_str::<RecoveryReport>(&serde_json::to_string(&r).unwrap())
                    .unwrap(),
                r
            );
        }
    }
    #[test]
    fn ordinary_error_root_exit_and_failed_cleanup_never_confirm_resources() {
        let r = RecoveryReport::unclassified_error("op".into()).unwrap();
        assert!(!r.resources_confirmed().unwrap());
        assert_eq!(r.next_step().unwrap(), NextStep::ManualReconcile);
        let mut r = report(Outcome::Completed, Some(0));
        r.termination = Termination::Exited;
        assert!(!r.resources_confirmed().unwrap());
        r.termination = Termination::TreeTerminated;
        r.cleanup.access_restored = Evidence::Failed;
        assert!(!r.resources_confirmed().unwrap());
    }
    #[test]
    fn elevation_no_response_and_denial_do_not_infer_launch_or_cleanup() {
        for outcome in [Outcome::ElevationNoResponse, Outcome::ElevationDenied] {
            let r = RecoveryReport::from_observation(
                "uac".into(),
                outcome,
                None,
                Termination::Unknown,
                Cleanup::unknown(),
                OutputCapture::Unknown,
                OutputCapture::Unknown,
            )
            .unwrap();
            assert!(!r.resources_confirmed().unwrap());
            assert_eq!(r.next_step().unwrap(), NextStep::ManualReconcile);
        }
    }
    #[test]
    fn truncation_keeps_primary_outcome_and_requires_review_not_rerun() {
        let mut r = report(Outcome::Completed, Some(0));
        r.stdout = OutputCapture::Truncated {
            captured_bytes: 8,
            total_bytes: 12,
        };
        assert_eq!(r.next_step().unwrap(), NextStep::ReviewOutput);
        r.outcome = Outcome::TimedOut;
        r.exit_code = Some(124);
        assert_eq!(r.next_step().unwrap(), NextStep::ManualReconcile);
        assert_eq!(r.outcome, Outcome::TimedOut);
        r.stdout = OutputCapture::Truncated {
            captured_bytes: 9,
            total_bytes: 8,
        };
        assert!(r.validate().is_err());
    }
    #[test]
    fn bad_version_identity_contradictory_exit_and_unknown_retry_are_rejected() {
        let mut r = report(Outcome::Completed, Some(0));
        r.version = 2;
        assert!(r.validate().is_err());
        r.version = 1;
        r.exit_code = Some(1);
        assert!(r.validate().is_err());
        r.exit_code = Some(0);
        r.termination = Termination::NotStarted;
        assert!(r.validate().is_err());
        assert!(RecoveryReport::unclassified_error(" op ".into()).is_err());
        let mut v = serde_json::to_value(report(Outcome::Completed, Some(0))).unwrap();
        v["automatic_retry"] = serde_json::json!("allowed");
        assert!(serde_json::from_value::<RecoveryReport>(v).is_err());
    }
    #[test]
    fn runner_constructors_preserve_counts_and_footer_without_automatic_retry() {
        let r =
            RecoveryReport::confirmed_result("op".into(), 0, false, false, 20, 8, 0, 0).unwrap();
        assert_eq!(
            r.stdout,
            OutputCapture::Truncated {
                captured_bytes: 8,
                total_bytes: 20
            }
        );
        let output = r
            .attach(crate::agent::tool::ToolResult::ok("existing output"))
            .unwrap();
        assert!(output
            .content
            .starts_with("existing output\n\n[recovery_v1]\n"));
        let json = output
            .content
            .split("[recovery_v1]\n")
            .nth(1)
            .unwrap()
            .split("\n[/recovery_v1]")
            .next()
            .unwrap();
        let saved: RecoveryReport = serde_json::from_str(json).unwrap();
        assert_eq!(saved.next_step().unwrap(), NextStep::ReviewOutput);
        assert!(!output.is_error);
        assert!(RecoveryReport::confirmed_result("op".into(), 0, true, true, 0, 0, 0, 0).is_err());
        assert!(
            RecoveryReport::confirmed_result("op".into(), 0, false, false, 2, 3, 0, 0).is_err()
        );
    }
    #[test]
    fn unsupported_elevation_is_not_reported_as_uac_denial_and_plain_error_stays_unknown() {
        let r = RecoveryReport::refused_elevation_before_launch("op".into()).unwrap();
        assert_eq!(r.outcome, Outcome::ElevationUnsupported);
        assert_eq!(r.termination, Termination::NotStarted);
        assert_eq!(r.next_step().unwrap(), NextStep::ManualReconcile);
        let r = RecoveryReport::refused_before_launch("op".into()).unwrap();
        assert!(r.resources_confirmed().unwrap());
        assert_eq!(r.outcome, Outcome::SetupFailed);
        let r = RecoveryReport::unknown_failure("op".into()).unwrap();
        assert!(!r.resources_confirmed().unwrap());
        let output = r
            .attach(crate::agent::tool::ToolResult::err("existing error"))
            .unwrap();
        assert!(output.is_error);
        assert!(output.content.contains("\"termination\":\"unknown\""));
    }
    #[test]
    fn presentation_omission_is_monotonic_without_rewriting_raw_counts() {
        let mut r =
            RecoveryReport::confirmed_result("op".into(), 0, false, false, 8, 8, 2, 2).unwrap();
        r.note_presentation_truncation(true, false);
        r.note_presentation_truncation(false, false);
        assert_eq!(r.stdout, OutputCapture::Complete { captured_bytes: 8 });
        assert_eq!(r.stderr, OutputCapture::Complete { captured_bytes: 2 });
        assert_eq!(r.next_step().unwrap(), NextStep::ReviewOutput);
        let mut v = serde_json::to_value(&r).unwrap();
        v.as_object_mut()
            .unwrap()
            .remove("stdout_presentation_truncated");
        v.as_object_mut()
            .unwrap()
            .remove("stderr_presentation_truncated");
        let old: RecoveryReport = serde_json::from_value(v).unwrap();
        assert!(!old.stdout_presentation_truncated);
        assert!(!old.stderr_presentation_truncated);
    }
}
