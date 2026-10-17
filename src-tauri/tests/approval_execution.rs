//! Actual pinned kernel lifecycle hooks + SQLite, without a model or native window.
use agentz_desktop_lib::{journal::open_project_journal, runtime::approval::ApprovalHooks};
use agentz_task_state::{
    approval::{ApprovalStore, Status},
    approval_broker::ApprovalBroker,
};
use async_trait::async_trait;
use piscis_core::host::EventSink;
use piscis_kernel::agent::{
    hooks::{AgentHooks, HookDecision, ToolHookEvent},
    tool::{Tool, ToolContext, ToolRegistry, ToolResult},
};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::mpsc;
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}
struct Sink(mpsc::UnboundedSender<Value>);
impl EventSink for Sink {
    fn emit_session(&self, _: &str, event: &str, payload: Value) {
        if event == "agent_event" {
            let _ = self.0.send(payload);
        }
    }
    fn emit_broadcast(&self, _: &str, _: Value) {}
}
struct Stub(&'static str);
struct ReadOnlyStub(&'static str, bool);
#[async_trait]
impl Tool for ReadOnlyStub {
    fn name(&self) -> &str {
        self.0
    }
    fn description(&self) -> &str {
        "fixture"
    }
    fn input_schema(&self) -> Value {
        json!({})
    }
    fn is_read_only(&self) -> bool {
        true
    }
    fn needs_confirmation(&self, _: &Value) -> bool {
        self.1
    }
    async fn call(&self, _: Value, _: &ToolContext) -> anyhow::Result<ToolResult> {
        Ok(ToolResult::ok("fixture"))
    }
}
struct ParamRead(Arc<AtomicUsize>);
#[async_trait]
impl Tool for ParamRead {
    fn name(&self) -> &str {
        "file_read"
    }
    fn description(&self) -> &str {
        "fixture"
    }
    fn input_schema(&self) -> Value {
        json!({})
    }
    fn is_read_only(&self) -> bool {
        true
    }
    fn needs_confirmation(&self, input: &Value) -> bool {
        input["dangerous"].as_bool().unwrap_or(false)
    }
    async fn call(&self, _: Value, _: &ToolContext) -> anyhow::Result<ToolResult> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(ToolResult::ok("read"))
    }
}
#[tokio::test]
async fn background_actual_parameters_and_file_scope_are_checked_before_provider_call() {
    let f = Fixture::new(false).await;
    std::fs::create_dir(f.root.join("src")).unwrap();
    std::fs::write(f.root.join(".env.local"), "fixture-secret-marker").unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(ParamRead(calls.clone())));
    registry.register(Box::new(ReadOnlyStub("file_search", false)));
    registry.register(Box::new(ReadOnlyStub("file_list", false)));
    agentz_desktop_lib::runtime::approval::restrict_unapproved_background(&mut registry);
    let ctx = ToolContext {
        session_id: "task-a".into(),
        workspace_root: f.root.clone(),
        bypass_permissions: true,
        settings: Arc::new(Default::default()),
        max_iterations: None,
        memory_owner_id: "fixture".into(),
        pool_session_id: None,
        tool_use_id: None,
        cancel: f.cancel.clone(),
        loop_halt: None,
    };
    let tool = registry.get("file_read").unwrap();
    for name in ["file_search", "file_list"] {
        let tool = registry.get(name).unwrap();
        assert!(
            tool.call(json!({"path":"../outside"}), &ctx)
                .await
                .unwrap()
                .is_error
        );
        assert!(
            tool.call(json!({"path":".agentz"}), &ctx)
                .await
                .unwrap()
                .is_error
        );
        assert!(
            tool.call(json!({"path":".", "include_hidden":true}), &ctx)
                .await
                .unwrap()
                .is_error
        );
        assert!(
            !tool
                .call(json!({"path":"src"}), &ctx)
                .await
                .unwrap()
                .is_error
        );
    }
    for input in [
        json!({"path":"source.txt","dangerous":true}),
        json!({"path":".agentz/approvals.db"}),
        json!({"path":".env"}),
        json!({"path":"../outside"}),
    ] {
        assert!(tool.call(input, &ctx).await.unwrap().is_error);
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(
        !tool
            .call(json!({"path":"source.txt"}), &ctx)
            .await
            .unwrap()
            .is_error
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let dependency =
        std::env::temp_dir().join(format!("agentz-read-dependency-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dependency).unwrap();
    std::fs::write(dependency.join("lib.rs"), "fixture").unwrap();
    let dependency_file = dependency.join("lib.rs").to_string_lossy().into_owned();
    assert!(
        tool.call(json!({"path":dependency_file}), &ctx)
            .await
            .unwrap()
            .is_error
    );
    let mut authorized = ToolRegistry::new();
    authorized.register(Box::new(ParamRead(calls.clone())));
    agentz_desktop_lib::runtime::approval::restrict_background_with_read_roots(
        &mut authorized,
        vec![dependency.clone()],
    );
    let tool = authorized.get("file_read").unwrap();
    assert!(
        !tool
            .call(json!({"path":dependency_file}), &ctx)
            .await
            .unwrap()
            .is_error
    );
    assert!(
        tool.call(
            json!({"path":dependency.join(".env").to_string_lossy()}),
            &ctx
        )
        .await
        .unwrap()
        .is_error
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}
#[test]
fn unapproved_headless_registry_removes_writes_and_capabilities_even_if_marked_read_only() {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(Stub("file_write")));
    for name in [
        "file_read",
        "shell",
        "powershell_query",
        "mcp_test",
        "api_connector",
        "delegate",
        "pool_chat",
        "read_lints",
        "lsp",
    ] {
        registry.register(Box::new(ReadOnlyStub(name, false)));
    }
    registry.register(Box::new(ReadOnlyStub("requires_confirmation", true)));
    agentz_desktop_lib::runtime::approval::restrict_unapproved_background(&mut registry);
    assert_eq!(
        registry
            .all()
            .iter()
            .map(|tool| tool.name())
            .collect::<Vec<_>>(),
        vec!["file_read"]
    );
}
#[async_trait]
impl Tool for Stub {
    fn name(&self) -> &str {
        self.0
    }
    fn description(&self) -> &str {
        "fixture"
    }
    fn input_schema(&self) -> Value {
        json!({})
    }
    async fn call(&self, _: Value, _: &ToolContext) -> anyhow::Result<ToolResult> {
        Ok(ToolResult::ok("fixture"))
    }
}
struct Fixture {
    root: PathBuf,
    broker: ApprovalBroker,
    hooks: Arc<ApprovalHooks>,
    rx: mpsc::UnboundedReceiver<Value>,
    cancel: Arc<AtomicBool>,
    journal: Arc<agentz_desktop_lib::journal::FileJournal>,
    turn: String,
}
impl Fixture {
    async fn new(interactive: bool) -> Self {
        Self::with_delegate(interactive, None).await
    }
    async fn with_delegate(interactive: bool, delegate: Option<Arc<dyn AgentHooks>>) -> Self {
        let root = std::env::temp_dir().join(format!("agentz-hook-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("source.txt"), "original").unwrap();
        let broker = ApprovalBroker::new("boot-fixture".into());
        let cancel = Arc::new(AtomicBool::new(false));
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(Stub("shell")));
        registry.register(Box::new(Stub("file_write")));
        registry.register(Box::new(ReadOnlyStub("file_search", false)));
        registry.register(Box::new(ReadOnlyStub("file_list", false)));
        registry.register(Box::new(ReadOnlyStub("lsp", false)));
        registry.register(Box::new(ReadOnlyStub("read_lints", false)));
        let (tx, rx) = mpsc::unbounded_channel();
        let journal = Arc::new(open_project_journal(&root.to_string_lossy()).unwrap());
        let turn = journal.begin_turn("task-a");
        let hooks = Arc::new(
            ApprovalHooks::new_with_identity(
                root.clone(),
                root.clone(),
                "task-a",
                broker.clone(),
                Arc::new(Sink(tx)),
                delegate.unwrap_or_else(|| journal.clone()),
                Arc::new(registry),
                cancel.clone(),
                interactive,
                ("fixture-run", "fixture-attempt"),
            )
            .await
            .unwrap(),
        );
        Self {
            root,
            broker,
            hooks,
            rx,
            cancel,
            journal,
            turn,
        }
    }
    fn start(&self) -> tokio::task::JoinHandle<HookDecision> {
        let root = self.root.clone();
        let hooks = self.hooks.clone();
        tokio::spawn(async move {
            hooks
                .before_tool(&ToolHookEvent {
                    session_id: "task-a",
                    tool_use_id: "tool-1",
                    tool_name: "shell",
                    input: &json!({"command":"fixture-only"}),
                    workspace_root: &root,
                })
                .await
        })
    }
    async fn event(&mut self) -> Value {
        tokio::time::timeout(Duration::from_secs(5), self.rx.recv())
            .await
            .unwrap()
            .unwrap()
    }
    fn status(&self, id: &str) -> Status {
        ApprovalStore::open_existing(
            &self.root.join(".agentz/approvals.db"),
            "boot-fixture",
            now(),
        )
        .unwrap()
        .get(id)
        .unwrap()
        .unwrap()
        .1
    }
}
#[derive(Default)]
struct CountHooks {
    before: AtomicUsize,
    after: AtomicUsize,
}
#[async_trait]
impl AgentHooks for CountHooks {
    async fn before_tool(&self, _: &ToolHookEvent<'_>) -> HookDecision {
        self.before.fetch_add(1, Ordering::SeqCst);
        HookDecision::Continue
    }
    async fn after_tool(&self, _: &ToolHookEvent<'_>, _: &ToolResult) {
        self.after.fetch_add(1, Ordering::SeqCst);
    }
}
#[cfg(windows)]
struct ReplaceSourceWithJunction {
    outside: PathBuf,
    called: AtomicBool,
}
#[cfg(windows)]
#[async_trait]
impl AgentHooks for ReplaceSourceWithJunction {
    async fn before_tool(&self, event: &ToolHookEvent<'_>) -> HookDecision {
        let source = event.workspace_root.join("src");
        let original = source.with_file_name("src-before-junction");
        std::fs::rename(&source, &original).expect("move preflighted source directory");
        let output = std::process::Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "New-Item -ItemType Junction -Path $env:AGENTZ_TEST_LINK -Target $env:AGENTZ_TEST_TARGET -ErrorAction Stop | Out-Null",
            ])
            .env("AGENTZ_TEST_LINK", &source)
            .env("AGENTZ_TEST_TARGET", &self.outside)
            .output()
            .expect("create out-of-scope junction after approval preflight");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        self.called.store(true, Ordering::SeqCst);
        HookDecision::Continue
    }
}
#[tokio::test]
async fn rejected_and_unmatched_callbacks_never_enter_delegate_and_matching_after_runs_once() {
    let delegate = Arc::new(CountHooks::default());
    let f = Fixture::with_delegate(false, Some(delegate.clone())).await;
    let input = json!({"path":".agentz/approvals.db"});
    let denied = ToolHookEvent {
        session_id: "task-a",
        tool_use_id: "denied",
        tool_name: "file_write",
        input: &input,
        workspace_root: &f.root,
    };
    assert!(matches!(
        f.hooks.before_tool(&denied).await,
        HookDecision::Deny(_)
    ));
    f.hooks
        .after_tool(&denied, &ToolResult::ok("untrusted result"))
        .await;
    assert_eq!(delegate.before.load(Ordering::SeqCst), 0);
    assert_eq!(delegate.after.load(Ordering::SeqCst), 0);
    let input = json!({"path":"source.txt","content":"safe"});
    let accepted = ToolHookEvent {
        session_id: "task-a",
        tool_use_id: "accepted",
        tool_name: "file_write",
        input: &input,
        workspace_root: &f.root,
    };
    f.hooks
        .after_tool(&accepted, &ToolResult::ok("premature result"))
        .await;
    assert_eq!(delegate.after.load(Ordering::SeqCst), 0);
    assert!(matches!(
        f.hooks.before_tool(&accepted).await,
        HookDecision::Continue
    ));
    f.hooks
        .after_tool(&accepted, &ToolResult::ok("complete"))
        .await;
    f.hooks
        .after_tool(&accepted, &ToolResult::ok("duplicate"))
        .await;
    assert_eq!(delegate.before.load(Ordering::SeqCst), 1);
    assert_eq!(delegate.after.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn exact_approval_is_claimed_before_execution_and_finished_by_actual_tool_id() {
    let mut f = Fixture::new(true).await;
    let pending = f.start();
    let event = f.event().await;
    let id = event["request_id"].as_str().unwrap();
    let inbox = agentz_task_state::inbox::InboxStore::open(&f.root).unwrap();
    let notices = inbox.recent(10).unwrap();
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].event_key, format!("approval:{id}"));
    assert_eq!(notices[0].task_id, "task-a");
    assert_eq!(
        notices[0].kind,
        agentz_task_state::inbox::NoticeKind::ActionRequired
    );
    let preview = f.broker.preview(id).await.unwrap();
    assert_eq!(preview["toolInput"]["command"], "fixture-only");
    let intent =
        ApprovalStore::open_existing(&f.root.join(".agentz/approvals.db"), "boot-fixture", now())
            .unwrap()
            .get(id)
            .unwrap()
            .unwrap()
            .0;
    assert_eq!(intent.binding.run_id, "fixture-run");
    assert_eq!(intent.binding.attempt_id, "fixture-attempt");
    assert_eq!(event["tool_input"], Value::Null);
    assert_eq!(f.status(id), Status::Pending);
    f.broker
        .respond(
            id,
            preview["approval"]["digest"].as_str().unwrap(),
            true,
            now(),
        )
        .await
        .unwrap();
    assert!(matches!(pending.await.unwrap(), HookDecision::Continue));
    assert_eq!(
        agentz_task_state::inbox::InboxStore::open(&f.root)
            .unwrap()
            .unread_count()
            .unwrap(),
        0
    );
    assert_eq!(f.status(id), Status::Claimed);
    let ordinary_input = json!({"path":"source.txt","content":"original"});
    let ordinary_event = ToolHookEvent {
        session_id: "task-a",
        tool_use_id: "tool-1",
        tool_name: "file_write",
        input: &ordinary_input,
        workspace_root: &f.root,
    };
    assert!(matches!(
        f.hooks.before_tool(&ordinary_event).await,
        HookDecision::Continue
    ));
    f.hooks
        .after_tool(&ordinary_event, &ToolResult::ok("fixture unchanged"))
        .await;
    assert_eq!(f.status(id), Status::Claimed);
    for (session, command) in [("task-b", "fixture-only"), ("task-a", "altered")] {
        f.hooks
            .after_tool(
                &ToolHookEvent {
                    session_id: session,
                    tool_use_id: "tool-1",
                    tool_name: "shell",
                    input: &json!({"command":command}),
                    workspace_root: &f.root,
                },
                &ToolResult::ok("unrelated result"),
            )
            .await;
        assert_eq!(f.status(id), Status::Claimed);
    }
    // Test fixture performs its side effect only after the actual before hook succeeds.
    std::fs::write(f.root.join("effect.txt"), "one").unwrap();
    f.hooks
        .after_tool(
            &ToolHookEvent {
                session_id: "task-a",
                tool_use_id: "tool-1",
                tool_name: "shell",
                input: &json!({"command":"fixture-only"}),
                workspace_root: &f.root,
            },
            &ToolResult::ok("fixture completed"),
        )
        .await;
    assert_eq!(f.status(id), Status::Executed);
    assert!(f.broker.preview(id).await.is_err());
}
#[tokio::test]
async fn denial_and_background_without_approval_never_reach_execution() {
    let mut f = Fixture::new(true).await;
    let pending = f.start();
    let event = f.event().await;
    let id = event["request_id"].as_str().unwrap();
    f.broker
        .respond(
            id,
            event["approval"]["digest"].as_str().unwrap(),
            false,
            now(),
        )
        .await
        .unwrap();
    assert!(matches!(pending.await.unwrap(), HookDecision::Deny(_)));
    assert_eq!(f.status(id), Status::Denied);
    assert!(!f.root.join("effect.txt").exists());
    let background = Fixture::new(false).await;
    assert!(matches!(
        background.start().await.unwrap(),
        HookDecision::Deny(_)
    ));
    assert!(!background.root.join(".agentz/approvals.db").exists());
}
#[tokio::test]
async fn cancelling_pending_approval_revokes_it_and_cannot_be_replayed() {
    let mut f = Fixture::new(true).await;
    let pending = f.start();
    let event = f.event().await;
    let id = event["request_id"].as_str().unwrap();
    f.cancel.store(true, Ordering::Relaxed);
    assert!(matches!(pending.await.unwrap(), HookDecision::Deny(_)));
    assert_eq!(f.status(id), Status::Revoked);
    assert!(f
        .broker
        .respond(
            id,
            event["approval"]["digest"].as_str().unwrap(),
            true,
            now()
        )
        .await
        .is_err());
}
#[tokio::test]
#[ignore = "Requires AZ-079 candidate kernel; pinned v0.8.68 retains the known Undo defect"]
async fn candidate_repeated_edits_undo_to_turn_start_through_host_hooks() {
    let f = Fixture::new(true).await;
    for id in ["candidate-edit-1", "candidate-edit-2"] {
        let input = json!({"path":"source.txt","content":id});
        let event = ToolHookEvent {
            session_id: "task-a",
            tool_use_id: id,
            tool_name: "file_write",
            input: &input,
            workspace_root: &f.root,
        };
        assert!(matches!(
            f.hooks.before_tool(&event).await,
            HookDecision::Continue
        ));
        std::fs::write(f.root.join("source.txt"), id).unwrap();
        f.hooks.after_tool(&event, &ToolResult::ok("edited")).await;
    }
    assert_eq!(
        std::fs::read_to_string(f.root.join("source.txt")).unwrap(),
        "candidate-edit-2"
    );
    assert_eq!(
        f.journal.undo_turn("task-a", &f.turn).unwrap(),
        vec!["source.txt"]
    );
    assert_eq!(
        std::fs::read_to_string(f.root.join("source.txt")).unwrap(),
        "original"
    );
    assert!(f.journal.undo_turn("task-a", &f.turn).unwrap().is_empty());
}
#[tokio::test]
async fn ordinary_edits_reuse_task_scope_keep_journal_and_block_metadata_and_cross_task() {
    let f = Fixture::new(true).await;

    for id in ["edit-1", "edit-2"] {
        let input = json!({"path":"source.txt","content":id});
        let event = ToolHookEvent {
            session_id: "task-a",
            tool_use_id: id,
            tool_name: "file_write",
            input: &input,
            workspace_root: &f.root,
        };
        assert!(matches!(
            f.hooks.before_tool(&event).await,
            HookDecision::Continue
        ));
        std::fs::write(f.root.join("source.txt"), id).unwrap();
        f.hooks.after_tool(&event, &ToolResult::ok("edited")).await;
        assert_eq!(f.journal.list_changes("task-a", &f.turn).unwrap().len(), 1);
        f.journal.undo_turn("task-a", &f.turn).unwrap();
        assert_eq!(
            std::fs::read_to_string(f.root.join("source.txt")).unwrap(),
            "original"
        );
    }
    assert!(!f.root.join(".agentz/approvals.db").exists());
    for (session, path) in [
        ("task-a", ".agentz/approvals.db"),
        ("task-a", "../outside.txt"),
        ("task-b", "source.txt"),
    ] {
        let input = json!({"path":path,"content":"bad"});
        assert!(matches!(
            f.hooks
                .before_tool(&ToolHookEvent {
                    session_id: session,
                    tool_use_id: "bad",
                    tool_name: "file_write",
                    input: &input,
                    workspace_root: &f.root
                })
                .await,
            HookDecision::Deny(_)
        ));
    }
}
#[tokio::test]
async fn missing_approval_database_does_not_recreate_project_or_deliver_allow() {
    let mut f = Fixture::new(true).await;
    let pending = f.start();
    let event = f.event().await;
    let id = event["request_id"].as_str().unwrap();
    let database = f.root.join(".agentz/approvals.db");
    let moved = database.with_extension("retained");
    std::fs::rename(&database, &moved).unwrap();
    assert!(f
        .broker
        .respond(
            id,
            event["approval"]["digest"].as_str().unwrap(),
            true,
            now()
        )
        .await
        .is_err());
    assert!(matches!(pending.await.unwrap(), HookDecision::Deny(_)));
    assert!(!database.exists());
    let store = ApprovalStore::open_existing(&moved, "boot-fixture", now()).unwrap();
    assert_eq!(store.get(id).unwrap().unwrap().1, Status::Pending);
}

#[tokio::test]
async fn managed_branch_changed_while_waiting_cannot_claim_or_enter_execution() {
    fn git(root: &std::path::Path, args: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().into()
    }
    let root =
        std::env::temp_dir().join(format!("agentz-approval-binding-{}", uuid::Uuid::new_v4()));
    let project = root.join("project");
    let workspace = root.join("task-a");
    std::fs::create_dir_all(&project).unwrap();
    git(&project, &["init", "-b", "main"]);
    git(
        &project,
        &["config", "user.email", "fixture@example.invalid"],
    );
    git(&project, &["config", "user.name", "Fixture"]);
    std::fs::write(project.join("source.txt"), "base").unwrap();
    git(&project, &["add", "."]);
    git(&project, &["commit", "-m", "fixture base"]);
    let base = git(&project, &["rev-parse", "HEAD"]);
    git(
        &project,
        &[
            "worktree",
            "add",
            "-b",
            "workz/task-a",
            workspace.to_str().unwrap(),
            &base,
        ],
    );
    agentz_task_state::register(
        &project,
        &agentz_task_state::TaskEnvironment {
            schema_version: 1,
            task_id: "task-a".into(),
            environment_id: "env-a".into(),
            project_root: agentz_task_state::canonical_root(&project).unwrap(),
            workspace_root: agentz_task_state::canonical_root(&workspace).unwrap(),
            kind: "managed-worktree".into(),
            branch: Some("workz/task-a".into()),
            base_ref: "main".into(),
            base_sha: base,
            created_at: "fixture".into(),
        },
    )
    .unwrap();
    let broker = ApprovalBroker::new("boot-fixture".into());
    let delegate = Arc::new(CountHooks::default());
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(Stub("shell")));
    let (tx, mut rx) = mpsc::unbounded_channel();
    let hooks = Arc::new(
        ApprovalHooks::new(
            project.clone(),
            workspace.clone(),
            "task-a",
            broker.clone(),
            Arc::new(Sink(tx)),
            delegate.clone(),
            Arc::new(registry),
            Arc::new(AtomicBool::new(false)),
            true,
        )
        .await
        .unwrap(),
    );
    let pending = {
        let hooks = hooks.clone();
        let workspace = workspace.clone();
        tokio::spawn(async move {
            hooks
                .before_tool(&ToolHookEvent {
                    session_id: "task-a",
                    tool_use_id: "tool-1",
                    tool_name: "shell",
                    input: &json!({"command":"fixture-only"}),
                    workspace_root: &workspace,
                })
                .await
        })
    };
    let event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .unwrap()
        .unwrap();
    git(&workspace, &["switch", "-c", "changed-after-preview"]);
    let id = event["request_id"].as_str().unwrap();
    broker
        .respond(
            id,
            event["approval"]["digest"].as_str().unwrap(),
            true,
            now(),
        )
        .await
        .unwrap();
    assert!(matches!(pending.await.unwrap(), HookDecision::Deny(_)));
    assert_eq!(delegate.before.load(Ordering::SeqCst), 0);
    assert_eq!(
        std::fs::read_to_string(workspace.join("source.txt")).unwrap(),
        "base"
    );
    let database = project.join(".agentz/approvals.db");
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let status = ApprovalStore::open_existing(&database, "boot-fixture", now())
                .unwrap()
                .get(id)
                .unwrap()
                .unwrap()
                .1;
            if status == Status::Revoked {
                break;
            }
            assert_eq!(status, Status::Approved);
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn search_start_directory_is_bounded_before_delegate_execution() {
    let delegate = Arc::new(CountHooks::default());
    let f = Fixture::with_delegate(false, Some(delegate.clone())).await;
    let outside = f.root.with_extension("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::create_dir(f.root.join("src")).unwrap();
    std::fs::write(f.root.join(".env.local"), "fixture-secret-marker").unwrap();
    for name in ["file_search", "file_list"] {
        assert!(matches!(
            f.hooks
                .before_tool(&ToolHookEvent {
                    session_id: "task-a",
                    tool_use_id: "protected-root",
                    tool_name: name,
                    input: &json!({"path":".", "include_hidden":true}),
                    workspace_root: &f.root,
                })
                .await,
            HookDecision::Deny(_)
        ));
    }
    assert_eq!(delegate.before.load(Ordering::SeqCst), 0);
    for name in ["file_search", "file_list"] {
        for path in [
            outside.to_string_lossy().into_owned(),
            "../outside".into(),
            ".agentz".into(),
        ] {
            assert!(matches!(
                f.hooks
                    .before_tool(&ToolHookEvent {
                        session_id: "task-a",
                        tool_use_id: "bad-search",
                        tool_name: name,
                        input: &json!({"path":path}),
                        workspace_root: &f.root
                    })
                    .await,
                HookDecision::Deny(_)
            ));
        }
        assert_eq!(
            delegate.before.load(Ordering::SeqCst),
            if name == "file_search" { 0 } else { 1 }
        );
        let input = json!({"path":"src"});
        let event = ToolHookEvent {
            session_id: "task-a",
            tool_use_id: name,
            tool_name: name,
            input: &input,
            workspace_root: &f.root,
        };
        assert!(matches!(
            f.hooks.before_tool(&event).await,
            HookDecision::Continue
        ));
        f.hooks.after_tool(&event, &ToolResult::ok("fixture")).await;
    }
    assert_eq!(delegate.before.load(Ordering::SeqCst), 2);
}

#[cfg(windows)]
#[test]
fn codebase_index_does_not_follow_an_external_directory_junction() {
    let root = std::env::temp_dir().join(format!("agentz-index-link-{}", uuid::Uuid::new_v4()));
    let outside = root.with_extension("outside");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("outside.rs"), "pub fn AZBOUNDARYSECRET() {}\n").unwrap();
    let output = std::process::Command::new("powershell.exe").args(["-NoProfile", "-NonInteractive", "-Command",
        "New-Item -ItemType Junction -Path $env:AGENTZ_TEST_LINK -Target $env:AGENTZ_TEST_TARGET -ErrorAction Stop | Out-Null"])
        .env("AGENTZ_TEST_LINK", root.join("linked")).env("AGENTZ_TEST_TARGET", &outside).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        agentz_desktop_lib::commands::codebase::search_index(&root, "AZBOUNDARYSECRET", 5)
            .unwrap()
            .is_empty()
    );
    let symbols = agentz_desktop_lib::commands::symbols::index_for(&root);
    assert!(!symbols
        .symbols
        .iter()
        .any(|symbol| symbol.name == "AZBOUNDARYSECRET"));
    let conn = rusqlite::Connection::open(root.join(".agentz/index.db")).unwrap();
    conn.execute("INSERT INTO chunks(path,start_line,end_line,content,lower) VALUES ('linked/outside.rs',1,1,'AZBOUNDARYSECRET','azboundarysecret')", []).unwrap();
    assert!(
        agentz_desktop_lib::commands::codebase::search_index(&root, "AZBOUNDARYSECRET", 5)
            .unwrap()
            .is_empty()
    );
}

#[cfg(windows)]
#[tokio::test]
async fn recursive_search_rejects_nested_junction_before_provider_execution() {
    let delegate = Arc::new(CountHooks::default());
    let f = Fixture::with_delegate(false, Some(delegate.clone())).await;
    let outside = f.root.with_extension("linked-outside");
    std::fs::create_dir(&outside).unwrap();
    let output=std::process::Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command",
        "New-Item -ItemType Junction -Path $env:AGENTZ_TEST_LINK -Target $env:AGENTZ_TEST_TARGET -ErrorAction Stop | Out-Null"])
        .env("AGENTZ_TEST_LINK",f.root.join("linked")).env("AGENTZ_TEST_TARGET",&outside).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for name in ["file_search", "file_list"] {
        let input = json!({"path":".","recursive":true});
        assert!(matches!(
            f.hooks
                .before_tool(&ToolHookEvent {
                    session_id: "task-a",
                    tool_use_id: name,
                    tool_name: name,
                    input: &input,
                    workspace_root: &f.root
                })
                .await,
            HookDecision::Deny(_)
        ));
    }
    assert_eq!(delegate.before.load(Ordering::SeqCst), 0);
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(ReadOnlyStub("file_search", false)));
    registry.register(Box::new(ReadOnlyStub("file_list", false)));
    agentz_desktop_lib::runtime::approval::restrict_unapproved_background(&mut registry);
    let ctx = ToolContext {
        session_id: "task-a".into(),
        workspace_root: f.root.clone(),
        bypass_permissions: true,
        settings: Arc::new(Default::default()),
        max_iterations: None,
        memory_owner_id: "fixture".into(),
        pool_session_id: None,
        tool_use_id: None,
        cancel: f.cancel.clone(),
        loop_halt: None,
    };
    for name in ["file_search", "file_list"] {
        assert!(
            registry
                .get(name)
                .unwrap()
                .call(json!({"path":".","recursive":true}), &ctx)
                .await
                .unwrap()
                .is_error
        );
    }
}

#[cfg(windows)]
#[tokio::test]
async fn recursive_search_rejects_junction_replaced_by_downstream_hook() {
    for name in ["file_search", "file_list"] {
        let outside_root =
            std::env::temp_dir().join(format!("agentz-hook-outside-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&outside_root).unwrap();
        std::fs::write(outside_root.join("secret.rs"), "AZ080_OUTSIDE_SECRET").unwrap();
        let replace = Arc::new(ReplaceSourceWithJunction {
            outside: outside_root.clone(),
            called: AtomicBool::new(false),
        });
        let f = Fixture::with_delegate(false, Some(replace.clone())).await;
        std::fs::create_dir(f.root.join("src")).unwrap();
        std::fs::write(f.root.join("src/safe.rs"), "safe fixture").unwrap();
        let input = json!({"path":"src","recursive":true,"max_depth":3});
        let decision = f
            .hooks
            .before_tool(&ToolHookEvent {
                session_id: "task-a",
                tool_use_id: name,
                tool_name: name,
                input: &input,
                workspace_root: &f.root,
            })
            .await;
        assert!(
            matches!(decision, HookDecision::Deny(_)),
            "{name} must be denied if the downstream hook replaces its source with an out-of-scope junction"
        );
        assert!(replace.called.load(Ordering::SeqCst));
        assert!(f.root.join("src-before-junction/safe.rs").exists());
        assert!(outside_root.join("secret.rs").exists());
    }
}
#[test]
fn historical_index_rows_cannot_return_outside_missing_or_protected_sources() {
    let root = std::env::temp_dir().join(format!("agentz-old-index-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("safe.rs"), "pub fn cacheboundmarker() {}\n").unwrap();
    std::fs::write(root.join(".env.rs"), "cacheboundmarker secret").unwrap();
    agentz_desktop_lib::commands::codebase::build_index(&root).unwrap();
    let conn = rusqlite::Connection::open(root.join(".agentz/index.db")).unwrap();
    for path in [
        "../outside.rs".to_string(),
        root.join("safe.rs").to_string_lossy().into_owned(),
        "missing.rs".into(),
        ".env.rs".into(),
        ".agentz/private.rs".into(),
    ] {
        conn.execute("INSERT INTO chunks(path,start_line,end_line,content,lower) VALUES (?1,1,1,'cacheboundmarker secret','cacheboundmarker secret')", [&path]).unwrap();
    }
    let hits = agentz_desktop_lib::commands::codebase::search_index(&root, "cacheboundmarker", 50)
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].path, "safe.rs");
}

#[tokio::test]
async fn direct_codez_branch_change_invalidates_approval_without_task_registration() {
    let mut f = Fixture::new(true).await;
    for args in [
        vec!["init", "-b", "main"],
        vec!["config", "user.email", "fixture@example.invalid"],
        vec!["config", "user.name", "Fixture"],
        vec!["add", "source.txt"],
        vec!["commit", "-m", "base"],
    ] {
        assert!(std::process::Command::new("git")
            .args(args)
            .current_dir(&f.root)
            .output()
            .unwrap()
            .status
            .success());
    }
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(Stub("shell")));
    let (tx, rx) = mpsc::unbounded_channel();
    let delegate = Arc::new(CountHooks::default());
    f.hooks = Arc::new(
        ApprovalHooks::new(
            f.root.clone(),
            f.root.clone(),
            "task-a",
            f.broker.clone(),
            Arc::new(Sink(tx)),
            delegate.clone(),
            Arc::new(registry),
            f.cancel.clone(),
            true,
        )
        .await
        .unwrap(),
    );
    f.rx = rx;
    let pending = f.start();
    let event = f.event().await;
    assert!(std::process::Command::new("git")
        .args(["switch", "-c", "after-preview"])
        .current_dir(&f.root)
        .output()
        .unwrap()
        .status
        .success());
    let id = event["request_id"].as_str().unwrap();
    f.broker
        .respond(
            id,
            event["approval"]["digest"].as_str().unwrap(),
            true,
            now(),
        )
        .await
        .unwrap();
    assert!(matches!(pending.await.unwrap(), HookDecision::Deny(_)));
    assert_eq!(delegate.before.load(Ordering::SeqCst), 0);
    assert!(agentz_task_state::load(&f.root, "task-a")
        .unwrap()
        .is_none());
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let status = f.status(id);
            if status == Status::Revoked {
                break;
            }
            assert_eq!(status, Status::Approved);
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn lsp_scope_and_diagnostics_background_denial() {
    let f = Fixture::new(false).await;
    for (name, input) in [
        ("lsp", json!({"file":"../outside.rs"})),
        ("lsp", json!({"file":".env"})),
        ("read_lints", json!({"paths":[]})),
        ("read_lints", json!({"paths":["source.txt"]})),
    ] {
        assert!(matches!(
            f.hooks
                .before_tool(&ToolHookEvent {
                    session_id: "task-a",
                    tool_use_id: "guard",
                    tool_name: name,
                    input: &input,
                    workspace_root: &f.root
                })
                .await,
            HookDecision::Deny(_)
        ));
    }
    assert!(matches!(
        f.hooks
            .before_tool(&ToolHookEvent {
                session_id: "task-a",
                tool_use_id: "safe",
                tool_name: "lsp",
                input: &json!({"file":"source.txt"}),
                workspace_root: &f.root
            })
            .await,
        HookDecision::Deny(_)
    ));
}
#[tokio::test]
async fn diagnostic_build_fallback_is_claimed_only_after_explicit_approval() {
    for name in ["read_lints", "lsp"] {
        let input = if name == "lsp" {
            json!({"file":"source.txt", "action":"hover", "line":1, "character":0})
        } else {
            json!({"paths":["source.txt"]})
        };
        let mut f = Fixture::new(true).await;
        let pending = {
            let hooks = f.hooks.clone();
            let root = f.root.clone();
            let input = input.clone();
            tokio::spawn(async move {
                hooks
                    .before_tool(&ToolHookEvent {
                        session_id: "task-a",
                        tool_use_id: "lint",
                        tool_name: name,
                        input: &input,
                        workspace_root: &root,
                    })
                    .await
            })
        };
        let event = f.event().await;
        let id = event["request_id"].as_str().unwrap();
        assert_eq!(f.status(id), Status::Pending);
        assert_eq!(f.broker.preview(id).await.unwrap()["toolInput"], input);
        f.broker
            .respond(
                id,
                event["approval"]["digest"].as_str().unwrap(),
                true,
                now(),
            )
            .await
            .unwrap();
        assert!(matches!(pending.await.unwrap(), HookDecision::Continue));
        assert_eq!(f.status(id), Status::Claimed);
        f.hooks
            .after_tool(
                &ToolHookEvent {
                    session_id: "task-a",
                    tool_use_id: "lint",
                    tool_name: name,
                    input: &input,
                    workspace_root: &f.root,
                },
                &ToolResult::ok("fixture; no compiler started"),
            )
            .await;
        assert_eq!(f.status(id), Status::Executed);
    }
}
#[tokio::test]
async fn non_git_preview_cannot_survive_a_new_invalid_git_marker() {
    let mut f = Fixture::new(true).await;
    let pending = f.start();
    let event = f.event().await;
    std::fs::write(f.root.join(".git"), "gitdir: missing-fixture-git\n").unwrap();
    let id = event["request_id"].as_str().unwrap();
    f.broker
        .respond(
            id,
            event["approval"]["digest"].as_str().unwrap(),
            true,
            now(),
        )
        .await
        .unwrap();
    assert!(matches!(pending.await.unwrap(), HookDecision::Deny(_)));
    assert!(!f.root.join("effect.txt").exists());
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let status = f.status(id);
            if status == Status::Revoked {
                break;
            }
            assert_eq!(status, Status::Approved);
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
}
