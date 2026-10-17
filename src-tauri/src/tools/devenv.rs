//! `devenv` — inspect the project's development environment and settle the
//! implicit environment todo. Never installs anything: configuration is done
//! by the agent with `shell`, which already asks the user to confirm.

use async_trait::async_trait;
use piscis_core::host::EventSink;
use piscis_kernel::agent::plan::PlanStore;
use piscis_kernel::agent::tool::{Tool, ToolContext, ToolResult};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::commands::devenv;

pub struct DevEnvTool {
    legacy_plan: Option<(PlanStore, Arc<dyn EventSink>)>,
}

impl DevEnvTool {
    /// Environment inspection/dismissal without access to any task plan projection.
    pub(crate) fn environment_only() -> Self {
        Self { legacy_plan: None }
    }

    /// Compatibility for registries that still own the implicit environment todo.
    #[allow(dead_code)]
    pub(crate) fn legacy(plan_store: PlanStore, event_sink: Arc<dyn EventSink>) -> Self {
        Self {
            legacy_plan: Some((plan_store, event_sink)),
        }
    }

    pub(crate) fn updates_legacy_plan(&self) -> bool {
        self.legacy_plan.is_some()
    }

    async fn settle_legacy_todo(&self, session_id: &str) {
        if let Some((plan_store, event_sink)) = &self.legacy_plan {
            devenv::clear_todo(plan_store, event_sink, session_id).await;
        }
    }
}

#[async_trait]
impl Tool for DevEnvTool {
    fn name(&self) -> &str {
        "devenv"
    }

    fn description(&self) -> &str {
        if !self.updates_legacy_plan() {
            return "Inspect the project's development environment and report tools, versions and install hints. Actions: `check` rescans and reports tools; `resolve` rescans and reports whether requirements are satisfied; `dismiss` stops environment reminders for specified tool ids (or all current problems). Environment checks and dismissal do not automatically modify the task plan. This tool never installs software.";
        }
        "Inspect the project's development environment (detected tech stacks, required toolchain, \
         versions, missing tools with per-OS install hints).\n\
         Actions:\n\
         - `check`: rescan now and report every tool.\n\
         - `resolve`: rescan; if nothing is missing any more, clear the implicit environment todo. \
           Call this after you finished configuring the environment.\n\
         - `dismiss`: stop reminding about the given `tools` (ids) — or all current problems when \
           omitted — and clear the todo. Use it when the user does not want them.\n\
         This tool never installs software; install with `shell` (the user confirms) and then \
         call `resolve`."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["check", "resolve", "dismiss"] },
                "tools": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "For dismiss: tool ids to stop reminding about."
                }
            },
            "required": ["action"]
        })
    }

    fn is_read_only(&self) -> bool {
        false
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let action = input["action"].as_str().unwrap_or("").trim();
        let project = ctx.workspace_root.clone();
        match action {
            "check" => {
                let report = devenv::scan(&project, true).await;
                let dismissed = devenv::load_dismissed(&project);
                Ok(ToolResult::ok(devenv::render_report(&report, &dismissed)))
            }
            "resolve" => {
                let report = devenv::scan(&project, true).await;
                let dismissed = devenv::load_dismissed(&project);
                let remaining = report.problems(&dismissed);
                if remaining.is_empty() {
                    self.settle_legacy_todo(&ctx.session_id).await;
                    Ok(ToolResult::ok(if self.updates_legacy_plan() {
                        "Environment looks good; the implicit environment todo is cleared."
                    } else {
                        "Environment looks good; the task plan was not automatically modified."
                    }))
                } else {
                    let names: Vec<&str> = remaining.iter().map(|t| t.id.as_str()).collect();
                    Ok(ToolResult::err(format!(
                        "Still missing or outdated: {}.\n\n{}",
                        names.join(", "),
                        devenv::render_report(&report, &dismissed)
                    )))
                }
            }
            "dismiss" => {
                let report = devenv::scan(&project, false).await;
                let existing = devenv::load_dismissed(&project);
                let ids: Vec<String> = match input["tools"].as_array() {
                    Some(list) if !list.is_empty() => list
                        .iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect(),
                    _ => report
                        .problems(&existing)
                        .iter()
                        .map(|t| t.id.clone())
                        .collect(),
                };
                if ids.is_empty() {
                    return Ok(ToolResult::ok("Nothing to dismiss."));
                }
                devenv::dismiss(&project, &ids).map_err(|e| anyhow::anyhow!(e))?;
                self.settle_legacy_todo(&ctx.session_id).await;
                Ok(ToolResult::ok(format!(
                    "Dismissed: {}. They will not be raised again for this project.{}",
                    ids.join(", "),
                    if self.updates_legacy_plan() {
                        ""
                    } else {
                        " The task plan was not automatically modified."
                    }
                )))
            }
            other => Ok(ToolResult::err(format!(
                "Unknown action '{other}'. Use check, resolve or dismiss."
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use piscis_kernel::agent::plan::{new_plan_store, PlanTodoItem};
    use std::sync::atomic::AtomicBool;

    #[derive(Default)]
    struct Recorder(std::sync::Mutex<Vec<Value>>);
    impl EventSink for Recorder {
        fn emit_session(&self, _: &str, _: &str, payload: Value) {
            self.0.lock().unwrap().push(payload);
        }
        fn emit_broadcast(&self, _: &str, _: Value) {
            panic!("unexpected broadcast");
        }
    }
    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            assert!(self.0.starts_with(std::env::temp_dir()));
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn context(directory: &Directory) -> ToolContext {
        ToolContext {
            session_id: "task".into(),
            workspace_root: directory.0.clone(),
            bypass_permissions: false,
            settings: Arc::new(piscis_kernel::agent::tool::ToolSettings::default()),
            max_iterations: None,
            memory_owner_id: "test".into(),
            pool_session_id: None,
            tool_use_id: None,
            cancel: Arc::new(AtomicBool::new(false)),
            loop_halt: None,
        }
    }
    async fn seed_plan() -> PlanStore {
        let cache = new_plan_store();
        cache.lock().await.insert(
            "task".into(),
            vec![PlanTodoItem {
                id: devenv::DEVENV_TODO_ID.into(),
                content: "retained environment requirement".into(),
                status: "pending".into(),
            }],
        );
        cache
    }

    #[tokio::test]
    async fn environment_only_resolve_and_dismiss_preserve_retained_plan() {
        let directory = Directory(
            std::env::temp_dir().join(format!("agentz-devenv-pure-{}", uuid::Uuid::new_v4())),
        );
        std::fs::create_dir_all(&directory.0).unwrap();
        let ctx = context(&directory);
        let cache = seed_plan().await;
        let recorder = Arc::new(Recorder::default());
        // Keep a legacy observer attached to the same retained projection. The pure
        // tool is deliberately given neither handle, so it cannot publish to it.
        let legacy_observer = DevEnvTool::legacy(cache.clone(), recorder.clone());
        let before = serde_json::to_value(cache.lock().await.get("task").unwrap()).unwrap();
        let tool = DevEnvTool::environment_only();
        assert!(!tool.updates_legacy_plan());
        assert!(tool.description().contains("do not automatically modify"));
        assert!(tool.input_schema()["properties"].get("mode").is_none());
        // Empty project; dismiss scanner findings without installing software or
        // depending on tool availability on the build machine.
        let report = devenv::scan(&directory.0, true).await;
        let ids: Vec<String> = report.tools.iter().map(|entry| entry.id.clone()).collect();
        devenv::dismiss(&directory.0, &ids).unwrap();
        let resolved = tool.call(json!({"action":"resolve"}), &ctx).await.unwrap();
        assert!(!resolved.is_error);
        assert!(resolved.content.contains("not automatically modified"));
        let dismissed = tool
            .call(
                json!({"action":"dismiss","tools":["fixture-missing-tool"]}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(!dismissed.is_error);
        assert!(dismissed.content.contains("not automatically modified"));
        assert!(devenv::load_dismissed(&directory.0).contains("fixture-missing-tool"));
        assert_eq!(
            serde_json::to_value(cache.lock().await.get("task").unwrap()).unwrap(),
            before
        );
        assert!(recorder.0.lock().unwrap().is_empty());
        // Positive control: the legacy observer actually can remove this exact todo
        // and send a PlanUpdate. A disconnected/misidentified fixture would fail.
        legacy_observer.settle_legacy_todo("task").await;
        assert!(cache.lock().await.get("task").unwrap().is_empty());
        assert_eq!(recorder.0.lock().unwrap().len(), 1);
    }
}
