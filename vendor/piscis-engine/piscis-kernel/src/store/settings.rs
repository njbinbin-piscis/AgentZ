use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Configuration for an MCP (Model Context Protocol) server.
///
/// This lives in the kernel's settings module (rather than the desktop
/// `tools/mcp.rs`) so that the serialized shape of [`Settings`] has no
/// dependency on tool implementation crates. Desktop code that actually
/// runs an MCP server still lives in `src/tools/mcp.rs` and re-uses this
/// struct via `piscis_kernel::store::settings::McpServerConfig`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpServerConfig {
    /// Display name for this server
    pub name: String,
    /// "stdio" | "sse" | "http" (streamable HTTP)
    pub transport: String,
    /// For stdio: the executable command
    #[serde(default)]
    pub command: String,
    /// For stdio: command arguments
    #[serde(default)]
    pub args: Vec<String>,
    /// For sse: the HTTP base URL (e.g. "http://localhost:3000").
    /// For http (streamable HTTP): the single JSON-RPC endpoint URL.
    #[serde(default)]
    pub url: String,
    /// Extra environment variables for stdio processes
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// For sse / http transports: extra HTTP headers sent on every request
    /// (e.g. `Authorization: Bearer <token>`). Enables authenticated remote
    /// MCP servers / connectors without leaking the token into `env`.
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Whether this server is enabled
    #[serde(default = "mcp_default_true")]
    pub enabled: bool,
}

fn mcp_default_true() -> bool {
    true
}

/// A named LLM provider configuration that can be assigned to individual Koi.
/// Multiple entries allow different Koi to use different models/keys.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmProviderConfig {
    /// Unique identifier (user-chosen, e.g. "gpt4-work", "claude-pro")
    pub id: String,
    /// Human-readable display label shown in dropdowns
    pub label: String,
    /// Provider type: "anthropic" | "openai" | "deepseek" | "qwen" | "minimax" | "zhipu" | "kimi" | "custom"
    pub provider: String,
    /// Model name (e.g. "gpt-4o", "claude-opus-4-5")
    pub model: String,
    /// API key (encrypted at rest, same scheme as global keys)
    #[serde(default)]
    pub api_key: String,
    /// Custom base URL for OpenAI-compatible endpoints (only used when provider = "custom")
    #[serde(default)]
    pub base_url: String,
    /// Max output tokens; 0 means inherit from global settings
    #[serde(default)]
    pub max_tokens: u32,
    /// Context window in tokens; 0 means inherit from global settings
    #[serde(default)]
    pub context_window: u32,
    /// Sampling temperature; None = provider default.
    #[serde(default)]
    pub temperature: Option<f32>,
    /// Nucleus sampling top_p; None = provider default.
    #[serde(default)]
    pub top_p: Option<f32>,
    /// Provider thinking mode; None/false = disabled.
    #[serde(default)]
    pub thinking: Option<bool>,
    /// Per-model streaming override; None inherits global `enable_streaming`.
    #[serde(default)]
    pub stream: Option<bool>,
}

/// List price of one model, per million tokens, in `currency`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelPrice {
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub input_per_million: f64,
    #[serde(default)]
    pub output_per_million: f64,
    /// ISO 4217 code such as "USD" or "CNY"; empty means USD.
    #[serde(default)]
    pub currency: String,
}

impl Default for LlmProviderConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            provider: "anthropic".into(),
            model: "claude-sonnet-4-5".into(),
            api_key: String::new(),
            base_url: String::new(),
            max_tokens: 0,
            context_window: 0,
            temperature: None,
            top_p: None,
            thinking: None,
            stream: None,
        }
    }
}

impl LlmProviderConfig {
    /// Returns the API key value to pass to provider clients.
    pub fn effective_api_key(&self) -> &str {
        if self.api_key.trim().is_empty()
            && is_local_ollama_openai_provider(&self.provider, &self.base_url)
        {
            return "ollama";
        }
        &self.api_key
    }
}

/// A pre-configured SSH server entry.
/// The password / private_key is stored encrypted on disk (same hex-AES scheme as API keys).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SshServerConfig {
    /// Unique alias used as `connection_id` (e.g. "prod", "dev-server")
    pub id: String,
    /// Display label (optional, falls back to id)
    #[serde(default)]
    pub label: String,
    /// Hostname or IP address
    pub host: String,
    /// SSH port (default 22)
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    /// SSH username
    pub username: String,
    /// Password (stored encrypted, empty if using key auth)
    #[serde(default)]
    pub password: String,
    /// PEM private key content (stored encrypted, empty if using password auth)
    #[serde(default)]
    pub private_key: String,
}

fn default_ssh_port() -> u16 {
    22
}

fn has_local_ollama_prefix(url: &str, prefix: &str) -> bool {
    url.strip_prefix(prefix)
        .map(|rest| rest.is_empty() || rest.starts_with('/'))
        .unwrap_or(false)
}

/// Ollama's local OpenAI-compatible API ignores bearer tokens, but OpenAI-style
/// clients still expect a non-empty API key value.
pub fn is_local_ollama_openai_provider(provider: &str, base_url: &str) -> bool {
    let provider = provider.trim().to_ascii_lowercase();
    if provider != "custom" && provider != "ollama" {
        return false;
    }

    let url = base_url.trim().trim_end_matches('/').to_ascii_lowercase();
    [
        "http://localhost:11434",
        "http://127.0.0.1:11434",
        "http://[::1]:11434",
    ]
    .iter()
    .any(|prefix| has_local_ollama_prefix(&url, prefix))
}

/// Tunables for background skill review, curator maintenance, and evolution policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillEvolutionSettings {
    /// Run background skill/memory review after each agent turn.
    #[serde(default = "default_true")]
    pub review_enabled: bool,
    /// When false, skip per-turn review (manual/curator paths still apply).
    #[serde(default = "default_true")]
    pub review_every_turn: bool,
    /// Minimum tool-call count in a turn before creating a new umbrella draft skill.
    #[serde(default = "default_create_skill_min_tool_calls")]
    pub create_skill_min_tool_calls: u32,
    /// Minimum agent turns between new umbrella draft skill creations.
    #[serde(default = "default_umbrella_skill_interval_turns")]
    pub umbrella_skill_interval_turns: u32,
    /// Curator auto-run interval in hours (default 168 = 7 days).
    #[serde(default = "default_curator_interval_hours")]
    pub curator_interval_hours: u32,
    /// Minimum system idle hours before curator may auto-run.
    #[serde(default = "default_curator_min_idle_hours")]
    pub curator_min_idle_hours: u32,
    /// Mark agent-created skills as stale after N days without use.
    #[serde(default = "default_stale_after_days")]
    pub stale_after_days: u32,
    /// Archive agent-created stale skills after N days without use.
    #[serde(default = "default_archive_after_days")]
    pub archive_after_days: u32,
    /// LLM pass to merge near-duplicate drafts and patch drift (curator phase 2).
    #[serde(default = "default_true")]
    pub curator_llm_merge_enabled: bool,
}

impl Default for SkillEvolutionSettings {
    fn default() -> Self {
        Self {
            review_enabled: true,
            review_every_turn: true,
            create_skill_min_tool_calls: default_create_skill_min_tool_calls(),
            umbrella_skill_interval_turns: default_umbrella_skill_interval_turns(),
            curator_interval_hours: default_curator_interval_hours(),
            curator_min_idle_hours: default_curator_min_idle_hours(),
            stale_after_days: default_stale_after_days(),
            archive_after_days: default_archive_after_days(),
            curator_llm_merge_enabled: true,
        }
    }
}

fn default_create_skill_min_tool_calls() -> u32 {
    5
}
fn default_umbrella_skill_interval_turns() -> u32 {
    10
}
fn default_curator_interval_hours() -> u32 {
    168
}
fn default_curator_min_idle_hours() -> u32 {
    2
}
fn default_stale_after_days() -> u32 {
    30
}
fn default_archive_after_days() -> u32 {
    90
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelRuntimeProfile {
    pub context_window: u32,
    pub max_output_tokens: u32,
    #[serde(default)]
    pub supported_endpoint_types: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Anthropic API key
    #[serde(default)]
    pub anthropic_api_key: String,
    /// OpenAI API key
    #[serde(default)]
    pub openai_api_key: String,
    /// DeepSeek API key
    #[serde(default)]
    pub deepseek_api_key: String,
    /// Qwen (通义千问) API key
    #[serde(default)]
    pub qwen_api_key: String,
    /// MiniMax API key
    #[serde(default)]
    pub minimax_api_key: String,
    /// Zhipu AI (智谱) API key
    #[serde(default)]
    pub zhipu_api_key: String,
    /// Kimi (Moonshot AI) API key
    #[serde(default)]
    pub kimi_api_key: String,
    /// Active LLM provider: "anthropic" | "openai" | "custom" | "deepseek" | "qwen"
    #[serde(default = "default_provider")]
    pub provider: String,
    /// Model name (e.g. "claude-sonnet-4-5" or "gpt-4o")
    #[serde(default = "default_model")]
    pub model: String,
    /// Custom base URL for OpenAI-compatible endpoints
    #[serde(default)]
    pub custom_base_url: String,
    /// Workspace root directory (files are restricted to this path)
    #[serde(default = "default_workspace")]
    pub workspace_root: String,
    /// When true, the agent may access files outside workspace_root (shows a warning).
    /// When false, workspace_root must be non-empty and all file access is restricted to it.
    #[serde(default)]
    pub allow_outside_workspace: bool,
    /// UI language: "zh" | "en"
    #[serde(default = "default_language")]
    pub language: String,
    /// Maximum tokens per LLM response (output only)
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    /// Context window size in tokens (input limit).
    /// 0 means "auto" — the backend will use a conservative default based on the model.
    /// Common values: 8192, 32768, 65536, 131072, 1000000
    #[serde(default)]
    pub context_window: u32,
    /// Fallback models tried in order when the primary model fails with
    /// rate_limit / overloaded / model_not_found errors.
    /// Format: "provider/model" e.g. ["anthropic/claude-haiku-3-5", "openai/gpt-4o-mini"]
    #[serde(default)]
    pub fallback_models: Vec<String>,
    /// Runtime capabilities discovered for each provider/model identifier.
    #[serde(default)]
    pub model_runtime_profiles: HashMap<String, ModelRuntimeProfile>,
    /// Whether to show permission dialogs for shell commands
    #[serde(default = "default_true")]
    pub confirm_shell_commands: bool,
    /// Whether to show permission dialogs for file writes
    #[serde(default = "default_true")]
    pub confirm_file_writes: bool,
    /// Tool names permanently auto-approved by user (e.g. ["shell", "code_run"] or ["*"])
    #[serde(default)]
    pub always_allow_tools: Vec<String>,
    /// Run browser in headless mode (invisible). false = user can see the browser window.
    #[serde(default = "default_true")]
    pub browser_headless: bool,
    /// Policy profile: strict | balanced | dev
    #[serde(default = "default_policy_mode")]
    pub policy_mode: String,
    /// Max tool calls per minute per session
    #[serde(default = "default_tool_rate_limit")]
    pub tool_rate_limit_per_minute: u32,

    // ── IM Gateway ──────────────────────────────────────────────────────────
    //
    // Layered architecture (see `piscis-core::scene::SceneKind::IMHeadless`
    // and `crate::tools::mcp`):
    //   * Credential layer  — `*_app_id` / `*_bot_id` / `*_app_secret` are
    //     the application-level credentials shared by the IM channel and
    //     by any enterprise-capability MCP that uses the same platform.
    //   * Channel layer    — `*_enabled` flips the WebSocket / Stream
    //     long-connection on or off; channels only carry message
    //     transport.
    //   * Capability layer — enterprise APIs (org chart, calendar, group
    //     chat, …) live in `mcp_servers` (see below) and reference the
    //     credentials above through `${settings:*}` placeholders so users
    //     never duplicate secrets between the channel and the MCP.
    /// Feishu App ID
    #[serde(default)]
    pub feishu_app_id: String,
    /// Feishu App Secret
    #[serde(default)]
    pub feishu_app_secret: String,
    /// Feishu domain: "feishu" | "lark"
    #[serde(default = "default_feishu_domain")]
    pub feishu_domain: String,
    /// Feishu enabled
    #[serde(default)]
    pub feishu_enabled: bool,

    /// WeCom smart robot Bot ID (long-connection mode)
    #[serde(default)]
    pub wecom_bot_id: String,
    /// WeCom smart robot Secret (long-connection mode)
    #[serde(default)]
    pub wecom_bot_secret: String,
    /// WeCom enabled
    #[serde(default)]
    pub wecom_enabled: bool,

    /// DingTalk App Key
    #[serde(default)]
    pub dingtalk_app_key: String,
    /// DingTalk App Secret
    #[serde(default)]
    pub dingtalk_app_secret: String,
    /// DingTalk robot code
    #[serde(default)]
    pub dingtalk_robot_code: String,
    /// DingTalk Corp ID (kept for official OpenClaw-compatible config)
    #[serde(default)]
    pub dingtalk_corp_id: String,
    /// DingTalk Agent ID (kept for official OpenClaw-compatible config)
    #[serde(default)]
    pub dingtalk_agent_id: String,
    /// DingTalk official MCP Marketplace / AIHub URL (Streamable HTTP/SSE)
    #[serde(default)]
    pub dingtalk_mcp_url: String,
    /// DingTalk enabled
    #[serde(default)]
    pub dingtalk_enabled: bool,

    /// Telegram Bot Token
    #[serde(default)]
    pub telegram_bot_token: String,
    /// Telegram enabled
    #[serde(default)]
    pub telegram_enabled: bool,

    /// Slack incoming webhook URL
    #[serde(default)]
    pub slack_webhook_url: String,
    #[serde(default)]
    pub slack_enabled: bool,

    /// Discord webhook URL
    #[serde(default)]
    pub discord_webhook_url: String,
    #[serde(default)]
    pub discord_enabled: bool,

    /// Microsoft Teams incoming webhook URL
    #[serde(default)]
    pub teams_webhook_url: String,
    #[serde(default)]
    pub teams_enabled: bool,

    /// Matrix homeserver base URL (e.g. https://matrix.org)
    #[serde(default)]
    pub matrix_homeserver: String,
    /// Matrix access token
    #[serde(default)]
    pub matrix_access_token: String,
    /// Matrix room id
    #[serde(default)]
    pub matrix_room_id: String,
    #[serde(default)]
    pub matrix_enabled: bool,

    /// Generic outbound webhook URL
    #[serde(default)]
    pub webhook_outbound_url: String,
    /// Optional bearer token for outbound webhook
    #[serde(default)]
    pub webhook_auth_token: String,
    #[serde(default)]
    pub webhook_enabled: bool,

    /// WeChat (iLink Bot HTTP server) enabled
    #[serde(default)]
    pub wechat_enabled: bool,
    /// Optional Bearer token for the local iLink Bot HTTP server (guards the listener)
    #[serde(default)]
    pub wechat_gateway_token: String,
    /// Local HTTP server port for the iLink Bot API (default 18788)
    #[serde(default = "default_wechat_gateway_port")]
    pub wechat_gateway_port: u16,
    /// bot_token obtained after QR-code login (encrypted at rest)
    #[serde(default)]
    pub wechat_bot_token: String,
    /// baseurl returned by the iLink login API (e.g. https://ilinkai.weixin.qq.com)
    #[serde(default)]
    pub wechat_base_url: String,
    /// ilink_bot_id of the bound WeChat account
    #[serde(default)]
    pub wechat_bot_id: String,

    /// Whether inbound IM messages should hide the main window and show the
    /// minimal overlay while Piscis replies.
    #[serde(default = "default_true")]
    pub im_auto_minimal_mode: bool,
    /// How to handle new inbound IM messages while Piscis is already processing
    /// a previous message in the same session.
    /// "queue"  = enqueue new messages and process them sequentially.
    /// "cancel" = cancel the current run and start processing immediately.
    #[serde(default = "default_im_message_mode")]
    pub im_message_mode: String,

    // ── Email (SMTP / IMAP) ──────────────────────────────────────────────────
    /// SMTP server hostname (e.g. smtp.gmail.com)
    #[serde(default)]
    pub smtp_host: String,
    /// SMTP port (default 587 for STARTTLS, 465 for SSL)
    #[serde(default = "default_smtp_port")]
    pub smtp_port: u16,
    /// SMTP/IMAP account username (usually the email address)
    #[serde(default)]
    pub smtp_username: String,
    /// SMTP account password or app-password (encrypted at rest)
    #[serde(default)]
    pub smtp_password: String,
    /// IMAP server hostname (e.g. imap.gmail.com)
    #[serde(default)]
    pub imap_host: String,
    /// IMAP port (default 993 for SSL)
    #[serde(default = "default_imap_port")]
    pub imap_port: u16,
    /// Sender display name shown in the From header (optional)
    #[serde(default)]
    pub smtp_from_name: String,
    /// Whether email tool is enabled
    #[serde(default)]
    pub email_enabled: bool,

    // ── Agent Loop ──────────────────────────────────────────────────────────
    /// Maximum tool-call iterations per agent run (default 50)
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,
    /// Cumulative-input-tokens cap after which an auto summary/compaction
    /// kicks in. Set to 0 to disable.
    ///
    /// Works alongside — not replaced by — the three-tier percent scheme
    /// ([`Self::compaction_micro_percent`] / `compaction_auto_percent` /
    /// `compaction_full_percent`): the percent tiers shape *how* compaction
    /// behaves at each utilisation band, while this absolute threshold
    /// decides *when* the threshold-driven path fires (scene policy can
    /// override via `auto_compact_threshold_override`).
    #[serde(default = "default_auto_compact_input_tokens_threshold")]
    pub auto_compact_input_tokens_threshold: u32,

    /// p5a / Claw-inspired three-tier compaction thresholds, expressed as
    /// *percentage of the effective input-token budget* (= context window
    /// minus `max_tokens`).
    ///
    /// *   Below `compaction_micro_percent` — nothing extra happens, just
    ///     the default receipt demotion from p5.
    /// *   `compaction_micro_percent`..`compaction_auto_percent` — MICRO:
    ///     aggressive receipt demotion + per-result truncation (see
    ///     `max_tool_result_tokens`).
    /// *   `compaction_auto_percent`..`compaction_full_percent` — AUTO:
    ///     trigger an async summary run (p7), replace the oldest non-recent
    ///     history with a rolling summary.
    /// *   `compaction_full_percent`+ — FULL: synchronous summary before
    ///     the next LLM call; last line of defence against overflow.
    #[serde(default = "default_compaction_micro_percent")]
    pub compaction_micro_percent: u8,
    #[serde(default = "default_compaction_auto_percent")]
    pub compaction_auto_percent: u8,
    #[serde(default = "default_compaction_full_percent")]
    pub compaction_full_percent: u8,

    /// Per-tool-result truncation cap in tokens (p5a). Tool results whose
    /// estimated tokens exceed this value are written to the minimal
    /// receipt form at the time the history is constructed, regardless of
    /// how recent they are. 0 disables the cap.
    #[serde(default = "default_max_tool_result_tokens")]
    pub max_tool_result_tokens: u32,

    /// Optional override for the model used to produce rolling summaries
    /// (p7). When `None` or empty, the main chat model is reused.
    #[serde(default)]
    pub summary_model: Option<String>,
    /// Maximum characters from project instruction files injected into the system prompt.
    #[serde(default = "default_project_instruction_budget_chars")]
    pub project_instruction_budget_chars: u32,
    /// Whether project-level instruction files (PISCIS.md, .piscis/instructions.md, etc.)
    /// should be discovered and injected into the system prompt.
    #[serde(default = "default_true")]
    pub enable_project_instructions: bool,
    /// Personal Piscis-only prompt appended to Piscis-owned sessions.
    /// This is guidance context for Piscis chat, heartbeat, pool coordination,
    /// and scheduled tasks; it must not be injected into Koi or Fish prompts.
    #[serde(default)]
    pub piscis_personal_prompt: String,
    /// LLM read timeout in seconds (default 120). Increase for slow models.
    #[serde(default = "default_llm_read_timeout_secs")]
    pub llm_read_timeout_secs: u32,
    /// Koi task execution timeout in seconds (default 600 = 10 min). Increase for complex tasks.
    #[serde(default = "default_koi_timeout_secs")]
    pub koi_timeout_secs: u32,

    // ── Heartbeat ───────────────────────────────────────────────────────────
    /// Whether the heartbeat runner is enabled
    #[serde(default = "default_heartbeat_enabled")]
    pub heartbeat_enabled: bool,
    /// Heartbeat interval in minutes (default 30)
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval_mins: u32,
    /// Prompt sent to the agent on each heartbeat
    #[serde(default = "default_heartbeat_prompt")]
    pub heartbeat_prompt: String,
    /// Whether the app has already seeded the first-run starter Koi set.
    #[serde(default)]
    pub starter_kois_initialized: bool,

    // ── Skill evolution ─────────────────────────────────────────────────────
    #[serde(default)]
    pub skill_evolution: SkillEvolutionSettings,

    // ── User Tools ──────────────────────────────────────────────────────────
    /// Per-user-tool config values, keyed by tool name.
    /// Each value is a JSON object with the fields from the tool's config_schema.
    /// Password fields are stored encrypted (same hex-AES scheme as API keys).
    #[serde(default)]
    pub user_tool_configs: HashMap<String, Value>,
    /// Built-in tool enable switches, keyed by tool name.
    /// Missing key means enabled by default.
    #[serde(default)]
    pub builtin_tool_enabled: HashMap<String, bool>,

    // ── MCP Servers ─────────────────────────────────────────────────────────
    /// Configured MCP (Model Context Protocol) servers.
    /// Each server exposes tools that are dynamically registered into the agent's tool registry.
    #[serde(default)]
    pub mcp_servers: Vec<McpServerConfig>,

    // ── SSH Servers ──────────────────────────────────────────────────────────
    /// Pre-configured SSH servers. The agent can connect by alias (connection_id)
    /// without needing to know the credentials.
    #[serde(default)]
    pub ssh_servers: Vec<SshServerConfig>,

    // ── Named LLM Providers ──────────────────────────────────────────────────
    /// Named LLM provider configurations. Each Koi can reference one by id.
    /// When a Koi has no provider assigned, the global provider/model/key is used.
    #[serde(default)]
    pub llm_providers: Vec<LlmProviderConfig>,

    /// User-entered list prices per provider + model, used for cost display.
    /// Empty means no price is known and no cost is shown.
    #[serde(default)]
    pub model_prices: Vec<ModelPrice>,

    // ── Runtime Paths ───────────────────────────────────────────────────────
    /// User-specified executable paths for runtimes not found on PATH.
    /// Keys: "node", "npm", "python", "pip", "git"
    /// Values: absolute path to the executable (e.g. "C:\\Python312\\python.exe")
    #[serde(default)]
    pub runtime_paths: HashMap<String, String>,

    // ── Vision / Multimodal ─────────────────────────────────────────────────
    /// Whether the current model supports vision (image input).
    /// When true, inbound IM images are passed as visual content blocks to the LLM.
    /// Auto-detected for known models (Claude 3+, GPT-4o, Gemini, etc.); set this
    /// manually when using a custom model name that isn't auto-recognised.
    #[serde(default)]
    pub vision_enabled: bool,

    // ── Vision Model (UIA / screen_capture / desktop_automation) ────────────
    /// When true, vision-based tools reuse the main LLM settings.
    /// When false, the separate vision_provider/model/api_key/base_url below are used.
    #[serde(default = "default_true")]
    pub vision_use_main_llm: bool,
    /// Vision LLM provider (e.g. "anthropic", "openai", "qwen")
    #[serde(default)]
    pub vision_provider: String,
    /// Vision LLM model name (e.g. "claude-sonnet-4-5", "gpt-4o")
    #[serde(default)]
    pub vision_model: String,
    /// Vision LLM API key
    #[serde(default)]
    pub vision_api_key: String,
    /// Vision LLM custom base URL
    #[serde(default)]
    pub vision_base_url: String,

    // ── Streaming output ────────────────────────────────────────────────────
    /// Stream LLM text deltas to the UI as they arrive instead of waiting
    /// for the full response. Only wired into the main chat / Koi task
    /// scenes; headless / coordinator / heartbeat paths ignore this flag
    /// because they do not surface text to a live UI.
    #[serde(default)]
    pub enable_streaming: bool,

    /// Sampling temperature for the active model; None = provider default.
    #[serde(default)]
    pub temperature: Option<f32>,
    /// Nucleus sampling top_p for the active model; None = provider default.
    #[serde(default)]
    pub top_p: Option<f32>,
    /// Provider thinking mode for the active model; None/false = disabled.
    #[serde(default)]
    pub thinking: Option<bool>,

    // ── Overlay position ─────────────────────────────────────────────────────
    /// Last saved X position of the overlay window (physical pixels).
    /// None means "first launch — center relative to main window".
    #[serde(default)]
    pub overlay_x: Option<i32>,
    /// Last saved Y position of the overlay window (physical pixels).
    #[serde(default)]
    pub overlay_y: Option<i32>,

    /// When true, multiple instances of the app may run simultaneously.
    /// When false (default), launching a second instance will focus the existing window
    /// and exit the new process immediately.
    #[serde(default)]
    pub allow_multiple_instances: bool,

    /// Internal: path to the config file (not serialized)
    #[serde(skip)]
    pub config_path: PathBuf,
}

fn default_feishu_domain() -> String {
    "feishu".into()
}
fn default_wechat_gateway_port() -> u16 {
    18789
}
fn default_smtp_port() -> u16 {
    587
}
fn default_imap_port() -> u16 {
    993
}
fn default_max_iterations() -> u32 {
    50
}
fn default_auto_compact_input_tokens_threshold() -> u32 {
    // Bumped from 100k → 200k after introducing dual-version tool results.
    // With minimal receipts in middle-tier turns, sessions can accumulate more
    // raw input tokens before a forced Level-2 summarisation buys us anything,
    // so the old default was firing far too often on long research sessions.
    200_000
}
fn default_compaction_micro_percent() -> u8 {
    // Align with `crate::agent::harness::budget::DEFAULT_TIER_MICRO_PERCENT`.
    // Duplicated here (rather than re-exported) so the store crate stays
    // leaf-level with no agent dependency.
    60
}
fn default_compaction_auto_percent() -> u8 {
    80
}
fn default_compaction_full_percent() -> u8 {
    95
}
fn default_max_tool_result_tokens() -> u32 {
    // 8k tokens ≈ 32 KB at ~4 chars/token; enough to hold a full file_read
    // of a medium source file but small enough that one oversized result
    // never dominates a 200k-token budget.
    8_000
}
fn default_project_instruction_budget_chars() -> u32 {
    8_000
}
fn default_llm_read_timeout_secs() -> u32 {
    120
}
fn default_koi_timeout_secs() -> u32 {
    600
}
fn default_heartbeat_interval() -> u32 {
    30
}
fn default_heartbeat_enabled() -> bool {
    true
}
pub fn default_heartbeat_prompt() -> String {
    "这是你的例行心跳巡查。你的职责是**监督项目是否按 org_spec 收敛**，而不是只扫一眼任务板有没有未完成任务。\n\n\
     ## 0. 硬性门禁（未通过不得回复 HEARTBEAT_OK）\n\
     对每个 active 项目池，你必须按顺序完成：\n\
     1) pool_org(action=\"read\", pool_id=...) — **完整阅读** org_spec；\n\
     2) pool_org(action=\"get_todos\", pool_id=...) + pool_org(action=\"get_messages\", pool_id=...)；\n\
     3) 用你自己的推理对照 org_spec：当前交付物、任务分解、消息记录是否已满足规约中的**全部**要求（含多阶段/多里程碑/角色分工/验收标准等，以 org_spec 原文为准，不要臆测阶段编号）。\n\n\
     **禁止**在以下任一情况仅回复 HEARTBEAT_OK 或“无需干预”：\n\
     - 未对该池执行过 pool_org(read)；\n\
     - 任务板看似“全 done”但 org_spec 中仍有未覆盖的工作项、后续阶段、未分配角色或未验收交付物；\n\
     - 仅完成了规约中的早期步骤，后续步骤在 org_spec 中有描述但板上无对应 todo/无进展消息；\n\
     - 存在 needs_review / blocked / task_failed 未处理。\n\n\
     若 org_spec 未收敛：必须 pool_org(action=\"post_status\") 写明差距与下一步，并用 pool_org(action=\"create_todo\") 或 pool_org(action=\"assign_koi\") **落实**后续工作；不得只提醒不派活。\n\n\
     ## 1. 活跃项目巡查\n\
     用 pool_org(action=\"list\") 列出所有 active 池，对每个池完成 §0 后按板面行动：\n\
     - needs_review：审查产物后合并分支、返工（resume/replace/assign_koi）或 post_status 说明需人工决策；\n\
     - blocked：解除阻塞或改派；\n\
     - 无活跃 todo 但 org_spec 未收敛（常见：只做完了第一阶段）：按 org_spec 拆下一批 todo 并 assign_koi，禁止把项目晾在那里；\n\
     - 无活跃 todo 且 org_spec 已收敛：post_status 说明收敛与待用户确认归档，**勿**自动 archive；\n\
     - Koi 空转：post_status 定方向或 assign_koi。\n\n\
     ⚠️ 心跳中**禁止** pool_org(action=\"create\") 新建项目池；追加工作只在现有池 create_todo / assign_koi。\n\n\
     ## 2. Koi 状态\n\
     长时间 busy 且无活跃 todo：post_status 或 assign_koi。\n\n\
     ## 3. 定时任务\n\
     app_control(action=\"list_scheduled_tasks\")，按需处理。\n\n\
     仅当：§0 对每个 active 池均已执行且 needs_review/blocked/失败/规约缺口均已处理或已派活后，才可回复 HEARTBEAT_OK。"
        .into()
}

fn default_provider() -> String {
    "anthropic".into()
}
fn default_model() -> String {
    "claude-sonnet-4-5".into()
}
pub fn default_workspace_path() -> String {
    dirs::document_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Piscis")
        .to_string_lossy()
        .into_owned()
}

fn default_workspace() -> String {
    default_workspace_path()
}
fn default_language() -> String {
    "zh".into()
}
fn default_max_tokens() -> u32 {
    8192
}
fn default_true() -> bool {
    true
}
fn default_policy_mode() -> String {
    "balanced".into()
}
fn default_tool_rate_limit() -> u32 {
    120
}
fn default_im_message_mode() -> String {
    "queue".into()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            anthropic_api_key: String::new(),
            openai_api_key: String::new(),
            deepseek_api_key: String::new(),
            qwen_api_key: String::new(),
            minimax_api_key: String::new(),
            zhipu_api_key: String::new(),
            kimi_api_key: String::new(),
            provider: default_provider(),
            model: default_model(),
            custom_base_url: String::new(),
            workspace_root: default_workspace(),
            allow_outside_workspace: false,
            language: default_language(),
            max_tokens: default_max_tokens(),
            context_window: 0,
            fallback_models: vec![],
            model_runtime_profiles: HashMap::new(),
            confirm_shell_commands: true,
            confirm_file_writes: true,
            always_allow_tools: Vec::new(),
            browser_headless: true,
            policy_mode: default_policy_mode(),
            tool_rate_limit_per_minute: default_tool_rate_limit(),
            feishu_app_id: String::new(),
            feishu_app_secret: String::new(),
            feishu_domain: default_feishu_domain(),
            feishu_enabled: false,
            wecom_bot_id: String::new(),
            wecom_bot_secret: String::new(),
            wecom_enabled: false,
            dingtalk_app_key: String::new(),
            dingtalk_app_secret: String::new(),
            dingtalk_robot_code: String::new(),
            dingtalk_corp_id: String::new(),
            dingtalk_agent_id: String::new(),
            dingtalk_mcp_url: String::new(),
            dingtalk_enabled: false,
            telegram_bot_token: String::new(),
            telegram_enabled: false,
            slack_webhook_url: String::new(),
            slack_enabled: false,
            discord_webhook_url: String::new(),
            discord_enabled: false,
            teams_webhook_url: String::new(),
            teams_enabled: false,
            matrix_homeserver: String::new(),
            matrix_access_token: String::new(),
            matrix_room_id: String::new(),
            matrix_enabled: false,
            webhook_outbound_url: String::new(),
            webhook_auth_token: String::new(),
            webhook_enabled: false,
            wechat_enabled: false,
            wechat_gateway_token: String::new(),
            wechat_gateway_port: default_wechat_gateway_port(),
            wechat_bot_token: String::new(),
            wechat_base_url: String::new(),
            wechat_bot_id: String::new(),
            im_auto_minimal_mode: true,
            im_message_mode: default_im_message_mode(),
            smtp_host: String::new(),
            smtp_port: default_smtp_port(),
            smtp_username: String::new(),
            smtp_password: String::new(),
            imap_host: String::new(),
            imap_port: default_imap_port(),
            smtp_from_name: String::new(),
            email_enabled: false,
            max_iterations: default_max_iterations(),
            auto_compact_input_tokens_threshold: default_auto_compact_input_tokens_threshold(),
            compaction_micro_percent: default_compaction_micro_percent(),
            compaction_auto_percent: default_compaction_auto_percent(),
            compaction_full_percent: default_compaction_full_percent(),
            max_tool_result_tokens: default_max_tool_result_tokens(),
            summary_model: None,
            project_instruction_budget_chars: default_project_instruction_budget_chars(),
            enable_project_instructions: true,
            piscis_personal_prompt: String::new(),
            llm_read_timeout_secs: default_llm_read_timeout_secs(),
            koi_timeout_secs: default_koi_timeout_secs(),
            heartbeat_enabled: default_heartbeat_enabled(),
            heartbeat_interval_mins: default_heartbeat_interval(),
            heartbeat_prompt: default_heartbeat_prompt(),
            starter_kois_initialized: false,
            skill_evolution: SkillEvolutionSettings::default(),
            user_tool_configs: HashMap::new(),
            builtin_tool_enabled: HashMap::new(),
            mcp_servers: Vec::new(),
            ssh_servers: Vec::new(),
            llm_providers: Vec::new(),
            model_prices: Vec::new(),
            runtime_paths: HashMap::new(),
            vision_enabled: false,
            vision_use_main_llm: true,
            vision_provider: String::new(),
            vision_model: String::new(),
            vision_api_key: String::new(),
            vision_base_url: String::new(),
            enable_streaming: false,
            temperature: None,
            top_p: None,
            thinking: None,
            overlay_x: None,
            overlay_y: None,
            allow_multiple_instances: false,
            config_path: PathBuf::new(),
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Result<Self> {
        let mut strict_secrets = false;
        let mut mcp_secrets_encrypted = false;
        let mut settings = if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let value: serde_json::Value = serde_json::from_str(&content)?;
            if let Some(version) = value.get("agentz_secret_format") {
                if !matches!(version.as_u64(), Some(1) | Some(2)) {
                    anyhow::bail!(
                        "Unsupported settings secret format; compatible recovery is required"
                    );
                }
                strict_secrets = true;
                mcp_secrets_encrypted = version.as_u64() == Some(2);
            }
            serde_json::from_value::<Settings>(value)?
        } else {
            Settings::default()
        };
        settings.config_path = path.to_path_buf();

        if strict_secrets
            && !path
                .parent()
                .is_some_and(|parent| parent.join(".secret_key").is_file())
        {
            anyhow::bail!("Protected settings key is missing; compatible recovery is required");
        }

        // Decrypt API keys (hex-encoded ciphertext on disk).
        // If decryption fails the value is likely still plaintext (pre-migration);
        // keep it as-is and the next save() will encrypt it.
        {
            let store = Self::secret_store(path)?;
            Self::try_decrypt_field(&store, &mut settings.anthropic_api_key, strict_secrets)?;
            Self::try_decrypt_field(&store, &mut settings.openai_api_key, strict_secrets)?;
            Self::try_decrypt_field(&store, &mut settings.deepseek_api_key, strict_secrets)?;
            Self::try_decrypt_field(&store, &mut settings.qwen_api_key, strict_secrets)?;
            Self::try_decrypt_field(&store, &mut settings.minimax_api_key, strict_secrets)?;
            Self::try_decrypt_field(&store, &mut settings.zhipu_api_key, strict_secrets)?;
            Self::try_decrypt_field(&store, &mut settings.kimi_api_key, strict_secrets)?;
            Self::try_decrypt_field(&store, &mut settings.feishu_app_secret, strict_secrets)?;
            Self::try_decrypt_field(&store, &mut settings.wecom_bot_secret, strict_secrets)?;
            Self::try_decrypt_field(&store, &mut settings.dingtalk_app_secret, strict_secrets)?;
            Self::try_decrypt_field(&store, &mut settings.dingtalk_robot_code, strict_secrets)?;
            Self::try_decrypt_field(&store, &mut settings.telegram_bot_token, strict_secrets)?;
            Self::try_decrypt_field(&store, &mut settings.matrix_access_token, strict_secrets)?;
            Self::try_decrypt_field(&store, &mut settings.webhook_auth_token, strict_secrets)?;
            Self::try_decrypt_field(&store, &mut settings.wechat_gateway_token, strict_secrets)?;
            Self::try_decrypt_field(&store, &mut settings.wechat_bot_token, strict_secrets)?;
            Self::try_decrypt_field(&store, &mut settings.smtp_password, strict_secrets)?;
            Self::try_decrypt_field(&store, &mut settings.vision_api_key, strict_secrets)?;
            // Decrypt SSH server credentials
            for srv in &mut settings.ssh_servers {
                Self::try_decrypt_field(&store, &mut srv.password, strict_secrets)?;
                Self::try_decrypt_field(&store, &mut srv.private_key, strict_secrets)?;
            }
            // Decrypt named LLM provider API keys
            for p in &mut settings.llm_providers {
                Self::try_decrypt_field(&store, &mut p.api_key, strict_secrets)?;
            }
            if mcp_secrets_encrypted {
                for server in &mut settings.mcp_servers {
                    Self::try_decrypt_field(&store, &mut server.command, true)?;
                    Self::try_decrypt_field(&store, &mut server.url, true)?;
                    for arg in &mut server.args {
                        Self::try_decrypt_field(&store, arg, true)?;
                    }
                    for value in server.env.values_mut().chain(server.headers.values_mut()) {
                        Self::try_decrypt_field(&store, value, true)?;
                    }
                }
            }
        }
        Ok(settings)
    }

    pub fn save(&self) -> Result<()> {
        self.save_with_encryptor(|store, value| store.encrypt_hex(value))
    }

    fn save_with_encryptor(
        &self,
        mut encrypt: impl FnMut(&crate::security::secrets::SecretStore, &str) -> Result<String>,
    ) -> Result<()> {
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut clone = self.clone();
        // Encrypt secret fields before writing to disk
        let directory = self
            .config_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .ok_or_else(|| anyhow::anyhow!("Settings require an explicit parent directory"))?;
        let store = crate::security::secrets::SecretStore::new(directory)?;
        {
            Self::encrypt_field(&store, &mut clone.anthropic_api_key, &mut encrypt)?;
            Self::encrypt_field(&store, &mut clone.openai_api_key, &mut encrypt)?;
            Self::encrypt_field(&store, &mut clone.deepseek_api_key, &mut encrypt)?;
            Self::encrypt_field(&store, &mut clone.qwen_api_key, &mut encrypt)?;
            Self::encrypt_field(&store, &mut clone.minimax_api_key, &mut encrypt)?;
            Self::encrypt_field(&store, &mut clone.zhipu_api_key, &mut encrypt)?;
            Self::encrypt_field(&store, &mut clone.kimi_api_key, &mut encrypt)?;
            Self::encrypt_field(&store, &mut clone.feishu_app_secret, &mut encrypt)?;
            Self::encrypt_field(&store, &mut clone.wecom_bot_secret, &mut encrypt)?;
            Self::encrypt_field(&store, &mut clone.dingtalk_app_secret, &mut encrypt)?;
            Self::encrypt_field(&store, &mut clone.dingtalk_robot_code, &mut encrypt)?;
            Self::encrypt_field(&store, &mut clone.telegram_bot_token, &mut encrypt)?;
            Self::encrypt_field(&store, &mut clone.matrix_access_token, &mut encrypt)?;
            Self::encrypt_field(&store, &mut clone.webhook_auth_token, &mut encrypt)?;
            Self::encrypt_field(&store, &mut clone.wechat_gateway_token, &mut encrypt)?;
            Self::encrypt_field(&store, &mut clone.wechat_bot_token, &mut encrypt)?;
            Self::encrypt_field(&store, &mut clone.smtp_password, &mut encrypt)?;
            Self::encrypt_field(&store, &mut clone.vision_api_key, &mut encrypt)?;
            // Encrypt SSH server credentials
            for srv in &mut clone.ssh_servers {
                Self::encrypt_field(&store, &mut srv.password, &mut encrypt)?;
                Self::encrypt_field(&store, &mut srv.private_key, &mut encrypt)?;
            }
            // Encrypt named LLM provider API keys
            for p in &mut clone.llm_providers {
                Self::encrypt_field(&store, &mut p.api_key, &mut encrypt)?;
            }
            for server in &mut clone.mcp_servers {
                Self::encrypt_field(&store, &mut server.command, &mut encrypt)?;
                Self::encrypt_field(&store, &mut server.url, &mut encrypt)?;
                for arg in &mut server.args {
                    Self::encrypt_field(&store, arg, &mut encrypt)?;
                }
                for value in server.env.values_mut().chain(server.headers.values_mut()) {
                    Self::encrypt_field(&store, value, &mut encrypt)?;
                }
            }
        }
        let mut value = serde_json::to_value(&clone)?;
        value["agentz_secret_format"] = serde_json::json!(2);
        let json = serde_json::to_string_pretty(&value)?;
        // Same-directory atomic replacement: failure leaves the old config intact.
        use std::io::Write;
        let mut file = tempfile::NamedTempFile::new_in(directory)?;
        file.write_all(json.as_bytes())?;
        file.as_file().sync_all()?;
        file.persist(&self.config_path)
            .map_err(|error| error.error)?;
        Ok(())
    }

    fn secret_store(config_path: &Path) -> Result<crate::security::secrets::SecretStore> {
        let directory = config_path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .ok_or_else(|| anyhow::anyhow!("Settings require an explicit parent directory"))?;
        crate::security::secrets::SecretStore::new(directory)
    }

    fn encrypt_field(
        store: &crate::security::secrets::SecretStore,
        field: &mut String,
        encrypt: &mut impl FnMut(&crate::security::secrets::SecretStore, &str) -> Result<String>,
    ) -> Result<()> {
        if !field.is_empty() {
            *field = encrypt(store, field)?;
        }
        Ok(())
    }

    fn try_decrypt_field(
        store: &crate::security::secrets::SecretStore,
        field: &mut String,
        strict: bool,
    ) -> Result<()> {
        if field.is_empty() {
            return Ok(());
        }
        match store.decrypt_hex(field) {
            Ok(decrypted) => *field = decrypted,
            Err(_) if strict => anyhow::bail!(
                "Cannot decrypt settings credential; compatible key recovery is required"
            ),
            Err(_) => {}
        }
        // Untagged legacy configurations can contain plaintext fields.
        Ok(())
    }

    /// Returns true if at least one API key is configured
    pub fn is_configured(&self) -> bool {
        !self.anthropic_api_key.trim().is_empty()
            || !self.openai_api_key.trim().is_empty()
            || !self.deepseek_api_key.trim().is_empty()
            || !self.qwen_api_key.trim().is_empty()
            || !self.minimax_api_key.trim().is_empty()
            || !self.zhipu_api_key.trim().is_empty()
            || !self.kimi_api_key.trim().is_empty()
            || is_local_ollama_openai_provider(&self.provider, &self.custom_base_url)
    }

    /// Returns the active API key value to pass to the configured provider.
    pub fn active_api_key(&self) -> &str {
        match self.provider.as_str() {
            "custom" | "ollama"
                if self.openai_api_key.trim().is_empty()
                    && is_local_ollama_openai_provider(&self.provider, &self.custom_base_url) =>
            {
                "ollama"
            }
            "openai" | "custom" | "ollama" => &self.openai_api_key,
            "deepseek" => &self.deepseek_api_key,
            "qwen" | "tongyi" => &self.qwen_api_key,
            "minimax" => &self.minimax_api_key,
            "zhipu" => &self.zhipu_api_key,
            "kimi" | "moonshot" => &self.kimi_api_key,
            _ => &self.anthropic_api_key,
        }
    }

    /// Look up a named LLM provider by its id.
    /// Returns `None` if no provider with that id exists.
    pub fn client_options(&self) -> crate::llm::ClientOptions {
        crate::llm::ClientOptions {
            temperature: self.temperature,
            top_p: self.top_p,
            thinking: self.thinking,
        }
    }

    pub fn find_llm_provider(&self, id: &str) -> Option<&LlmProviderConfig> {
        self.llm_providers.iter().find(|p| p.id == id)
    }

    /// Returns the default config file path used by the app before Tauri's AppHandle is available.
    /// Mirrors the path that `AppState::new_sync` computes via `app.path().app_data_dir()`.
    /// On Windows this is `%APPDATA%\com.piscis.desktop\config.json`.
    pub fn default_config_path() -> PathBuf {
        // Tauri v2 uses `<data_dir>/<bundle_identifier>` on Windows (APPDATA\<id>).
        // The bundle identifier is "com.piscis.desktop" (from tauri.conf.json).
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("com.piscis.desktop")
            .join("config.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_settings_json_defaults_model_runtime_profiles_to_empty() {
        let settings: Settings = serde_json::from_str("{}").expect("legacy settings JSON");
        assert!(settings.model_runtime_profiles.is_empty());
    }

    #[test]
    fn model_runtime_profiles_roundtrip_without_losing_capabilities() {
        let mut settings = Settings::default();
        settings.model_runtime_profiles.insert(
            "provider/model".into(),
            ModelRuntimeProfile {
                context_window: 128_000,
                max_output_tokens: 16_384,
                supported_endpoint_types: vec!["chat".into(), "image".into()],
            },
        );

        let encoded = serde_json::to_string(&settings).expect("serialize settings");
        let decoded: Settings = serde_json::from_str(&encoded).expect("deserialize settings");
        let profile = decoded
            .model_runtime_profiles
            .get("provider/model")
            .expect("runtime profile");
        assert_eq!(profile.context_window, 128_000);
        assert_eq!(profile.max_output_tokens, 16_384);
        assert_eq!(profile.supported_endpoint_types, ["chat", "image"]);
    }
    fn fixture() -> (tempfile::TempDir, Settings) {
        let directory = tempfile::tempdir().unwrap();
        let settings = Settings {
            config_path: directory.path().join("config.json"),
            openai_api_key: "fixture-secret-never-plaintext".into(),
            vision_api_key: "fixture-vision-secret".into(),
            ..Settings::default()
        };
        (directory, settings)
    }

    #[test]
    fn secrets_save_roundtrips_and_atomically_replaces_existing_config() {
        let (directory, mut settings) = fixture();
        settings.save().unwrap();
        settings.openai_api_key = "replacement-fixture-secret".into();
        settings.save().unwrap();
        let raw = std::fs::read_to_string(&settings.config_path).unwrap();
        assert!(!raw.contains("replacement-fixture-secret"));
        assert!(!raw.contains("fixture-vision-secret"));
        let loaded = Settings::load(&settings.config_path).unwrap();
        assert_eq!(loaded.openai_api_key, settings.openai_api_key);
        assert_eq!(loaded.vision_api_key, settings.vision_api_key);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
    }

    #[test]
    fn secrets_corrupted_key_preserves_old_config() {
        let (directory, settings) = fixture();
        settings.save().unwrap();
        let original = std::fs::read(&settings.config_path).unwrap();
        std::fs::write(directory.path().join(".secret_key"), b"damaged").unwrap();
        assert!(settings.save().is_err());
        assert_eq!(std::fs::read(&settings.config_path).unwrap(), original);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
    }

    #[test]
    fn secrets_protected_load_failure_preserves_config_and_does_not_generate_a_key() {
        let (directory, settings) = fixture();
        settings.save().unwrap();
        let original = std::fs::read(&settings.config_path).unwrap();
        let key_path = directory.path().join(".secret_key");
        let protected_key = std::fs::read(&key_path).unwrap();
        std::fs::remove_file(&key_path).unwrap();
        assert!(Settings::load(&settings.config_path).is_err());
        assert!(!key_path.exists());
        std::fs::write(&key_path, &protected_key).unwrap();
        let mut invalid_config: serde_json::Value = serde_json::from_slice(&original).unwrap();
        invalid_config["openai_api_key"] = serde_json::json!("corrupted-ciphertext");
        let damaged = serde_json::to_vec(&invalid_config).unwrap();
        std::fs::write(&settings.config_path, &damaged).unwrap();
        assert!(Settings::load(&settings.config_path).is_err());
        assert_eq!(std::fs::read(&settings.config_path).unwrap(), damaged);
        std::fs::write(&settings.config_path, &original).unwrap();
        assert_eq!(
            Settings::load(&settings.config_path)
                .unwrap()
                .openai_api_key,
            settings.openai_api_key
        );
    }

    #[test]
    fn secrets_legacy_config_migrates_without_changing_ciphertext() {
        let (directory, settings) = fixture();
        let store = crate::security::secrets::SecretStore::new(directory.path()).unwrap();
        let mut legacy = serde_json::to_value(&settings).unwrap();
        legacy["openai_api_key"] =
            serde_json::json!(store.encrypt_hex(&settings.openai_api_key).unwrap());
        legacy["vision_api_key"] = serde_json::json!(settings.vision_api_key.clone());
        let original = serde_json::to_vec(&legacy).unwrap();
        std::fs::write(&settings.config_path, &original).unwrap();
        let loaded = Settings::load(&settings.config_path).unwrap();
        assert_eq!(loaded.openai_api_key, settings.openai_api_key);
        assert_eq!(loaded.vision_api_key, settings.vision_api_key);
        assert_eq!(std::fs::read(&settings.config_path).unwrap(), original);
        loaded.save().unwrap();
        let migrated: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&settings.config_path).unwrap()).unwrap();
        assert_eq!(migrated["agentz_secret_format"], 2);
        assert_eq!(
            Settings::load(&settings.config_path)
                .unwrap()
                .vision_api_key,
            settings.vision_api_key
        );
    }

    #[test]
    fn secrets_encryption_failure_after_first_field_preserves_old_config() {
        let (directory, settings) = fixture();
        settings.save().unwrap();
        let original = std::fs::read(&settings.config_path).unwrap();
        let mut calls = 0;
        let result = settings.save_with_encryptor(|store, value| {
            calls += 1;
            if calls == 2 {
                anyhow::bail!("Injected nonce/random-source failure");
            }
            store.encrypt_hex(value)
        });
        assert!(result.is_err());
        assert_eq!(calls, 2);
        assert_eq!(std::fs::read(&settings.config_path).unwrap(), original);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
    }

    #[cfg(windows)]
    #[test]
    fn secrets_locked_config_preserves_old_bytes_and_cleans_temp_file() {
        use std::os::windows::fs::OpenOptionsExt;
        let (directory, settings) = fixture();
        settings.save().unwrap();
        let original = std::fs::read(&settings.config_path).unwrap();
        let locked = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&settings.config_path)
            .unwrap();
        assert!(settings.save().is_err());
        assert_eq!(std::fs::read(&settings.config_path).unwrap(), original);
        drop(locked);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
    }
    #[test]
    fn secrets_unreadable_key_path_preserves_old_config() {
        let (directory, settings) = fixture();
        std::fs::write(&settings.config_path, b"previous-config-bytes").unwrap();
        std::fs::create_dir(directory.path().join(".secret_key")).unwrap();
        assert!(settings.save().is_err());
        assert_eq!(
            std::fs::read(&settings.config_path).unwrap(),
            b"previous-config-bytes"
        );
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
    }
    #[test]
    fn secrets_nested_credentials_are_encrypted_and_failure_does_not_partially_save() {
        let (directory, mut settings) = fixture();
        settings.ssh_servers.push(SshServerConfig {
            password: "fixture-ssh-password".into(),
            private_key: "fixture-ssh-private-key".into(),
            ..SshServerConfig::default()
        });
        settings.llm_providers.push(LlmProviderConfig {
            api_key: "fixture-provider-api-key".into(),
            ..LlmProviderConfig::default()
        });
        settings.save().unwrap();
        let original = std::fs::read(&settings.config_path).unwrap();
        let raw = String::from_utf8(original.clone()).unwrap();
        for secret in [
            "fixture-ssh-password",
            "fixture-ssh-private-key",
            "fixture-provider-api-key",
        ] {
            assert!(!raw.contains(secret));
        }
        let loaded = Settings::load(&settings.config_path).unwrap();
        assert_eq!(
            loaded.ssh_servers[0].password,
            settings.ssh_servers[0].password
        );
        assert_eq!(
            loaded.ssh_servers[0].private_key,
            settings.ssh_servers[0].private_key
        );
        assert_eq!(
            loaded.llm_providers[0].api_key,
            settings.llm_providers[0].api_key
        );
        assert!(settings
            .save_with_encryptor(|store, value| {
                if value == "fixture-provider-api-key" {
                    anyhow::bail!("Injected final nested credential failure");
                }
                store.encrypt_hex(value)
            })
            .is_err());
        assert_eq!(std::fs::read(&settings.config_path).unwrap(), original);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
    }

    #[test]
    fn mcp_values_migrate_from_v1_plaintext_to_v2_encrypted() {
        let (_directory, mut settings) = fixture();
        settings.mcp_servers.push(McpServerConfig {
            name: "fixture".into(),
            command: "az-mcp-command-marker".into(),
            args: vec!["az-mcp-arg-marker".into()],
            url: "az-mcp-url-marker".into(),
            env: HashMap::from([("CUSTOM".into(), "az-mcp-env-marker".into())]),
            headers: HashMap::from([("Authorization".into(), "az-mcp-header-marker".into())]),
            ..McpServerConfig::default()
        });
        settings.save().unwrap();
        let mut v1: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&settings.config_path).unwrap()).unwrap();
        v1["agentz_secret_format"] = serde_json::json!(1);
        v1["mcp_servers"] = serde_json::to_value(&settings.mcp_servers).unwrap();
        std::fs::write(&settings.config_path, serde_json::to_vec(&v1).unwrap()).unwrap();
        let migrated = Settings::load(&settings.config_path).unwrap();
        assert_eq!(migrated.mcp_servers[0].env["CUSTOM"], "az-mcp-env-marker");
        migrated.save().unwrap();
        let raw = std::fs::read_to_string(&settings.config_path).unwrap();
        for marker in [
            "az-mcp-command-marker",
            "az-mcp-arg-marker",
            "az-mcp-url-marker",
            "az-mcp-env-marker",
            "az-mcp-header-marker",
        ] {
            assert!(!raw.contains(marker));
        }
        let loaded = Settings::load(&settings.config_path).unwrap();
        assert_eq!(
            loaded.mcp_servers[0].command,
            settings.mcp_servers[0].command
        );
        assert_eq!(loaded.mcp_servers[0].args, settings.mcp_servers[0].args);
        assert_eq!(loaded.mcp_servers[0].url, settings.mcp_servers[0].url);
        assert_eq!(loaded.mcp_servers[0].env, settings.mcp_servers[0].env);
        assert_eq!(
            loaded.mcp_servers[0].headers,
            settings.mcp_servers[0].headers
        );
        let mut damaged: serde_json::Value = serde_json::from_str(&raw).unwrap();
        damaged["mcp_servers"][0]["env"]["CUSTOM"] = serde_json::json!("broken-ciphertext");
        std::fs::write(&settings.config_path, serde_json::to_vec(&damaged).unwrap()).unwrap();
        assert!(Settings::load(&settings.config_path).is_err());
    }
}
