//! Opt-in paid provider baseline. Synthetic fixtures only, no desktop/host acceptance.
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use piscis_kernel::{
    agent::{
        harness::LayeredBudget,
        loop_::{confirm_flags_handle, AgentLoop, DefaultCompaction},
        messages::AgentEvent,
        tool::{Tool, ToolContext, ToolRegistry, ToolResult, ToolSettings},
    },
    llm::{
        error_class::classify_error, openai::OpenAiClient, ClientOptions, LlmChunk, LlmClient,
        LlmMessage, LlmRequest, LlmResponse, MessageContent,
    },
    policy::PolicyGate,
};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc, Mutex},
    time::{Duration, Instant},
};

// BASELINE_SPEC_BEGIN
const SPEC_JSON: &str = r#"[
{"id":"J1-01","source":"export const add=(a,b)=>a-b;","prompt":"Read source.txt; fix add to add operands, preserve compact style. Write out.txt.","required":["a+b"],"forbidden":["a-b"]},
{"id":"J1-02","source":"function eligible(age){return age>18;}","prompt":"Read source.txt; fix eligibility to include age18, preserve function name. Write out.txt.","required":["age>=18","eligible"],"forbidden":["age>18"]},
{"id":"J1-03","source":"export const items=['red','blue'];","prompt":"Read source.txt; extend items with green without removing existing entries. Write out.txt.","required":["red","blue","green"],"forbidden":[]},
{"id":"J1-04","source":"{\"enabled\":false,\"timeout\":30}","prompt":"Read source.txt; write valid JSON out.txt with enabled true and timeout unchanged.","required":["\"enabled\":true","\"timeout\":30"],"forbidden":["\"enabled\":false"]},
{"id":"J2-01","source":"module-a.ts: entry=start; module-b.ts: entry=finish;","prompt":"Read source.txt; write out.txt research identifying both module entry points and cite both exact file names.","required":["module-a.ts","start","module-b.ts","finish"],"forbidden":[]},
{"id":"J2-02","source":"policy.md: retries=3; cache.md: ttl=60 seconds;","prompt":"Read source.txt; write out.txt two-setting summary with file citations, values and TTL unit.","required":["policy.md","3","cache.md","60","seconds"],"forbidden":[]},
{"id":"J2-03","source":"release.md: tested=Windows; macOS=not tested; Linux=not tested;","prompt":"Read source.txt; write out.txt explicit platform support evidence. Do not claim untested platforms tested.","required":["Windows","macOS","Linux","not tested"],"forbidden":["all platforms tested"]},
{"id":"J2-04","source":"trace.md: observed=timeout; cause=unknown;","prompt":"Read source.txt; write out.txt separating observed timeout from unknown cause, cite trace.md. Do not invent network failure.","required":["timeout","unknown","trace.md"],"forbidden":["network failure"]},
{"id":"J3-01","source":"meeting: owner=Ada; deadline=Friday; deliverable=draft;","prompt":"Read source.txt; write Markdown checklist out.txt containing owner, deadline and deliverable.","required":["Ada","Friday","draft","- [ ]"],"forbidden":[]},
{"id":"J3-02","source":"title=Local report; scope=synthetic; cost=unknown;","prompt":"Read source.txt; write Markdown out.txt with title Local report, synthetic scope and explicit unknown cost, never zero cost.","required":["Local report","synthetic","unknown"],"forbidden":["cost=0","cost: 0"]},
{"id":"J3-03","source":"HUMAN NOTE: preserve this line.\nDraft: add a Risks section.","prompt":"Read source.txt; produce out.txt adding a Markdown Risks section while retaining exact HUMAN NOTE line.","required":["HUMAN NOTE: preserve this line.","Risks"],"forbidden":[]},
{"id":"J3-04","source":"Goal: offline guide.\nCompleted: overview.\nOpen: backup instructions.","prompt":"Read source.txt; continue the existing guide into out.txt preserving goal/overview and adding backup instructions. Do not replace goal with a new task.","required":["offline","overview","backup"],"forbidden":[]}
]"#;
// BASELINE_SPEC_END

#[derive(Default)]
struct Stats {
    calls: usize,
    input_bytes: usize,
    input_tokens: u64,
    output_tokens: u64,
    unknown_usage: usize,
    stop: Option<String>,
    last_error: Option<String>,
    reads: usize,
    writes: usize,
}
struct BoundedClient {
    inner: OpenAiClient,
    stats: Arc<Mutex<Stats>>,
}
#[async_trait]
impl LlmClient for BoundedClient {
    async fn stream(&self, _: LlmRequest, _: tokio::sync::mpsc::Sender<LlmChunk>) -> Result<()> {
        Err(anyhow!("baseline streaming disabled"))
    }
    async fn complete(&self, mut req: LlmRequest) -> Result<LlmResponse> {
        req.max_tokens = req.max_tokens.min(256);
        req.stream = false;
        let bytes = serde_json::to_vec(
            &json!({"messages":req.messages,"system":req.system,"tools":req.tools} ),
        )?
        .len();
        {
            let mut s = self.stats.lock().unwrap();
            if s.stop.is_some() {
                return Err(anyhow!("baseline stopped after fatal provider failure"));
            }
            if s.calls >= 4 || s.input_bytes + bytes > 16384 {
                s.last_error = Some("harness_budget".into());
                return Err(anyhow!("baseline logical request budget exhausted"));
            }
            s.calls += 1;
            s.input_bytes += bytes;
        }
        match self.inner.complete(req).await {
            Ok(r) => {
                let mut s = self.stats.lock().unwrap();
                if r.input_tokens == 0 || r.output_tokens == 0 {
                    s.unknown_usage += 1;
                } else {
                    s.input_tokens += u64::from(r.input_tokens);
                    s.output_tokens += u64::from(r.output_tokens);
                }
                Ok(r)
            }
            Err(e) => {
                let code = classify_error(&e.to_string()).code().to_owned();
                self.stats.lock().unwrap().unknown_usage += 1;
                self.stats.lock().unwrap().last_error = Some(code.clone());
                if matches!(code.as_str(), "auth_failed" | "quota_exceeded") {
                    self.stats.lock().unwrap().stop = Some(code.clone());
                }
                Err(anyhow!("provider failure class: {code}"))
            }
        }
    }
}
struct FixtureTool {
    root: PathBuf,
    stats: Arc<Mutex<Stats>>,
}
#[async_trait]
impl Tool for FixtureTool {
    fn name(&self) -> &str {
        "fixture"
    }
    fn description(&self) -> &str {
        "Read source.txt or write out.txt synthetic fixture only"
    }
    fn input_schema(&self) -> Value {
        json!({"type":"object","properties":{"op":{"type":"string","enum":["read","write"]},"text":{"type":"string"}},"required":["op"],"additionalProperties":false})
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult> {
        if ctx.workspace_root != self.root {
            return Err(anyhow!("fixture scope mismatch"));
        }
        match input["op"].as_str() {
            Some("read") => {
                self.stats.lock().unwrap().reads += 1;
                Ok(ToolResult::ok(std::fs::read_to_string(
                    self.root.join("source.txt"),
                )?))
            }
            Some("write") => {
                let text = input["text"]
                    .as_str()
                    .ok_or_else(|| anyhow!("fixture write requires text"))?;
                if text.len() > 4096 {
                    return Err(anyhow!("fixture artifact too large"));
                }
                std::fs::write(self.root.join("out.txt"), text)?;
                self.stats.lock().unwrap().writes += 1;
                Ok(ToolResult::ok("out.txt saved"))
            }
            _ => Err(anyhow!("invalid fixture operation")),
        }
    }
}

#[tokio::test]
#[ignore = "Paid live provider; root explicitly injects temporary environment credentials"]
async fn fixed_twelve_synthetic_tasks_live() {
    let key =
        std::env::var("AGENTZ_BASELINE_API_KEY").expect("temporary credential environment missing");
    assert!(!key.trim().is_empty(), "empty temporary credential");
    let hash = std::env::var("AGENTZ_BASELINE_SPEC_SHA256").expect("frozen spec hash missing");
    let source_hash =
        std::env::var("AGENTZ_BASELINE_SOURCE_SHA256").expect("frozen source hash missing");
    let mut validator = piscis_kernel::proc::std_command("node").args(["-e", r##"const fs=require('node:fs'),c=require('node:crypto'),s=fs.readFileSync(0,'utf8'),raw=s.match(/const SPEC_JSON: &str = r#"([\s\S]*?)"#;/)[1],h=v=>c.createHash('sha256').update(v).digest('hex');process.stdout.write(JSON.stringify({raw,spec:h(raw),source:h(s)}));"##]).env_remove("AGENTZ_BASELINE_API_KEY").stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).spawn().expect("local source identity validator missing");
    std::io::Write::write_all(
        &mut validator.stdin.take().expect("validator stdin missing"),
        include_bytes!("upgrade_live_baseline.rs"),
    )
    .expect("compiled fixture hash input failed");
    let checked = validator
        .wait_with_output()
        .expect("identity validator failed");
    assert!(
        checked.status.success(),
        "source identity validation failed"
    );
    let identity: Value =
        serde_json::from_slice(&checked.stdout).expect("invalid local identity result");
    assert_eq!(
        identity["raw"], SPEC_JSON,
        "compiled synthetic spec changed"
    );
    assert_eq!(identity["spec"], hash, "spec fingerprint mismatch");
    assert_eq!(
        identity["source"], source_hash,
        "source fingerprint mismatch"
    );
    assert!(
        hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid spec identity"
    );
    let repeats = std::env::var("AGENTZ_BASELINE_REPEATS")
        .unwrap_or_else(|_| "1".into())
        .parse::<usize>()
        .expect("invalid repeat count");
    assert!((1..=3).contains(&repeats));
    let output =
        PathBuf::from(std::env::var("AGENTZ_BASELINE_OUTPUT").expect("output destination missing"));
    let specs: Vec<Value> = serde_json::from_str(SPEC_JSON).unwrap();
    assert_eq!(specs.len(), 12);
    let workspace = tempfile::tempdir().unwrap();
    let mut records = vec![];

    'runs: for repeat in 1..=repeats {
        for spec in &specs {
            let id = spec["id"].as_str().unwrap();
            let dir = workspace.path().join(format!("{id}-{repeat}"));
            std::fs::create_dir(&dir).unwrap();
            std::fs::write(dir.join("source.txt"), spec["source"].as_str().unwrap()).unwrap();
            let stats = Arc::new(Mutex::new(Stats::default()));
            let mut registry = ToolRegistry::new();
            registry.register(Box::new(FixtureTool {
                root: dir.clone(),
                stats: stats.clone(),
            }));
            let agent=AgentLoop{client:Box::new(BoundedClient{inner:OpenAiClient::with_timeout(&key,"https://api.deepseek.com",45).with_options(ClientOptions{thinking:Some(false),temperature:Some(0.0),top_p:None}),stats:stats.clone()}),registry:Arc::new(registry),policy:Arc::new(PolicyGate::new(dir.clone())),system_prompt:"Synthetic local fixture. Use fixture tool read then write. No external resources. Stop after artifact saved.".into(),model:"deepseek-flash".into(),max_tokens:256,context_window:8192,budget:LayeredBudget::from_context_window(8192,256),fallback_models:vec![],db:None,plan_state:None,confirmation_responses:None,confirm_flags:confirm_flags_handle(false,false),vision_override:Some(false),vision_delegate:None,vision_model:String::new(),notification_rx:None,auto_compact_input_tokens_threshold:0,enable_streaming:false,hooks:None,compaction_strategy:Arc::new(DefaultCompaction),memory_plugin:None,context_manager:None,memory_retrieval_prompt:None,loop_strategy:None,same_model_transient_retries:1};
            let cancel = Arc::new(AtomicBool::new(false));
            let ctx = ToolContext {
                session_id: format!("{id}-{repeat}"),
                workspace_root: dir.clone(),
                bypass_permissions: false,
                settings: Arc::new(ToolSettings::default()),
                max_iterations: Some(4),
                memory_owner_id: "baseline-synthetic".into(),
                pool_session_id: None,
                tool_use_id: None,
                cancel: cancel.clone(),
                loop_halt: None,
            };
            let (tx, mut rx) = tokio::sync::mpsc::channel(64);
            let collector = tokio::spawn(async move {
                let mut codes = vec![];
                while let Some(event) = rx.recv().await {
                    if let AgentEvent::Error {
                        code: Some(code), ..
                    } = event
                    {
                        codes.push(code);
                    }
                }
                codes
            });
            let start = Instant::now();
            let result = tokio::time::timeout(
                Duration::from_secs(180),
                agent.run(
                    vec![LlmMessage {
                        role: "user".into(),
                        content: MessageContent::text(spec["prompt"].as_str().unwrap()),
                    }],
                    tx,
                    cancel.clone(),
                    ctx,
                ),
            )
            .await;
            let elapsed = start.elapsed().as_millis();
            let (codes, collector_failed) = match collector.await {
                Ok(codes) => (codes, false),
                Err(_) => (vec![], true),
            };
            let artifact = std::fs::read_to_string(dir.join("out.txt")).ok();
            let artifact_path = if let Some(text) = artifact.as_ref() {
                let directory = output.with_extension("artifacts");
                std::fs::create_dir_all(&directory).unwrap();
                let path = directory.join(format!("{id}-{repeat}.txt"));
                std::fs::write(&path, text).unwrap();
                Some(path.to_string_lossy().to_string())
            } else {
                None
            };
            let compact = artifact
                .as_deref()
                .unwrap_or("")
                .split_whitespace()
                .collect::<String>();
            let verified = artifact.as_ref().is_some_and(|a| {
                spec["required"].as_array().unwrap().iter().all(|v| {
                    a.contains(v.as_str().unwrap()) || compact.contains(v.as_str().unwrap())
                }) && spec["forbidden"].as_array().unwrap().iter().all(|v| {
                    !a.contains(v.as_str().unwrap()) && !compact.contains(v.as_str().unwrap())
                })
            }) && (id != "J1-04"
                || artifact
                    .as_ref()
                    .and_then(|a| serde_json::from_str::<Value>(a).ok())
                    .is_some_and(|v| v == json!({"enabled":true,"timeout":30})));
            let s = stats.lock().unwrap();
            let succeeded = matches!(&result, Ok(Ok(_)))
                && verified
                && s.reads > 0
                && s.writes > 0
                && codes.is_empty()
                && s.stop.is_none()
                && s.last_error.is_none()
                && !collector_failed;
            let status = if result.is_err() {
                "host_timeout"
            } else if collector_failed {
                "collector_failed"
            } else if let Some(code) = s.last_error.as_deref() {
                code
            } else if !codes.is_empty() || !matches!(&result, Ok(Ok(_))) {
                "kernel_failed"
            } else if !verified || s.reads == 0 || s.writes == 0 {
                "artifact_failed"
            } else {
                "passed"
            };
            records.push(json!({"task_id":id,"repeat":repeat,"success":succeeded,"status":status,"collector_failed":collector_failed,"artifact_verified":verified,"artifact_path":artifact_path,"elapsed_ms":elapsed,"logical_calls":s.calls,"request_payload_bytes":s.input_bytes,"input_tokens":if s.unknown_usage==0 && s.calls>0 {Some(s.input_tokens)}else{None},"output_tokens":if s.unknown_usage==0 && s.calls>0 {Some(s.output_tokens)}else{None},"unknown_usage_responses":s.unknown_usage,"cost":null,"manual_interventions":0,"tool_reads":s.reads,"tool_writes":s.writes,"error_codes":codes,"timeout":result.is_err()}));
            let stop = s.stop.clone();
            drop(s);
            let summary = json!({"schema_version":1,"baseline":"first-fixed-synthetic-v1","spec_sha256":hash,"fixture_source_sha256":source_hash,"model":"deepseek-flash","endpoint":"https://api.deepseek.com","thinking":false,"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"unverified":["host","GUI","native sandbox","priced settlement","cross-platform","true session continuation"],"max_logical_calls_per_sample":4,"max_output_tokens_per_call":256,"max_request_payload_bytes_per_sample":16384,"repeats":repeats,"records":records,"stopped":stop,"scope":"real kernel synthetic tools, not host/GUI/OS sandbox or priced settlement"});
            std::fs::write(&output, serde_json::to_vec_pretty(&summary).unwrap()).unwrap();
            if stop.is_some() {
                break 'runs;
            }
        }
    }
    assert!(!records.is_empty(), "no observations recorded");
}
