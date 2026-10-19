use super::approval::{ApprovalStore, Binding, Intent, Status};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, RwLock},
};
use tokio::sync::{oneshot, Mutex};

struct Pending {
    path: PathBuf,
    intent: Intent,
    response: oneshot::Sender<bool>,
    preview: Value,
}
/// One process authority, shared by all windows/turns. Paths and identity come from the host.
/// UI can decide only an existing immutable intent; it cannot issue or expand one.
#[derive(Clone)]
pub struct ApprovalBroker {
    boot_id: String,
    pending: Arc<Mutex<HashMap<String, Pending>>>,
    always_allowed: Arc<RwLock<HashSet<String>>>,
}
impl ApprovalBroker {
    pub fn new(boot_id: String) -> Self {
        Self {
            boot_id,
            pending: Arc::new(Mutex::new(HashMap::new())),
            always_allowed: Arc::new(RwLock::new(HashSet::new())),
        }
    }
    pub fn boot_id(&self) -> &str {
        &self.boot_id
    }
    pub fn grant_always_allow(&self, action: &str) {
        let normalized = action.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return;
        }
        if let Ok(mut guard) = self.always_allowed.write() {
            if matches!(
                normalized.as_str(),
                "shell" | "bash" | "powershell" | "powershell_query" | "code_run"
            ) {
                guard.insert("shell".into());
                guard.insert("code_run".into());
            }
            guard.insert(normalized);
        }
    }
    pub fn set_always_allowed(&self, actions: impl IntoIterator<Item = String>) {
        if let Ok(mut guard) = self.always_allowed.write() {
            guard.clear();
            for action in actions {
                let normalized = action.trim().to_ascii_lowercase();
                if !normalized.is_empty() {
                    if matches!(
                        normalized.as_str(),
                        "shell" | "bash" | "powershell" | "powershell_query" | "code_run"
                    ) {
                        guard.insert("shell".into());
                        guard.insert("code_run".into());
                    }
                    guard.insert(normalized);
                }
            }
        }
    }
    pub fn is_always_allowed(&self, action: &str) -> bool {
        let normalized = action.trim().to_ascii_lowercase();
        let Ok(guard) = self.always_allowed.read() else {
            return false;
        };
        if guard.contains("*") || guard.contains(&normalized) {
            return true;
        }
        if matches!(
            normalized.as_str(),
            "shell" | "bash" | "powershell" | "powershell_query" | "code_run"
        ) && guard.contains("shell")
        {
            return true;
        }
        false
    }
    pub async fn pending_action(&self, id: &str) -> Option<String> {
        self.pending
            .lock()
            .await
            .get(id)
            .filter(|pending| !pending.response.is_closed())
            .map(|pending| pending.intent.action.clone())
    }
    pub async fn request(
        &self,
        path: PathBuf,
        id: String,
        binding: Binding,
        action: String,
        resource: Value,
        parameters: Value,
        now_ms: i64,
        ttl_ms: i64,
    ) -> Result<(Intent, oneshot::Receiver<bool>), String> {
        if !id.starts_with("approval:") {
            return Err("Durable approvals require the host approval namespace".into());
        }
        let preview_input = parameters.clone();
        let preview_resource = resource.clone();
        let preview_action = action.clone();
        let description = if action == "read_lints" {
            "Read diagnostics for these exact files. This may start a language server or run cargo check / tsc, including project build scripts, and create build outputs. Approve this operation once."
        } else if action == "lsp" {
            "Run this exact semantic code operation. This may start a language server, load project configuration, run project build scripts, and read dependency sources outside the workspace. Approve this operation once."
        } else {
            "Allow this exact action once"
        };
        let boot = self.boot_id.clone();
        let store_path = path.clone();
        let intent = tokio::task::spawn_blocking(move || {
            ApprovalStore::open(&store_path, &boot, now_ms)?.issue(
                &id,
                binding,
                &action,
                &resource,
                &parameters,
                now_ms,
                ttl_ms,
            )
        })
        .await
        .map_err(|e| format!("approval issuer worker: {e}"))??;
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(
            intent.request_id.clone(),
            Pending {
                path,
                intent: intent.clone(),
                response: tx,
                preview: serde_json::json!({
                    "requestId":intent.request_id, "toolName":preview_action,
                    "toolInput":preview_input, "description":description,
                    "approval":{"digest":intent.digest,"taskId":intent.binding.task_id,
                        "environmentId":intent.binding.environment_id,"expiresMs":intent.expires_ms,"resource":preview_resource}
                }),
            },
        );
        Ok((intent, rx))
    }

    pub async fn preview(&self, id: &str) -> Result<Value, String> {
        self.pending
            .lock()
            .await
            .get(id)
            .filter(|pending| !pending.response.is_closed())
            .map(|pending| pending.preview.clone())
            .ok_or_else(|| "Approval preview is no longer active".into())
    }

    pub async fn respond(
        &self,
        id: &str,
        expected_digest: &str,
        approved: bool,
        now_ms: i64,
    ) -> Result<(), String> {
        let pending = {
            let mut entries = self.pending.lock().await;
            let pending = entries
                .get(id)
                .ok_or("Approval request not found or no longer active")?;
            if pending.intent.digest != expected_digest {
                return Err("Approval preview changed; refresh the original task card".into());
            }
            entries.remove(id).ok_or("Approval request disappeared")?
        };
        let boot = self.boot_id.clone();
        let intent = pending.intent.clone();
        let path = pending.path.clone();
        // The worker owns the response, so cancellation of an IPC waiter cannot leave a
        // persisted decision without completing delivery or recording its failure.
        tokio::task::spawn_blocking(move || {
            let mut store = ApprovalStore::open_existing(&path, &boot, now_ms)?;
            store.decide(&intent, approved, now_ms)?;
            if pending.response.send(approved).is_err() {
                if approved {
                    store.revoke(&intent, now_ms)?;
                }
                return Err("Approval receiver closed; decision was not delivered".into());
            }
            Ok(())
        })
        .await
        .map_err(|e| format!("approval decision worker: {e}"))?
    }

    /// Attempt cancellation removes pending transport and invalidates unconsumed consent.
    /// A claimed operation cannot be retracted here; its executor must cancel/reconcile.
    pub async fn cancel(&self, path: PathBuf, intent: Intent, now_ms: i64) -> Result<(), String> {
        self.pending.lock().await.remove(&intent.request_id);
        let boot = self.boot_id.clone();
        tokio::task::spawn_blocking(move || {
            let mut store = ApprovalStore::open_existing(&path, &boot, now_ms)?;
            let status = store
                .get(&intent.request_id)?
                .ok_or("Approval not found")?
                .1;
            match status {
                Status::Pending | Status::Approved => store.revoke(&intent, now_ms),
                Status::Claimed => Err(
                    "Approval already claimed; cancel the executor and reconcile its result".into(),
                ),
                _ => Ok(()),
            }
        })
        .await
        .map_err(|e| format!("approval cancellation worker: {e}"))?
    }
}

/// Called using Tauri-injected webview identity and its actual origin, never renderer claims.
/// This is an IPC boundary, not proof that OS input cannot click the human UI.
pub fn validate_surface(label: &str, origin: &str, development: bool) -> Result<(), String> {
    let origin_allowed = matches!(
        origin,
        "tauri://localhost" | "http://tauri.localhost" | "https://tauri.localhost"
    ) || (development && origin == "http://localhost:5273");
    if label != "main" || !origin_allowed {
        return Err("Approval decisions require the trusted main application surface".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn binding() -> Binding {
        Binding {
            project_id: "project-a".into(),
            task_id: "task-a".into(),
            environment_id: "env-a".into(),
            binding_version: "v1".into(),
            run_id: "run-a".into(),
            attempt_id: "attempt-a".into(),
            capability_version: "cap1".into(),
        }
    }
    fn path() -> PathBuf {
        std::env::temp_dir()
            .join(format!("agentz-broker-{}", uuid::Uuid::new_v4()))
            .join("approvals.db")
    }
    async fn request(
        broker: &ApprovalBroker,
        path: PathBuf,
        id: &str,
    ) -> (Intent, oneshot::Receiver<bool>) {
        broker
            .request(
                path,
                id.into(),
                binding(),
                "send".into(),
                serde_json::json!({"account":"a"}),
                serde_json::json!({"message":"A"}),
                100,
                1000,
            )
            .await
            .unwrap()
    }
    #[tokio::test]
    async fn broker_decision_is_bound_and_does_not_claim_or_execute() {
        let path = path();
        let broker = ApprovalBroker::new("boot-a".into());
        let (intent, rx) = request(&broker, path.clone(), "approval:request-a").await;
        assert!(broker
            .respond("approval:request-a", "different-preview", true, 101)
            .await
            .is_err());
        let clone = broker.clone();
        clone
            .respond("approval:request-a", &intent.digest, true, 102)
            .await
            .unwrap();
        assert!(rx.await.unwrap());
        let store = ApprovalStore::open(&path, "boot-a", 103).unwrap();
        assert_eq!(
            store.get("approval:request-a").unwrap().unwrap().1,
            Status::Approved
        );
        assert!(broker
            .respond("approval:request-a", &intent.digest, true, 104)
            .await
            .is_err());
        assert_eq!(store.history("approval:request-a").unwrap().len(), 2);
    }
    #[tokio::test]
    async fn diagnostic_preview_explains_build_effects_from_host_not_parameters() {
        let broker = ApprovalBroker::new("boot-a".into());
        let (_, _rx) = broker
            .request(
                path(),
                "approval:diagnostics".into(),
                binding(),
                "read_lints".into(),
                serde_json::json!({"workspace":"a"}),
                serde_json::json!({"paths":["source.rs"],"description":"harmless read only"}),
                100,
                1000,
            )
            .await
            .unwrap();
        let preview = broker.preview("approval:diagnostics").await.unwrap();
        let description = preview["description"].as_str().unwrap();
        assert!(description.contains("cargo check / tsc"));
        assert!(description.contains("project build scripts"));
        assert!(description.contains("build outputs"));
        assert!(!description.contains("harmless"));
        assert_eq!(
            preview["toolInput"]["paths"],
            serde_json::json!(["source.rs"])
        );
    }
    #[tokio::test]
    async fn lsp_preview_explains_server_effects_from_host() {
        let broker = ApprovalBroker::new("boot-a".into());
        let (_, _rx) = broker
            .request(
                path(),
                "approval:lsp".into(),
                binding(),
                "lsp".into(),
                serde_json::json!({"workspace":"a"}),
                serde_json::json!({"file":"source.rs", "description":"harmless"}),
                100,
                1000,
            )
            .await
            .unwrap();
        let preview = broker.preview("approval:lsp").await.unwrap();
        let description = preview["description"].as_str().unwrap();
        assert!(description.contains("language server"));
        assert!(description.contains("project build scripts"));
        assert!(description.contains("outside the workspace"));
        assert!(!description.contains("harmless"));
    }
    #[tokio::test]
    async fn closed_receiver_revokes_approval_and_cancellation_never_delivers_allow() {
        let path = path();
        let broker = ApprovalBroker::new("boot-a".into());
        let (intent, rx) = request(&broker, path.clone(), "approval:closed").await;
        drop(rx);
        assert!(broker
            .preview("approval:closed")
            .await
            .unwrap_err()
            .contains("no longer active"));
        assert!(broker
            .respond("approval:closed", &intent.digest, true, 101)
            .await
            .unwrap_err()
            .contains("not delivered"));
        assert_eq!(
            ApprovalStore::open(&path, "boot-a", 102)
                .unwrap()
                .get("approval:closed")
                .unwrap()
                .unwrap()
                .1,
            Status::Revoked
        );
        let (intent, rx) = request(&broker, path.clone(), "approval:cancel").await;
        broker
            .cancel(path.clone(), intent.clone(), 101)
            .await
            .unwrap();
        assert!(rx.await.is_err());
        assert!(broker
            .respond("approval:cancel", &intent.digest, true, 102)
            .await
            .is_err());
        assert_eq!(
            ApprovalStore::open(&path, "boot-a", 102)
                .unwrap()
                .get("approval:cancel")
                .unwrap()
                .unwrap()
                .1,
            Status::Revoked
        );
    }
    #[tokio::test]
    async fn expired_and_previous_boot_requests_cannot_be_delivered() {
        let path = path();
        let broker = ApprovalBroker::new("boot-a".into());
        let (intent, rx) = request(&broker, path.clone(), "approval:expired").await;
        assert!(broker
            .respond("approval:expired", &intent.digest, true, 1100)
            .await
            .is_err());
        assert!(rx.await.is_err());
        let (intent, rx) = request(&broker, path.clone(), "approval:restart").await;
        let restored = ApprovalStore::open(&path, "boot-b", 102).unwrap();
        assert_eq!(
            restored.get("approval:restart").unwrap().unwrap().1,
            Status::Expired
        );
        drop(restored);
        assert!(broker
            .respond("approval:restart", &intent.digest, true, 103)
            .await
            .is_err());
        assert!(rx.await.is_err());
    }
    #[test]
    fn only_main_trusted_origins_can_submit_human_decisions() {
        assert!(validate_surface("main", "http://tauri.localhost", false).is_ok());
        assert!(validate_surface("main", "http://localhost:5273", true).is_ok());
        for (label, origin, dev) in [
            ("extension-webview-1", "http://tauri.localhost", false),
            ("main", "https://example.test", false),
            ("main", "http://localhost:5273", false),
            ("main", "http://tauri.localhost.evil.test", true),
        ] {
            assert!(validate_surface(label, origin, dev).is_err());
        }
    }
    #[tokio::test]
    async fn a_durable_intent_cannot_accidentally_use_the_legacy_bool_route() {
        let path = path();
        let broker = ApprovalBroker::new("boot-a".into());
        let result = broker
            .request(
                path.clone(),
                "legacy-id".into(),
                binding(),
                "send".into(),
                serde_json::json!({"account":"a"}),
                serde_json::json!({"message":"A"}),
                100,
                1000,
            )
            .await;
        assert!(result.is_err());
        assert!(!path.exists());
        assert!(broker
            .respond("legacy-id", "missing", true, 101)
            .await
            .is_err());
    }
}
