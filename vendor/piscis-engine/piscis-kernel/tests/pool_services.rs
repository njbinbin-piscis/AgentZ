//! Integration tests for `piscis_kernel::pool::services`.
//!
//! These tests exercise the kernel pool layer end-to-end against a real
//! in-memory SQLite `Database`, a capturing `PoolEventSink`, and the
//! in-process [`StubSubagentRuntime`]. They intentionally do NOT spin
//! up an LLM, Tauri app handle, or filesystem worktree — the only side
//! effect under test is `(DB rows, emitted events)`.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use piscis_core::host::{KoiTurnRequest, PoolEvent, PoolEventSink};
use piscis_kernel::agent::tool::{Tool, ToolContext, ToolSettings};
use piscis_kernel::pool::{
    self, coordinator::CoordinatorConfig, services, store::PoolStore, AssignKoiArgs, CallerContext,
    CreatePoolArgs, CreateTodoArgs, DeleteTodoArgs, PostStatusArgs, SendPoolMessageArgs,
    StubOutcome, StubSubagentRuntime, WaitForKoiArgs,
};
use piscis_kernel::store::Database;
use piscis_kernel::tools::pool_chat::PoolChatTool;
use tokio::sync::Mutex;

// ─── test plumbing ─────────────────────────────────────────────────────

#[derive(Default)]
struct CapturingSink {
    events: StdMutex<Vec<PoolEvent>>,
}

impl CapturingSink {
    fn drain_kinds(&self) -> Vec<&'static str> {
        let events = self.events.lock().unwrap();
        events.iter().map(|e| e.kind()).collect()
    }

    fn count(&self) -> usize {
        self.events.lock().unwrap().len()
    }
}

impl PoolEventSink for CapturingSink {
    fn emit_pool(&self, event: &PoolEvent) {
        self.events.lock().unwrap().push(event.clone());
    }
}

fn piscis_caller<'a>(session_id: &'a str) -> CallerContext<'a> {
    CallerContext {
        memory_owner_id: "piscis",
        session_id,
        session_source: Some("test"),
        pool_session_id: None,
        cancel: None,
    }
}

fn koi_caller<'a>(koi: &'a str, pool_id: &'a str) -> CallerContext<'a> {
    CallerContext {
        memory_owner_id: koi,
        session_id: koi,
        session_source: Some("test"),
        pool_session_id: Some(pool_id),
        cancel: None,
    }
}

fn build_store() -> PoolStore {
    let db = Database::open_in_memory().expect("open in-memory db");
    // Satisfy `koi_todos.owner_id`/`claimed_by` FK constraints for every
    // caller used in the suite. Production hosts seed Piscis via
    // `ensure_starter_kois`; tests insert them explicitly so they can
    // use stable literal ids.
    for (id, name) in [
        ("piscis", "Piscis"),
        ("koi-alpha", "Alpha"),
        ("koi-beta", "Beta"),
    ] {
        db.upsert_koi_with_id(id, name).expect("seed koi");
    }
    PoolStore::new(Arc::new(Mutex::new(db)))
}

async fn create_test_pool(store: &PoolStore, sink: &Arc<CapturingSink>) -> String {
    let caller = piscis_caller("sess-1");
    let value = services::create_pool(
        store,
        sink.as_ref(),
        &caller,
        CreatePoolArgs {
            name: "Integration Pool".into(),
            project_dir: None,
            org_spec: Some("be nice".into()),
            task_timeout_secs: 0,
            origin_im_binding_key: None,
        },
    )
    .await
    .expect("create_pool");
    value["pool"]["id"].as_str().unwrap().to_string()
}

// ─── actual tests ──────────────────────────────────────────────────────

fn make_sink() -> Arc<CapturingSink> {
    Arc::new(CapturingSink::default())
}

fn sink_arc(sink: &Arc<CapturingSink>) -> Arc<dyn PoolEventSink> {
    sink.clone() as Arc<dyn PoolEventSink>
}

#[tokio::test]
async fn create_pool_emits_pool_created_and_welcome_message() {
    let store = build_store();
    let sink = make_sink();
    let pool_id = create_test_pool(&store, &sink).await;
    assert!(!pool_id.is_empty());

    let kinds = sink.drain_kinds();
    assert_eq!(
        kinds,
        vec!["pool_created", "message_appended"],
        "create_pool must emit exactly PoolCreated + MessageAppended"
    );
}

#[tokio::test]
async fn create_pool_persists_origin_im_binding_key_when_provided() {
    let store = build_store();
    let sink = make_sink();
    let caller = piscis_caller("sess-im");
    let value = services::create_pool(
        &store,
        sink.as_ref(),
        &caller,
        CreatePoolArgs {
            name: "IM-originated pool".into(),
            project_dir: None,
            org_spec: None,
            task_timeout_secs: 0,
            origin_im_binding_key: Some("wechat::dm:user-1".into()),
        },
    )
    .await
    .expect("create_pool");
    let pool_id = value["pool"]["id"].as_str().unwrap().to_string();

    let pool = store
        .read(move |db| db.get_pool_session(&pool_id))
        .await
        .expect("read pool")
        .expect("pool exists");
    assert_eq!(
        pool.origin_im_binding_key.as_deref(),
        Some("wechat::dm:user-1")
    );
}

#[tokio::test]
async fn create_pool_leaves_origin_im_binding_key_none_for_desktop_callers() {
    let store = build_store();
    let sink = make_sink();
    let pool_id = create_test_pool(&store, &sink).await;

    let pool = store
        .read(move |db| db.get_pool_session(&pool_id))
        .await
        .expect("read pool")
        .expect("pool exists");
    assert!(pool.origin_im_binding_key.is_none());
}

#[tokio::test]
async fn send_pool_message_emits_message_appended() {
    let store = build_store();
    let sink = make_sink();
    let pool_id = create_test_pool(&store, &sink).await;

    let caller = piscis_caller("sess-1");
    let before = sink.count();
    let cfg = CoordinatorConfig::default();
    let msg = services::send_pool_message(
        &store,
        sink_arc(&sink),
        None,
        &cfg,
        &caller,
        SendPoolMessageArgs {
            pool_id: pool_id.clone(),
            sender_id: "piscis".into(),
            content: "hello pool".into(),
            reply_to_message_id: None,
        },
    )
    .await
    .expect("send_pool_message");
    assert_eq!(msg.pool_session_id, pool_id);
    assert_eq!(msg.content, "hello pool");

    let all = sink.events.lock().unwrap();
    assert_eq!(all.len(), before + 1, "exactly one new event expected");
    match &all[before] {
        PoolEvent::MessageAppended {
            pool_id: pid,
            message,
        } => {
            assert_eq!(pid, &pool_id);
            assert_eq!(message.content, "hello pool");
        }
        other => panic!("expected MessageAppended, got {:?}", other.kind()),
    }
}

#[tokio::test]
async fn pool_chat_tool_rejects_piscis_send_but_allows_koi_send() {
    let store = build_store();
    let sink = make_sink();
    let pool_id = create_test_pool(&store, &sink).await;
    let cfg = CoordinatorConfig::default();
    let tool = PoolChatTool {
        store: store.clone(),
        sink: sink_arc(&sink),
        subagent: None,
        coordinator_cfg: cfg,
    };
    let base_ctx = |memory_owner_id: &str| ToolContext {
        session_id: format!("session-{memory_owner_id}"),
        workspace_root: PathBuf::from("."),
        bypass_permissions: false,
        settings: Arc::new(ToolSettings::default()),
        max_iterations: None,
        memory_owner_id: memory_owner_id.to_string(),
        pool_session_id: Some(pool_id.clone()),
        tool_use_id: None,
        cancel: Arc::new(AtomicBool::new(false)),
        loop_halt: None,
    };

    let piscis_result = tool
        .call(
            serde_json::json!({
                "action": "send",
                "content": "@!Alpha do this through the wrong channel"
            }),
            &base_ctx("piscis"),
        )
        .await
        .expect("piscis tool call");
    assert!(piscis_result.is_error);
    assert!(piscis_result.content.contains("cannot send pool_chat"));

    let koi_result = tool
        .call(
            serde_json::json!({
                "action": "send",
                "content": "Koi-visible progress update"
            }),
            &base_ctx("koi-alpha"),
        )
        .await
        .expect("koi tool call");
    assert!(!koi_result.is_error, "{}", koi_result.content);
}

#[tokio::test]
async fn set_pool_status_pause_then_resume() {
    let store = build_store();
    let sink = make_sink();
    let pool_id = create_test_pool(&store, &sink).await;

    let caller = piscis_caller("sess-1");
    services::set_pool_status(&store, sink.as_ref(), &caller, &pool_id, "paused")
        .await
        .expect("pause");
    services::set_pool_status(&store, sink.as_ref(), &caller, &pool_id, "active")
        .await
        .expect("resume");

    let kinds = sink.drain_kinds();
    // create_pool + welcome + (pool_paused + status_msg) + (pool_resumed + status_msg)
    assert!(
        kinds.contains(&"pool_paused"),
        "expected pool_paused event in {:?}",
        kinds
    );
    assert!(
        kinds.contains(&"pool_resumed"),
        "expected pool_resumed event in {:?}",
        kinds
    );
}

#[tokio::test]
async fn archive_rejects_pool_with_active_todos() {
    let store = build_store();
    let sink = make_sink();
    let pool_id = create_test_pool(&store, &sink).await;

    let caller = piscis_caller("sess-1");
    services::create_todo(
        &store,
        sink.as_ref(),
        &caller,
        CreateTodoArgs {
            pool_id: pool_id.clone(),
            title: "ship the feature".into(),
            description: "do it".into(),
            priority: "".into(),
            timeout_secs: 0,
            depends_on: None,
        },
    )
    .await
    .expect("create_todo");

    let err = services::set_pool_status(&store, sink.as_ref(), &caller, &pool_id, "archived")
        .await
        .expect_err("archive must bail when active todos remain");
    assert!(
        err.to_string().contains("active todo"),
        "unexpected archive error: {}",
        err
    );
}

#[tokio::test]
async fn koi_can_only_manage_its_own_todos() {
    let store = build_store();
    let sink = make_sink();
    let pool_id = create_test_pool(&store, &sink).await;

    // Piscis creates a todo owned by piscis.
    let piscis = piscis_caller("sess-1");
    let created = services::create_todo(
        &store,
        sink.as_ref(),
        &piscis,
        CreateTodoArgs {
            pool_id: pool_id.clone(),
            title: "piscis task".into(),
            description: "".into(),
            priority: "".into(),
            timeout_secs: 0,
            depends_on: None,
        },
    )
    .await
    .expect("create_todo");
    let todo_id = created["todo"]["id"].as_str().unwrap().to_string();

    // A Koi tries to cancel it — permission denied.
    let koi = koi_caller("koi-alpha", &pool_id);
    let err = services::cancel_todo(&store, sink.as_ref(), &koi, &todo_id, "no reason")
        .await
        .expect_err("koi must not cancel piscis-owned todo");
    assert!(
        err.to_string().contains("Permission denied"),
        "unexpected ownership error: {}",
        err
    );

    // Piscis can cancel it just fine.
    services::cancel_todo(&store, sink.as_ref(), &piscis, &todo_id, "scope changed")
        .await
        .expect("piscis cancel");
    let kinds = sink.drain_kinds();
    assert!(
        kinds.contains(&"todo_changed"),
        "cancel must emit TodoChanged in {:?}",
        kinds
    );
}

#[tokio::test]
async fn assign_koi_creates_todo_posts_mention_and_emits_events() {
    let store = build_store();
    let sink = make_sink();
    let pool_id = create_test_pool(&store, &sink).await;

    let caller = piscis_caller("sess-1");
    // assign_koi now requires the Koi to be a project member first.
    services::add_member(&store, sink.as_ref(), &caller, &pool_id, "koi-alpha")
        .await
        .expect("add_member");
    let subagent = Arc::new(StubSubagentRuntime::always_complete("ok"))
        as Arc<dyn piscis_core::host::SubagentRuntime>;
    let cfg = CoordinatorConfig::default();
    let value = services::assign_koi(
        &store,
        sink_arc(&sink),
        Some(subagent),
        &cfg,
        &caller,
        AssignKoiArgs {
            pool_id: pool_id.clone(),
            koi_id: "koi-alpha".into(),
            task: "build the thing".into(),
            priority: "high".into(),
            timeout_secs: 30,
            context: None,
            depends_on: None,
        },
    )
    .await
    .expect("assign_koi");
    assert_eq!(value["koi_id"], "koi-alpha");
    assert_eq!(value["next_required_action"]["tool"], "pool_org");
    assert_eq!(value["next_required_action"]["action"], "get_todos");
    assert!(
        value["summary"]
            .as_str()
            .unwrap_or_default()
            .contains("get_todos"),
        "assign_koi must direct Piscis to check with get_todos"
    );

    let kinds = sink.drain_kinds();
    // Assign path emits: todo_changed (created), message_appended
    // (mention), koi_assigned. (The coordinator also spawns a
    // fire-and-forget turn but its events may land later in a separate
    // task, so we only assert the synchronous tail here.)
    assert!(
        kinds
            .windows(3)
            .any(|w| w == ["todo_changed", "message_appended", "koi_assigned"]),
        "expected todo_changed/message_appended/koi_assigned in {:?}",
        kinds
    );
    // Silence the clippy lint about unused modules.
    let _ = pool::session_source::PISCIS_POOL;
}

#[tokio::test]
async fn post_status_is_controlled_and_does_not_dispatch_mentions() {
    let store = build_store();
    let sink = make_sink();
    let pool_id = create_test_pool(&store, &sink).await;
    let caller = piscis_caller("sess-1");

    let value = services::post_status(
        &store,
        sink.as_ref(),
        &caller,
        PostStatusArgs {
            pool_id: pool_id.clone(),
            content: "@!Alpha supervisor note only; do not fan out".into(),
            event_type: None,
        },
    )
    .await
    .expect("post_status");
    assert_eq!(value["pool_id"], pool_id);

    let alpha_todos = store
        .read(|db| db.list_koi_todos(Some("koi-alpha")))
        .await
        .expect("list alpha todos");
    assert!(
        alpha_todos.is_empty(),
        "post_status must not trigger @! mention todo creation"
    );
}

#[tokio::test]
async fn wait_for_koi_uses_elapsed_time_before_timeout() {
    let store = build_store();
    let sink = make_sink();
    let pool_id = create_test_pool(&store, &sink).await;
    let caller = piscis_caller("sess-1");
    services::create_todo(
        &store,
        sink.as_ref(),
        &caller,
        CreateTodoArgs {
            pool_id: pool_id.clone(),
            title: "wait on this".into(),
            description: "".into(),
            priority: "".into(),
            timeout_secs: 0,
            depends_on: None,
        },
    )
    .await
    .expect("create_todo");

    let started = Instant::now();
    let value = services::wait_for_koi(
        &store,
        &caller,
        WaitForKoiArgs {
            pool_id: pool_id.clone(),
            koi_id: Some("piscis".into()),
            todo_id: None,
            min_wait_secs: 0,
            timeout_secs: 1,
            initial_backoff_ms: 25,
            max_backoff_ms: 25,
        },
    )
    .await
    .expect("wait_for_koi");
    assert!(value["timed_out"].as_bool().unwrap_or(false));
    assert!(
        started.elapsed() >= Duration::from_millis(900),
        "wait_for_koi must use real elapsed time, not immediate status checks"
    );
}

#[tokio::test]
async fn plain_mention_is_chat_only_and_does_not_dispatch_subagent() {
    // Post-refactor mention semantics: `@Name` is a chat-only
    // notification. The coordinator MUST NOT spawn a Koi turn and MUST
    // NOT pre-create a board todo. The idle Koi observes the message
    // through the normal `MessageAppended` event on its next turn —
    // only `@!Name` (forced delegation) wakes a runtime turn.
    let store = build_store();
    let sink = make_sink();
    let pool_id = create_test_pool(&store, &sink).await;
    let caller = piscis_caller("sess-1");
    let requests: Arc<StdMutex<Vec<KoiTurnRequest>>> = Arc::new(StdMutex::new(Vec::new()));
    let requests_cl = requests.clone();
    let subagent = Arc::new(StubSubagentRuntime::new(move |request| {
        requests_cl.lock().unwrap().push(request.clone());
        StubOutcome::Completed("noticed".into())
    })) as Arc<dyn piscis_core::host::SubagentRuntime>;
    let cfg = CoordinatorConfig::default();

    let baseline_events = sink.count();
    services::send_pool_message(
        &store,
        sink_arc(&sink),
        Some(subagent),
        &cfg,
        &caller,
        SendPoolMessageArgs {
            pool_id: pool_id.clone(),
            sender_id: "piscis".into(),
            content:
                "@Alpha please review the latest status and decide whether follow-up is needed."
                    .into(),
            reply_to_message_id: None,
        },
    )
    .await
    .expect("send_pool_message");

    tokio::time::sleep(Duration::from_millis(25)).await;

    let requests = requests.lock().unwrap().clone();
    assert_eq!(
        requests.len(),
        0,
        "plain @mention must not dispatch a Koi turn under the post-refactor semantics; \
         only @!mention triggers execution"
    );

    let todos = store
        .read(|db| db.list_koi_todos(Some("koi-alpha")))
        .await
        .expect("list todos");
    assert!(
        todos.is_empty(),
        "plain @mention must not create board todos"
    );

    let new_kinds: Vec<&'static str> = sink
        .drain_kinds()
        .into_iter()
        .skip(baseline_events)
        .collect();
    assert!(
        new_kinds.contains(&"message_appended"),
        "plain @mention must still surface as a MessageAppended event so idle Kois can observe \
         it on their next turn; got events: {:?}",
        new_kinds
    );
    assert!(
        !new_kinds.contains(&"todo_changed"),
        "plain @mention must not emit any TodoChanged events; got events: {:?}",
        new_kinds
    );
}

#[tokio::test]
async fn forced_mention_creates_todo_and_dispatches_execution() {
    let store = build_store();
    let sink = make_sink();
    let pool_id = create_test_pool(&store, &sink).await;
    let caller = piscis_caller("sess-1");
    let requests: Arc<StdMutex<Vec<KoiTurnRequest>>> = Arc::new(StdMutex::new(Vec::new()));
    let requests_cl = requests.clone();
    let subagent = Arc::new(StubSubagentRuntime::new(move |request| {
        requests_cl.lock().unwrap().push(request.clone());
        StubOutcome::Completed("done".into())
    })) as Arc<dyn piscis_core::host::SubagentRuntime>;
    let cfg = CoordinatorConfig::default();

    services::send_pool_message(
        &store,
        sink_arc(&sink),
        Some(subagent),
        &cfg,
        &caller,
        SendPoolMessageArgs {
            pool_id: pool_id.clone(),
            sender_id: "piscis".into(),
            content: "@!Alpha implement the migration and report back.".into(),
            reply_to_message_id: None,
        },
    )
    .await
    .expect("send_pool_message");

    tokio::time::sleep(Duration::from_millis(25)).await;

    let requests = requests.lock().unwrap().clone();
    assert_eq!(requests.len(), 1, "expected one delegated wake-up");
    assert_eq!(requests[0].koi_id, "koi-alpha");
    assert!(
        requests[0].todo_id.is_some(),
        "forced @!mention must dispatch against a todo"
    );

    let todos = store
        .read(|db| db.list_koi_todos(Some("koi-alpha")))
        .await
        .expect("list todos");
    assert_eq!(todos.len(), 1, "forced @!mention should create a todo");
}

#[tokio::test]
async fn forced_mention_to_busy_koi_queues_todo_without_parallel_dispatch() {
    let store = build_store();
    let sink = make_sink();
    let pool_id = create_test_pool(&store, &sink).await;
    store
        .write(|db| db.update_koi_status("koi-alpha", "busy"))
        .await
        .expect("mark alpha busy");
    let caller = piscis_caller("sess-1");
    let requests: Arc<StdMutex<Vec<KoiTurnRequest>>> = Arc::new(StdMutex::new(Vec::new()));
    let requests_cl = requests.clone();
    let subagent = Arc::new(StubSubagentRuntime::new(move |request| {
        requests_cl.lock().unwrap().push(request.clone());
        StubOutcome::Completed("done".into())
    })) as Arc<dyn piscis_core::host::SubagentRuntime>;
    let cfg = CoordinatorConfig::default();

    services::send_pool_message(
        &store,
        sink_arc(&sink),
        Some(subagent),
        &cfg,
        &caller,
        SendPoolMessageArgs {
            pool_id: pool_id.clone(),
            sender_id: "piscis".into(),
            content: "@!Alpha queue this while the current turn is running.".into(),
            reply_to_message_id: None,
        },
    )
    .await
    .expect("send_pool_message");

    tokio::time::sleep(Duration::from_millis(25)).await;

    assert_eq!(
        requests.lock().unwrap().len(),
        0,
        "busy Koi must not receive a concurrent spawned turn"
    );
    let todos = store
        .read(|db| db.list_koi_todos(Some("koi-alpha")))
        .await
        .expect("list todos");
    assert_eq!(todos.len(), 1, "busy Koi should receive queued todo");
    assert_eq!(todos[0].status, "todo");
}

#[tokio::test]
async fn concurrent_dispatch_of_same_pending_todo_runs_exactly_once() {
    use piscis_kernel::pool::coordinator::{execute_todo_turn, ExecuteTodoArgs};

    let store = build_store();
    let sink = make_sink();
    let pool_id = create_test_pool(&store, &sink).await;

    // A single pending todo owned by an idle Koi.
    let pool_for_todo = pool_id.clone();
    let todo = store
        .write(move |db| {
            db.create_koi_todo(
                "koi-alpha",
                "shared task",
                "",
                "medium",
                "piscis",
                Some(&pool_for_todo),
                "koi",
                None,
                0,
            )
        })
        .await
        .expect("create todo");

    // Stub holds each turn open long enough for both dispatchers to overlap.
    let requests: Arc<StdMutex<Vec<KoiTurnRequest>>> = Arc::new(StdMutex::new(Vec::new()));
    let requests_cl = requests.clone();
    let subagent = Arc::new(StubSubagentRuntime::new(move |request| {
        requests_cl.lock().unwrap().push(request.clone());
        StubOutcome::Delayed {
            delay: Duration::from_millis(200),
            response_text: "done".into(),
        }
    })) as Arc<dyn piscis_core::host::SubagentRuntime>;
    let cfg = CoordinatorConfig::default();

    let short = &todo.id[..8.min(todo.id.len())];
    let mk_args = || ExecuteTodoArgs {
        koi_id: "koi-alpha".into(),
        todo_id: todo.id.clone(),
        assign_msg_id: None,
        session_id: format!("koi_task_koi-alpha_{}", short),
        extra_tool_profile: Vec::new(),
        extra_system_context: None,
    };

    // Two concurrent dispatchers race on the same pending todo.
    let (a, b) = tokio::join!(
        execute_todo_turn(&store, sink_arc(&sink), subagent.clone(), &cfg, mk_args()),
        execute_todo_turn(&store, sink_arc(&sink), subagent.clone(), &cfg, mk_args()),
    );
    let a = a.expect("turn a");
    let b = b.expect("turn b");

    // Exactly one real subagent turn was spawned; the loser was fenced out.
    assert_eq!(
        requests.lock().unwrap().len(),
        1,
        "the same pending todo must not be dispatched twice concurrently"
    );
    assert!(
        a.success ^ b.success,
        "exactly one dispatcher should have run the turn (a.success={}, b.success={})",
        a.success,
        b.success
    );
}

#[tokio::test]
async fn concurrent_dispatch_of_two_todos_for_one_idle_koi_serializes() {
    use piscis_kernel::pool::coordinator::{execute_todo_turn, ExecuteTodoArgs};

    let store = build_store();
    let sink = make_sink();
    let pool_id = create_test_pool(&store, &sink).await;

    // Two distinct pending todos, both owned by the same idle Koi.
    let pool_a = pool_id.clone();
    let todo_a = store
        .write(move |db| {
            db.create_koi_todo(
                "koi-alpha",
                "task a",
                "",
                "medium",
                "piscis",
                Some(&pool_a),
                "koi",
                None,
                0,
            )
        })
        .await
        .expect("create todo a");
    let pool_b = pool_id.clone();
    let todo_b = store
        .write(move |db| {
            db.create_koi_todo(
                "koi-alpha",
                "task b",
                "",
                "medium",
                "piscis",
                Some(&pool_b),
                "koi",
                None,
                0,
            )
        })
        .await
        .expect("create todo b");

    // Stub holds each turn open so both dispatchers overlap in time.
    let requests: Arc<StdMutex<Vec<KoiTurnRequest>>> = Arc::new(StdMutex::new(Vec::new()));
    let requests_cl = requests.clone();
    let subagent = Arc::new(StubSubagentRuntime::new(move |request| {
        requests_cl.lock().unwrap().push(request.clone());
        StubOutcome::Delayed {
            delay: Duration::from_millis(150),
            response_text: "done".into(),
        }
    })) as Arc<dyn piscis_core::host::SubagentRuntime>;
    let cfg = CoordinatorConfig::default();

    let args = |todo: &piscis_core::models::KoiTodo| ExecuteTodoArgs {
        koi_id: "koi-alpha".into(),
        todo_id: todo.id.clone(),
        assign_msg_id: None,
        session_id: format!("koi_task_koi-alpha_{}", &todo.id[..8.min(todo.id.len())]),
        extra_tool_profile: Vec::new(),
        extra_system_context: None,
    };

    let _ = tokio::join!(
        execute_todo_turn(
            &store,
            sink_arc(&sink),
            subagent.clone(),
            &cfg,
            args(&todo_a)
        ),
        execute_todo_turn(
            &store,
            sink_arc(&sink),
            subagent.clone(),
            &cfg,
            args(&todo_b)
        ),
    );

    // The Koi is single-threaded: while one turn is in flight the other
    // dispatch is fenced out, so only one subagent turn starts.
    assert_eq!(
        requests.lock().unwrap().len(),
        1,
        "a single Koi must never run two turns concurrently"
    );
}

#[tokio::test]
async fn koi_has_in_progress_todo_tracks_board_state() {
    let store = build_store();
    let sink = make_sink();
    let pool_id = create_test_pool(&store, &sink).await;

    // No work yet → not busy by board.
    let busy0 = store
        .read(|db| db.koi_has_in_progress_todo("koi-alpha"))
        .await
        .expect("query");
    assert!(!busy0, "koi with no todos is not board-busy");

    let pool_for_todo = pool_id.clone();
    let todo = store
        .write(move |db| {
            db.create_koi_todo(
                "koi-alpha",
                "t",
                "",
                "medium",
                "piscis",
                Some(&pool_for_todo),
                "koi",
                None,
                0,
            )
        })
        .await
        .expect("create todo");

    // Pending todo (status='todo') is not in_progress.
    let busy1 = store
        .read(|db| db.koi_has_in_progress_todo("koi-alpha"))
        .await
        .expect("query");
    assert!(!busy1, "a pending (todo) row is not board-busy");

    let claim_id = todo.id.clone();
    store
        .write(move |db| db.claim_koi_todo(&claim_id, "koi-alpha"))
        .await
        .expect("claim");
    let busy2 = store
        .read(|db| db.koi_has_in_progress_todo("koi-alpha"))
        .await
        .expect("query");
    assert!(busy2, "an in_progress row makes the Koi board-busy");

    let done_id = todo.id.clone();
    store
        .write(move |db| db.complete_koi_todo(&done_id, None))
        .await
        .expect("complete");
    let busy3 = store
        .read(|db| db.koi_has_in_progress_todo("koi-alpha"))
        .await
        .expect("query");
    assert!(!busy3, "a completed row no longer keeps the Koi board-busy");
}

#[tokio::test]
async fn finished_turn_restores_koi_to_idle_when_board_is_clear() {
    use piscis_kernel::pool::coordinator::{execute_todo_turn, ExecuteTodoArgs};

    let store = build_store();
    let sink = make_sink();
    let pool_id = create_test_pool(&store, &sink).await;

    let pool_for_todo = pool_id.clone();
    let todo = store
        .write(move |db| {
            db.create_koi_todo(
                "koi-alpha",
                "solo task",
                "",
                "medium",
                "piscis",
                Some(&pool_for_todo),
                "koi",
                None,
                0,
            )
        })
        .await
        .expect("create todo");

    let subagent = Arc::new(StubSubagentRuntime::always_complete("done"))
        as Arc<dyn piscis_core::host::SubagentRuntime>;
    let cfg = CoordinatorConfig::default();

    execute_todo_turn(
        &store,
        sink_arc(&sink),
        subagent,
        &cfg,
        ExecuteTodoArgs {
            koi_id: "koi-alpha".into(),
            todo_id: todo.id.clone(),
            assign_msg_id: None,
            session_id: format!("koi_task_koi-alpha_{}", &todo.id[..8.min(todo.id.len())]),
            extra_tool_profile: Vec::new(),
            extra_system_context: None,
        },
    )
    .await
    .expect("turn");

    // Board has no remaining in_progress work → board-aware restore idles it.
    let status = store
        .read(|db| {
            Ok(db
                .get_koi("koi-alpha")?
                .map(|k| k.status)
                .unwrap_or_default())
        })
        .await
        .expect("read status");
    assert_eq!(
        status, "idle",
        "Koi must return to idle after its only turn finishes"
    );
}

#[tokio::test]
async fn forced_all_mention_creates_todos_and_dispatches_each_koi() {
    let store = build_store();
    let sink = make_sink();
    let pool_id = create_test_pool(&store, &sink).await;
    let caller = piscis_caller("sess-1");
    let requests: Arc<StdMutex<Vec<KoiTurnRequest>>> = Arc::new(StdMutex::new(Vec::new()));
    let requests_cl = requests.clone();
    let subagent = Arc::new(StubSubagentRuntime::new(move |request| {
        requests_cl.lock().unwrap().push(request.clone());
        StubOutcome::Completed("done".into())
    })) as Arc<dyn piscis_core::host::SubagentRuntime>;
    let cfg = CoordinatorConfig::default();

    services::send_pool_message(
        &store,
        sink_arc(&sink),
        Some(subagent),
        &cfg,
        &caller,
        SendPoolMessageArgs {
            pool_id: pool_id.clone(),
            sender_id: "piscis".into(),
            content: "@!all split this implementation and report progress.".into(),
            reply_to_message_id: None,
        },
    )
    .await
    .expect("send_pool_message");

    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut requested_kois: Vec<String> = requests
        .lock()
        .unwrap()
        .iter()
        .map(|request| request.koi_id.clone())
        .collect();
    requested_kois.sort();
    assert_eq!(requested_kois, vec!["koi-alpha", "koi-beta"]);

    let todos = store
        .read(|db| db.list_koi_todos(None))
        .await
        .expect("list todos");
    let mut todo_owners: Vec<String> = todos.into_iter().map(|todo| todo.owner_id).collect();
    todo_owners.sort();
    assert_eq!(todo_owners, vec!["koi-alpha", "koi-beta"]);
}

#[tokio::test]
async fn delete_todo_can_batch_delete_cancelled_items_in_pool() {
    let store = build_store();
    let sink = make_sink();
    let pool_id = create_test_pool(&store, &sink).await;
    let piscis = piscis_caller("sess-1");

    let first = services::create_todo(
        &store,
        sink.as_ref(),
        &piscis,
        CreateTodoArgs {
            pool_id: pool_id.clone(),
            title: "cancel me".into(),
            description: "".into(),
            priority: "".into(),
            timeout_secs: 0,
            depends_on: None,
        },
    )
    .await
    .expect("create first");
    let first_id = first["todo"]["id"].as_str().unwrap().to_string();
    services::cancel_todo(&store, sink.as_ref(), &piscis, &first_id, "done elsewhere")
        .await
        .expect("cancel first");

    services::create_todo(
        &store,
        sink.as_ref(),
        &piscis,
        CreateTodoArgs {
            pool_id: pool_id.clone(),
            title: "keep me".into(),
            description: "".into(),
            priority: "".into(),
            timeout_secs: 0,
            depends_on: None,
        },
    )
    .await
    .expect("create second");

    let value = services::delete_todo(
        &store,
        sink.as_ref(),
        &piscis,
        DeleteTodoArgs {
            todo_id: None,
            pool_id: Some(pool_id.clone()),
            status: Some("cancelled".into()),
            owner_id: None,
        },
    )
    .await
    .expect("delete_todo");
    assert_eq!(value["deleted_count"], 1);

    let todos = store
        .read(|db| {
            let all = db.list_koi_todos(None)?;
            Ok::<_, anyhow::Error>(
                all.into_iter()
                    .filter(|todo| todo.pool_session_id.as_deref() == Some(pool_id.as_str()))
                    .collect::<Vec<_>>(),
            )
        })
        .await
        .expect("remaining todos");
    assert_eq!(todos.len(), 1);
    assert_eq!(todos[0].title, "keep me");
}

// ─── pool membership ────────────────────────────────────────────────────

#[tokio::test]
async fn add_list_remove_member_round_trip() {
    let store = build_store();
    let sink = make_sink();
    let pool_id = create_test_pool(&store, &sink).await;
    let caller = piscis_caller("sess-1");

    // Fresh pool starts with no members.
    let listed = services::list_members(&store, &caller, &pool_id)
        .await
        .expect("list_members");
    assert_eq!(listed["members"].as_array().unwrap().len(), 0);

    services::add_member(&store, sink.as_ref(), &caller, &pool_id, "koi-alpha")
        .await
        .expect("add alpha");
    services::add_member(&store, sink.as_ref(), &caller, &pool_id, "koi-beta")
        .await
        .expect("add beta");

    let listed = services::list_members(&store, &caller, &pool_id)
        .await
        .expect("list_members");
    assert_eq!(listed["members"].as_array().unwrap().len(), 2);

    // Adding a member emits PoolUpdated so the UI can refresh participants.
    assert!(
        sink.drain_kinds().contains(&"pool_updated"),
        "add_member must emit PoolUpdated"
    );

    services::remove_member(&store, sink.as_ref(), &caller, &pool_id, "koi-beta")
        .await
        .expect("remove beta");
    let listed = services::list_members(&store, &caller, &pool_id)
        .await
        .expect("list_members");
    let names: Vec<&str> = listed["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["koi-alpha"]);
}

#[tokio::test]
async fn add_member_rejects_duplicate() {
    let store = build_store();
    let sink = make_sink();
    let pool_id = create_test_pool(&store, &sink).await;
    let caller = piscis_caller("sess-1");

    services::add_member(&store, sink.as_ref(), &caller, &pool_id, "koi-alpha")
        .await
        .expect("add alpha");
    let err = services::add_member(&store, sink.as_ref(), &caller, &pool_id, "koi-alpha")
        .await
        .expect_err("duplicate add must fail");
    assert!(
        err.to_string().contains("already a member"),
        "unexpected duplicate error: {}",
        err
    );
}

#[tokio::test]
async fn assign_koi_rejects_non_member() {
    let store = build_store();
    let sink = make_sink();
    let pool_id = create_test_pool(&store, &sink).await;
    let caller = piscis_caller("sess-1");
    let cfg = CoordinatorConfig::default();

    let err = services::assign_koi(
        &store,
        sink_arc(&sink),
        None,
        &cfg,
        &caller,
        AssignKoiArgs {
            pool_id: pool_id.clone(),
            koi_id: "koi-alpha".into(),
            task: "do work without joining".into(),
            priority: "medium".into(),
            timeout_secs: 0,
            context: None,
            depends_on: None,
        },
    )
    .await
    .expect_err("assign_koi must reject a non-member Koi");
    assert!(
        err.to_string().contains("not a member"),
        "unexpected non-member error: {}",
        err
    );

    // No todo should have been created for the rejected assignment.
    let todos = store
        .read(|db| db.list_koi_todos(Some("koi-alpha")))
        .await
        .expect("list todos");
    assert!(
        todos.is_empty(),
        "rejected assign_koi must not create a todo"
    );
}

#[tokio::test]
async fn remove_member_blocked_while_active_todo_exists() {
    let store = build_store();
    let sink = make_sink();
    let pool_id = create_test_pool(&store, &sink).await;
    let caller = piscis_caller("sess-1");

    services::add_member(&store, sink.as_ref(), &caller, &pool_id, "koi-alpha")
        .await
        .expect("add alpha");

    // Give Alpha an active todo in the pool.
    let pid = pool_id.clone();
    store
        .write(move |db| {
            db.create_koi_todo(
                "koi-alpha",
                "live work",
                "",
                "medium",
                "piscis",
                Some(&pid),
                "koi",
                None,
                0,
            )
        })
        .await
        .expect("seed active todo");

    let err = services::remove_member(&store, sink.as_ref(), &caller, &pool_id, "koi-alpha")
        .await
        .expect_err("remove must fail with active todos");
    assert!(
        err.to_string().contains("active todo"),
        "unexpected remove error: {}",
        err
    );
}

#[tokio::test]
async fn backfill_seeds_members_from_existing_todos() {
    let store = build_store();
    let sink = make_sink();
    let pool_id = create_test_pool(&store, &sink).await;

    // Simulate a legacy pool: Alpha already owns a todo but was never
    // explicitly added as a member.
    let pid = pool_id.clone();
    store
        .write(move |db| {
            db.create_koi_todo(
                "koi-alpha",
                "legacy task",
                "",
                "medium",
                "piscis",
                Some(&pid),
                "koi",
                None,
                0,
            )
        })
        .await
        .expect("seed legacy todo");

    // Run the one-shot backfill explicitly (startup runs it once).
    store
        .write(|db| db.backfill_pool_members_from_todos())
        .await
        .expect("backfill");

    let pid = pool_id.clone();
    let member_ids = store
        .read(move |db| db.list_pool_member_ids(&pid))
        .await
        .expect("member ids");
    assert_eq!(member_ids, vec!["koi-alpha".to_string()]);
}
