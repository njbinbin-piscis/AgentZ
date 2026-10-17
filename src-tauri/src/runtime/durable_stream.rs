//! Captured host Run/Attempt writer. Commit returned events before publishing them.
use agentz_task_state::{approval::Binding, events::Event, run::RunStore};
use serde_json::Value;
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

pub struct DurableStreamWriter {
    store: RunStore,
    binding: Binding,
}

impl DurableStreamWriter {
    pub fn open(project: &Path, binding: &Binding) -> Result<Self, String> {
        binding.validate()?;
        if binding.project_id != agentz_task_state::canonical_root(project)? {
            return Err("durable stream project binding mismatch".into());
        }
        if !project
            .join(".agentz/runs.db")
            .try_exists()
            .map_err(|e| format!("cannot inspect durable stream Run database: {e}"))?
        {
            return Err("durable stream requires an existing Run database".into());
        }
        let store = RunStore::open_existing_readwrite(project)?;
        let run = store
            .load(&binding.run_id)?
            .ok_or("durable stream Run not found")?;
        if run.task_id != binding.task_id
            || run.environment_id != binding.environment_id
            || run.attempt_id != binding.attempt_id
        {
            return Err("durable stream Run/Attempt binding mismatch".into());
        }
        if run.execution != agentz_task_state::run::Execution::Running
            || run.delivery != agentz_task_state::run::Delivery::NotReady
        {
            return Err("durable stream requires a Running undelivered Run".into());
        }
        Ok(Self {
            store,
            binding: binding.clone(),
        })
    }

    /// Each received event gets one ID and one append attempt. No automatic retry.
    /// An error is not durable success and must not be published with a cursor.
    pub fn record(&mut self, payload: &Value) -> Result<Event, String> {
        let event_id = uuid::Uuid::new_v4().to_string();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "invalid durable stream clock")?
            .as_millis();
        let now = i64::try_from(now).map_err(|_| "durable stream clock overflow")?;
        self.store
            .append_agent_event(&self.binding, &event_id, payload, now)
    }

    /// The publication callback runs only after the SQLite append commits.
    pub fn record_before_publish<F>(&mut self, payload: &Value, publish: F) -> Result<Event, String>
    where
        F: FnOnce(&Value),
    {
        let event = self.record(payload)?;
        publish(payload);
        Ok(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentz_task_state::run::Execution;
    struct Directory(std::path::PathBuf);
    impl Directory {
        fn new() -> Self {
            let p = std::env::temp_dir()
                .join(format!("agentz-durable-stream-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
        fn binding(&self) -> Binding {
            Binding {
                project_id: agentz_task_state::canonical_root(&self.0).unwrap(),
                task_id: "task".into(),
                environment_id: format!("direct:{}", "a".repeat(64)),
                binding_version: "a".repeat(64),
                run_id: "run".into(),
                attempt_id: "attempt".into(),
                capability_version: "fixture".into(),
            }
        }
        fn running(&self, b: &Binding) -> RunStore {
            let mut store = RunStore::open(&self.0).unwrap();
            store.create_direct(b, 1).unwrap();
            store
                .transition(
                    &b.run_id,
                    &b.attempt_id,
                    Execution::Queued,
                    Execution::Running,
                    2,
                )
                .unwrap();
            store
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            assert!(self.0.starts_with(std::env::temp_dir()));
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn persisted_event_reopens_with_same_identity_payload_and_cursor() {
        let d = Directory::new();
        let b = d.binding();
        drop(d.running(&b));
        let payload = serde_json::json!({"type":"text_delta","delta":"hello"});
        let mut writer = DurableStreamWriter::open(&d.0, &b).unwrap();
        let event = writer.record(&payload).unwrap();
        assert_eq!(event.payload, payload);
        assert_eq!(event.cursor.stream.run_id, b.run_id);
        assert_eq!(event.cursor.stream.attempt_id, b.attempt_id);
        assert!(uuid::Uuid::parse_str(event.event_id.strip_prefix("agent.").unwrap()).is_ok());
        drop(writer);
        let mut reader = RunStore::open_existing_readonly(&d.0).unwrap();
        let events = reader
            .replay_events(&b.run_id, &b.attempt_id, None, 20)
            .unwrap();
        assert_eq!(events.last(), Some(&event));
        assert!(reader
            .replay_events(&b.run_id, &b.attempt_id, Some(&event.cursor), 20)
            .unwrap()
            .is_empty());
    }
    #[test]
    fn missing_database_is_not_created_and_invalid_bindings_reject() {
        let d = Directory::new();
        let b = d.binding();
        assert!(DurableStreamWriter::open(&d.0, &b).is_err());
        assert!(!d.0.join(".agentz").exists());
        drop(d.running(&b));
        for field in ["task", "environment", "attempt"] {
            let mut wrong = b.clone();
            match field {
                "task" => wrong.task_id = "other".into(),
                "environment" => wrong.environment_id = "other".into(),
                _ => wrong.attempt_id = "other".into(),
            };
            assert!(DurableStreamWriter::open(&d.0, &wrong).is_err());
        }
        let queued = Directory::new();
        let queued_binding = queued.binding();
        let mut store = RunStore::open(&queued.0).unwrap();
        store.create_direct(&queued_binding, 1).unwrap();
        assert!(DurableStreamWriter::open(&queued.0, &queued_binding).is_err());
        store
            .transition("run", "attempt", Execution::Queued, Execution::Running, 2)
            .unwrap();
        store
            .transition("run", "attempt", Execution::Running, Execution::Failed, 3)
            .unwrap();
        assert!(DurableStreamWriter::open(&queued.0, &queued_binding).is_err());
    }
    #[test]
    fn oversized_terminal_and_stale_events_fail_without_append() {
        let d = Directory::new();
        let b = d.binding();
        let mut store = d.running(&b);
        let mut writer = DurableStreamWriter::open(&d.0, &b).unwrap();
        let before = store
            .replay_events(&b.run_id, &b.attempt_id, None, 20)
            .unwrap();
        assert!(writer
            .record(&serde_json::json!({"delta":"x".repeat(1024*1024+1)}))
            .is_err());
        assert_eq!(
            store
                .replay_events(&b.run_id, &b.attempt_id, None, 20)
                .unwrap(),
            before
        );
        store
            .transition(
                &b.run_id,
                &b.attempt_id,
                Execution::Running,
                Execution::Failed,
                3,
            )
            .unwrap();
        assert!(writer.record(&serde_json::json!({"type":"done"})).is_err());
        store.retry(&b.run_id, &b.attempt_id, "retry", 4).unwrap();
        store
            .transition(&b.run_id, "retry", Execution::Queued, Execution::Running, 5)
            .unwrap();
        assert!(writer
            .record(&serde_json::json!({"type":"text_delta","delta":"late"}))
            .is_err());
    }

    #[test]
    fn callback_observes_committed_event_and_failed_append_never_publishes() {
        let d = Directory::new();
        let b = d.binding();
        let mut store = d.running(&b);
        let mut writer = DurableStreamWriter::open(&d.0, &b).unwrap();
        let payload = serde_json::json!({"type":"text_delta","delta":"committed"});
        let observed = std::cell::RefCell::new(None);
        let returned = writer
            .record_before_publish(&payload, |published| {
                assert_eq!(published, &payload);
                // A separate connection sees the committed row before publication.
                let mut reader = RunStore::open_existing_readonly(&d.0).unwrap();
                let events = reader
                    .replay_events(&b.run_id, &b.attempt_id, None, 20)
                    .unwrap();
                let event = events.last().unwrap().clone();
                assert_eq!(&event.payload, published);
                *observed.borrow_mut() = Some(event);
            })
            .unwrap();
        assert_eq!(observed.into_inner(), Some(returned));
        let called = std::cell::Cell::new(0);
        assert!(writer
            .record_before_publish(
                &serde_json::json!({"delta":"x".repeat(1024*1024+1)}),
                |_| called.set(called.get() + 1),
            )
            .is_err());
        store
            .transition(
                &b.run_id,
                &b.attempt_id,
                Execution::Running,
                Execution::Failed,
                i64::try_from(
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis(),
                )
                .unwrap(),
            )
            .unwrap();
        assert!(writer
            .record_before_publish(&serde_json::json!({"type":"done"}), |_| called
                .set(called.get() + 1),)
            .is_err());
        assert_eq!(called.get(), 0);
    }

    /// Opt-in local measurement, not a latency SLA or a desktop/model acceptance test.
    #[test]
    #[ignore = "real SQLite per-event commits; run explicitly with --ignored --nocapture"]
    fn measure_thousand_delta_commits_and_reopened_paged_replay() {
        let d = Directory::new();
        let b = d.binding();
        drop(d.running(&b));
        let mut baseline_reader = RunStore::open_existing_readonly(&d.0).unwrap();
        let baseline = baseline_reader
            .replay_events(&b.run_id, &b.attempt_id, None, 200)
            .unwrap();
        drop(baseline_reader);
        let initial_bytes = std::fs::metadata(d.0.join(".agentz/runs.db"))
            .unwrap()
            .len();
        let mut expected = baseline;
        let mut writer = DurableStreamWriter::open(&d.0, &b).unwrap();
        let write_started = std::time::Instant::now();
        for index in 0..1000 {
            if index % 50 == 0 {
                expected.push(
                    writer
                        .record(&serde_json::json!({
                            "type":"tool_start", "id":format!("tool-{index}"),
                            "name":"read_file", "input":{"path":"fixture.txt"}
                        }))
                        .unwrap(),
                );
            }
            expected.push(
                writer
                    .record(&serde_json::json!({
                        "type":"text_delta", "delta":format!("delta-{index} ")
                    }))
                    .unwrap(),
            );
            if index % 50 == 0 {
                expected.push(
                    writer
                        .record(&serde_json::json!({
                            "type":"tool_end", "id":format!("tool-{index}"),
                            "name":"read_file", "result":"fixture result", "is_error":false
                        }))
                        .unwrap(),
                );
            }
        }
        let write_elapsed = write_started.elapsed();
        drop(writer);
        let replay_started = std::time::Instant::now();
        let mut reader = RunStore::open_existing_readonly(&d.0).unwrap();
        let mut after = None;
        let mut replayed = Vec::new();
        let mut pages = 0;
        loop {
            let page = reader
                .replay_events(&b.run_id, &b.attempt_id, after.as_ref(), 200)
                .unwrap();
            if page.is_empty() {
                break;
            }
            pages += 1;
            assert!(page.len() <= 200);
            for event in &page {
                assert_eq!(event.cursor.sequence, replayed.len() as i64 + 1);
                // Count immediately so every cursor is checked, including page boundaries.
                replayed.push(event.clone());
            }
            after = page.last().map(|event| event.cursor.clone());
        }
        let replay_elapsed = replay_started.elapsed();
        assert_eq!(replayed, expected);
        assert_eq!(
            replayed
                .iter()
                .filter(|event| event.kind == "agent.event")
                .count(),
            1040
        );
        assert!(pages > 1);
        drop(reader);
        let final_bytes = std::fs::metadata(d.0.join(".agentz/runs.db"))
            .unwrap()
            .len();
        println!(
            "durable_stream_measurement agent_events=1040 total_events={} pages={} page_limit=200 write_elapsed_ms={:.3} reopened_replay_elapsed_ms={:.3} db_initial_bytes={} db_final_bytes={} os={} arch={}",
            replayed.len(), pages, write_elapsed.as_secs_f64() * 1000.0,
            replay_elapsed.as_secs_f64() * 1000.0, initial_bytes, final_bytes,
            std::env::consts::OS, std::env::consts::ARCH,
        );
    }
}
