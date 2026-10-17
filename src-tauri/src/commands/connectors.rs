//! Connectors (Phase 0B) — authenticated external services exposed to the agent
//! as MCP tools (e.g. 通达信 / 腾讯文档 / QQ邮箱 / 飞书·钉钉·企微 的 OA-数据接口).
//!
//! A connector is a curated, friendly wrapper around an MCP server plus an auth
//! method. Manifests live in `{config}/connectors/<id>/connector.json`; the
//! user's credentials live next to them in `credentials.json` (never inside the
//! manifest). At agent-turn time [`resolve_connector_mcp_configs`] turns each
//! enabled + authorized connector into a kernel [`McpServerConfig`], injecting
//! credentials into `env` / `url` / `headers` via `${cred:<id>.<field>}`
//! placeholders.
//!
//! Transports map straight onto the kernel (v0.8.42+): `stdio` for local
//! subprocesses, and `sse` / `http` (streamable HTTP) for remote servers. For
//! remote transports the connector declares `headers` (e.g.
//! `Authorization: Bearer ${cred:foo.token}`) so authenticated / one-click
//! OAuth services work natively without proxying through a subprocess. For
//! `oauth2` connectors the stored `access_token` is auto-injected as a Bearer
//! header when the manifest does not declare an explicit `Authorization`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tracing::{info, warn};

use piscis_kernel::store::settings::McpServerConfig;

use crate::commands::data_scope::resolve_global_config_dir;

// ─── Manifest schema ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorAuthField {
    pub key: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub secret: bool,
    #[serde(default)]
    pub placeholder: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConnectorOAuth {
    #[serde(default)]
    pub authorize_url: String,
    #[serde(default)]
    pub token_url: String,
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub redirect: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorAuth {
    /// "none" | "api_key" | "token" | "oauth2"
    #[serde(default = "default_auth_method")]
    pub method: String,
    #[serde(default)]
    pub fields: Vec<ConnectorAuthField>,
    #[serde(default)]
    pub oauth: Option<ConnectorOAuth>,
}

fn default_auth_method() -> String {
    "none".into()
}

impl Default for ConnectorAuth {
    fn default() -> Self {
        Self {
            method: default_auth_method(),
            fields: Vec::new(),
            oauth: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorManifest {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub description: String,
    /// "mcp" (default). Reserved for future "builtin" native adapters.
    #[serde(default = "default_kind")]
    pub kind: String,
    /// "stdio" | "sse" | "http" (streamable HTTP)
    #[serde(default = "default_transport")]
    pub transport: String,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub url: String,
    /// Environment variables passed to the MCP subprocess. Values may contain
    /// `${cred:<id>.<field>}` placeholders resolved from stored credentials.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// HTTP headers for `sse` / `http` transports. Values may contain
    /// `${cred:<id>.<field>}` placeholders (e.g.
    /// `Authorization: Bearer ${cred:foo.token}`).
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub auth: ConnectorAuth,
    /// Whether this connector is active (its tools are registered into agents).
    #[serde(default)]
    pub enabled: bool,
    /// HTTP method for `kind: api` connectors (GET / POST / PUT).
    #[serde(default = "default_api_method")]
    pub api_method: String,
    /// When the agent should call this API (usage scenario).
    #[serde(default)]
    pub use_case: String,
    /// Parameter documentation or JSON-schema hint for the agent.
    #[serde(default)]
    pub parameters: String,
}

fn default_api_method() -> String {
    "POST".into()
}

fn default_kind() -> String {
    "mcp".into()
}
fn default_transport() -> String {
    "stdio".into()
}

impl ConnectorManifest {
    fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&text).map_err(|e| format!("invalid connector.json: {e}"))
    }
}

// ─── DTO ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ConnectorInfo {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub color: String,
    pub category: String,
    pub description: String,
    pub kind: String,
    pub transport: String,
    pub auth_method: String,
    pub fields: Vec<ConnectorAuthField>,
    pub enabled: bool,
    /// True when every required secret field has a stored credential (or the
    /// connector needs no auth).
    pub authorized: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_case: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<String>,
}

/// Resolved API connector ready for the `api_connector` agent tool.
#[derive(Debug, Clone)]
pub struct ApiConnectorEntry {
    pub id: String,
    pub name: String,
    pub url: String,
    pub method: String,
    pub use_case: String,
    pub parameters: String,
    pub api_key: String,
}

// ─── Paths ───────────────────────────────────────────────────────────────────

fn connectors_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(resolve_global_config_dir(app)?.join("connectors"))
}

fn safe_id(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .to_lowercase()
}

fn credentials_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(safe_id(id)).join("credentials.json")
}

fn load_credentials(dir: &Path, id: &str) -> HashMap<String, String> {
    let path = credentials_path(dir, id);
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str::<HashMap<String, String>>(&t).ok())
        .unwrap_or_default()
}

// Mutations must not treat an unreadable or corrupt credential file as an empty one:
// doing so would silently discard unrelated stored fields on the next save.
fn load_credentials_for_update(dir: &Path, id: &str) -> Result<HashMap<String, String>, String> {
    let path = credentials_path(dir, id);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(_) => return Err("stored connector credentials could not be read".into()),
    };
    serde_json::from_str(&text).map_err(|_| "stored connector credentials are invalid".into())
}

fn is_authorized(manifest: &ConnectorManifest, creds: &HashMap<String, String>) -> bool {
    match manifest.auth.method.as_str() {
        "none" => return true,
        // OAuth2 is authorized while a stored access token has not expired. An expired token
        // counts as unauthorized, so the connector is excluded until the user authorizes again.
        "oauth2" => {
            let has_token = creds.get("access_token").is_some_and(|v| !v.is_empty());
            return has_token && !oauth_token_expired(creds, unix_now());
        }
        _ => {}
    }
    manifest
        .auth
        .fields
        .iter()
        .filter(|f| f.secret)
        .all(|f| creds.get(&f.key).map(|v| !v.is_empty()).unwrap_or(false))
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Seconds before expiry at which a token is already treated as expired, so a request
/// never leaves with a token that dies in flight.
const OAUTH_EXPIRY_SKEW_SECS: i64 = 60;

/// A token whose `expires_at` (unix seconds) has passed. A missing or unparsable value
/// means the provider did not declare an expiry, so the token is not treated as expired.
fn oauth_token_expired(creds: &HashMap<String, String>, now: i64) -> bool {
    creds
        .get("expires_at")
        .and_then(|v| v.trim().parse::<i64>().ok())
        .is_some_and(|expires_at| now + OAUTH_EXPIRY_SKEW_SECS >= expires_at)
}

fn random_hex_token() -> String {
    // Two v4 UUIDs give 64 random hex characters (RFC 7636 allows 43..=128 unreserved chars).
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

/// Fresh PKCE code verifier, kept server-side for the token exchange.
pub(crate) fn new_pkce_verifier() -> String {
    random_hex_token()
}

/// Fresh anti-CSRF state for one authorization attempt.
pub(crate) fn new_oauth_state() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// PKCE S256 code challenge: base64url(SHA-256(verifier)) without padding.
pub(crate) fn pkce_challenge(verifier: &str) -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    use sha2::{Digest, Sha256};
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// Constant-time comparison of the callback `state` with the value we issued.
pub(crate) fn oauth_state_matches(expected: &str, received: &str) -> bool {
    if expected.is_empty() || expected.len() != received.len() {
        return false;
    }
    expected
        .bytes()
        .zip(received.bytes())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

/// Authorization URL for the single connector flow: code response, PKCE S256 and state.
/// Only `https` endpoints are accepted so credentials and codes never travel in clear text.
pub(crate) fn oauth_authorize_url(
    oauth: &ConnectorOAuth,
    state: &str,
    challenge: &str,
) -> Result<String, String> {
    if oauth.client_id.trim().is_empty() {
        return Err("OAuth connector has no client_id".into());
    }
    let mut url = reqwest::Url::parse(oauth.authorize_url.trim())
        .map_err(|e| format!("invalid authorize_url: {e}"))?;
    if url.scheme() != "https" {
        return Err("OAuth authorize_url must use https".into());
    }
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", oauth.client_id.trim())
        .append_pair("redirect_uri", oauth.redirect.trim())
        .append_pair("scope", &oauth.scopes.join(" "))
        .append_pair("state", state)
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256");
    Ok(url.into())
}

/// Store the tokens from a successful exchange. Other stored fields are kept. An absent
/// `expires_in` clears any previous expiry instead of keeping a stale one.
pub(crate) fn store_oauth_tokens(
    dir: &Path,
    id: &str,
    access_token: &str,
    refresh_token: Option<&str>,
    expires_in: Option<i64>,
    now: i64,
) -> Result<(), String> {
    if access_token.trim().is_empty() {
        return Err("token response has no access_token".into());
    }
    let mut creds = load_credentials_for_update(dir, id)?;
    creds.insert("access_token".into(), access_token.to_string());
    match refresh_token.filter(|t| !t.trim().is_empty()) {
        Some(refresh) => {
            creds.insert("refresh_token".into(), refresh.to_string());
        }
        None => {
            creds.remove("refresh_token");
        }
    }
    match expires_in.filter(|s| *s > 0) {
        Some(seconds) => {
            creds.insert("expires_at".into(), (now + seconds).to_string());
        }
        None => {
            creds.remove("expires_at");
        }
    }
    let text = serde_json::to_string_pretty(&creds).map_err(|e| e.to_string())?;
    std::fs::write(credentials_path(dir, id), text).map_err(|e| e.to_string())
}

/// Remove OAuth tokens so a disabled connector cannot be used until it is authorized again.
fn clear_oauth_tokens(dir: &Path, id: &str) -> Result<(), String> {
    let path = credentials_path(dir, id);
    if !path.exists() {
        return Ok(());
    }
    let mut creds = load_credentials_for_update(dir, id)?;
    for key in ["access_token", "refresh_token", "expires_at"] {
        creds.remove(key);
    }
    let text = serde_json::to_string_pretty(&creds).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}

// ─── OAuth authorization code flow (AZ-051) ──────────────────────────────────
//
// `connectors_oauth_start` binds the manifest's loopback redirect before it returns the
// authorization URL, so the browser never lands on a closed port. A single listener accepts
// one callback, checks `state` in constant time, exchanges the code with the PKCE verifier
// and stores the tokens. The outcome is reported through `CONNECTOR_OAUTH_RESULT_EVENT`.
// Provider accounts and live endpoints are not exercised here; tests use a local fake server.

use tauri::Emitter;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

pub const CONNECTOR_OAUTH_RESULT_EVENT: &str = "connectors:oauth-result";
const OAUTH_CALLBACK_TIMEOUT_SECS: u64 = 300;
const OAUTH_REQUEST_TIMEOUT_SECS: u64 = 30;
const CALLBACK_DONE_PAGE: &str = "AgentZ 已完成授权，可以关闭此页面。";
const CALLBACK_FAILED_PAGE: &str = "AgentZ 授权失败，请回到应用查看原因。";

#[derive(Debug, Clone, Serialize)]
pub struct ConnectorOAuthResult {
    pub id: String,
    pub ok: bool,
    pub error: Option<String>,
}

/// One authorization attempt waiting for its redirect. Starting a new attempt aborts the old one.
struct PendingOAuth {
    state: String,
    task: JoinHandle<()>,
}

fn pending_oauth() -> &'static std::sync::Mutex<HashMap<String, PendingOAuth>> {
    static PENDING: std::sync::OnceLock<std::sync::Mutex<HashMap<String, PendingOAuth>>> =
        std::sync::OnceLock::new();
    PENDING.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

/// Token endpoint response (RFC 6749 §5.1). Only the fields this app stores are kept.
#[derive(Debug, Deserialize)]
pub(crate) struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
}

fn parse_token_response(body: &str) -> Result<TokenResponse, String> {
    // Never echo the body: a malformed response may still contain token material.
    let parsed: TokenResponse = serde_json::from_str(body)
        .map_err(|_| "token response is not a valid JSON token object".to_string())?;
    if parsed.access_token.trim().is_empty() {
        return Err("token response has no access_token".into());
    }
    Ok(parsed)
}

/// Token endpoints must be https. Loopback http is accepted only in test builds.
fn validate_token_url(raw: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(raw.trim()).map_err(|e| format!("invalid token_url: {e}"))?;
    let test_loopback = cfg!(test) && url.scheme() == "http" && url.host_str() == Some("127.0.0.1");
    if url.scheme() != "https" && !test_loopback {
        return Err("OAuth token_url must use https".into());
    }
    Ok(url)
}

/// POST an `application/x-www-form-urlencoded` body. The reqwest `form` feature is not enabled,
/// so the body is encoded through `Url::query_pairs_mut`.
async fn post_token_form(token_url: &str, form: &[(&str, &str)]) -> Result<TokenResponse, String> {
    let url = validate_token_url(token_url)?;
    let mut encoder = reqwest::Url::parse("https://form.invalid/").map_err(|e| e.to_string())?;
    encoder.query_pairs_mut().extend_pairs(form.iter().copied());
    let body = encoder.query().unwrap_or_default().to_string();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(OAUTH_REQUEST_TIMEOUT_SECS))
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .post(url)
        .header(reqwest::header::ACCEPT, "application/json")
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
        .send()
        .await
        .map_err(|_| "token request failed".to_string())?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|_| "token response could not be read".to_string())?;
    if !status.is_success() {
        return Err(format!("token endpoint returned HTTP {}", status.as_u16()));
    }
    parse_token_response(&text)
}

async fn exchange_authorization_code(
    oauth: &ConnectorOAuth,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> Result<TokenResponse, String> {
    post_token_form(
        &oauth.token_url,
        &[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", oauth.client_id.trim()),
            ("code_verifier", verifier),
        ],
    )
    .await
}

async fn refresh_access_token(
    oauth: &ConnectorOAuth,
    refresh_token: &str,
) -> Result<TokenResponse, String> {
    post_token_form(
        &oauth.token_url,
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", oauth.client_id.trim()),
        ],
    )
    .await
}

/// Providers may omit `refresh_token` on refresh; the stored one stays valid then.
fn merged_refresh_token(tokens: &TokenResponse, stored: &str) -> String {
    tokens
        .refresh_token
        .clone()
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| stored.to_string())
}

/// Loopback redirect only (`127.0.0.1` or `localhost` with an explicit port), so the callback
/// listener can never be exposed on a non-local interface.
/// Returns (bind address, callback path, redirect URI as declared).
fn loopback_redirect(redirect: &str) -> Result<(String, String, String), String> {
    let declared = redirect.trim();
    let url = reqwest::Url::parse(declared).map_err(|e| format!("invalid redirect: {e}"))?;
    if url.scheme() != "http" {
        return Err("OAuth redirect must be a loopback http URL".into());
    }
    let host = url.host_str().unwrap_or_default();
    if host != "127.0.0.1" && host != "localhost" {
        return Err("OAuth redirect must point at 127.0.0.1 or localhost".into());
    }
    let port = url
        .port()
        .ok_or_else(|| "OAuth redirect needs an explicit port".to_string())?;
    Ok((
        format!("127.0.0.1:{port}"),
        url.path().to_string(),
        declared.to_string(),
    ))
}

/// Parse `GET <path>?query HTTP/1.1` into query parameters. Other methods and paths are refused.
fn parse_callback_request(request: &str, expected_path: &str) -> Result<HashMap<String, String>, String> {
    let line = request.lines().next().unwrap_or_default();
    let mut parts = line.split_whitespace();
    if parts.next() != Some("GET") {
        return Err("callback must be a GET request".into());
    }
    let target = parts
        .next()
        .ok_or_else(|| "callback request has no target".to_string())?;
    let url = reqwest::Url::parse(&format!("http://127.0.0.1{target}"))
        .map_err(|_| "callback target is malformed".to_string())?;
    if url.path() != expected_path {
        return Err("unexpected callback path".into());
    }
    Ok(url.query_pairs().into_owned().collect())
}

async fn write_callback_page(socket: &mut TcpStream, status: &str, message: &'static str) {
    let body = format!(
        "<!doctype html><meta charset=\"utf-8\"><title>AgentZ</title><p>{message}</p>"
    );
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = socket.write_all(head.as_bytes()).await;
    let _ = socket.write_all(body.as_bytes()).await;
    let _ = socket.shutdown().await;
}

async fn reject(socket: &mut TcpStream, reason: String) -> Result<TokenResponse, String> {
    write_callback_page(socket, "400 Bad Request", CALLBACK_FAILED_PAGE).await;
    Err(reason)
}

/// Accept one redirect, validate it and exchange the code. Requests that fail validation
/// never reach the token endpoint.
async fn await_callback_and_exchange(
    listener: TcpListener,
    oauth: &ConnectorOAuth,
    redirect_path: &str,
    redirect_uri: &str,
    state: &str,
    verifier: &str,
) -> Result<TokenResponse, String> {
    let (mut socket, _) = listener
        .accept()
        .await
        .map_err(|_| "callback listener failed".to_string())?;
    let mut buf = vec![0u8; 8192];
    let read = tokio::time::timeout(
        std::time::Duration::from_secs(OAUTH_REQUEST_TIMEOUT_SECS),
        socket.read(&mut buf),
    )
    .await;
    let n = match read {
        Ok(Ok(n)) if n > 0 => n,
        _ => return reject(&mut socket, "callback was not received".into()).await,
    };
    let request = String::from_utf8_lossy(&buf[..n]).into_owned();
    let params = match parse_callback_request(&request, redirect_path) {
        Ok(params) => params,
        Err(reason) => return reject(&mut socket, reason).await,
    };
    let received_state = params.get("state").map(String::as_str).unwrap_or("");
    if !oauth_state_matches(state, received_state) {
        return reject(
            &mut socket,
            "authorization state mismatch; the callback was ignored".into(),
        )
        .await;
    }
    if let Some(error) = params.get("error") {
        // The provider's error code is shown to the user, so keep only a short safe subset.
        let safe: String = error
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .take(64)
            .collect();
        return reject(
            &mut socket,
            format!("authorization was refused by the provider: {safe}"),
        )
        .await;
    }
    let code = match params.get("code").filter(|c| !c.is_empty()) {
        Some(code) => code.clone(),
        None => return reject(&mut socket, "callback has no authorization code".into()).await,
    };
    match exchange_authorization_code(oauth, &code, verifier, redirect_uri).await {
        Ok(tokens) => {
            write_callback_page(&mut socket, "200 OK", CALLBACK_DONE_PAGE).await;
            Ok(tokens)
        }
        Err(reason) => reject(&mut socket, reason).await,
    }
}

fn require_oauth2(manifest: &ConnectorManifest) -> Result<ConnectorOAuth, String> {
    if manifest.auth.method != "oauth2" {
        return Err("connector does not use OAuth2 authorization".into());
    }
    manifest
        .auth
        .oauth
        .clone()
        .ok_or_else(|| "OAuth connector has no oauth settings".to_string())
}

/// Open the authorization URL in the default browser. No shell is involved, so `&` and other
/// URL characters are never interpreted by cmd.exe.
fn open_in_browser(url: &str) -> Result<(), String> {
    let mut command = if cfg!(windows) {
        let mut c = piscis_kernel::proc::std_command("rundll32");
        c.args(["url.dll,FileProtocolHandler", url]);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("open");
        c.arg(url);
        c
    } else {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(url);
        c
    };
    command
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("could not open the browser: {e}"))
}

/// Begin an OAuth2 authorization for one connector. The loopback listener is bound before the
/// browser opens, and the authorization URL is returned for display if opening fails.
#[tauri::command]
pub async fn connectors_oauth_start(app: AppHandle, id: String) -> Result<String, String> {
    let dir = connectors_dir(&app)?;
    let manifest = ConnectorManifest::load(&dir.join(safe_id(&id)).join("connector.json"))?;
    let oauth = require_oauth2(&manifest)?;
    let (bind_addr, redirect_path, redirect_uri) = loopback_redirect(&oauth.redirect)?;
    let verifier = new_pkce_verifier();
    let state = new_oauth_state();
    let authorize_url = oauth_authorize_url(&oauth, &state, &pkce_challenge(&verifier))?;

    // Abort an attempt already in flight for this connector, and wait for it to release the port.
    let previous = {
        let mut map = pending_oauth()
            .lock()
            .map_err(|_| "authorization state unavailable".to_string())?;
        map.remove(&manifest.id)
    };
    if let Some(previous) = previous {
        previous.task.abort();
        let _ = previous.task.await;
    }

    let listener = TcpListener::bind(&bind_addr).await.map_err(|_| {
        format!(
            "the OAuth callback address {bind_addr} is unavailable; close other sign-in windows and retry"
        )
    })?;
    let id_for_task = manifest.id.clone();
    let state_for_task = state.clone();
    let task_app = app.clone();
    let task_dir = dir.clone();
    let task_oauth = oauth.clone();
    let task = tokio::spawn(async move {
        let outcome = tokio::time::timeout(
            std::time::Duration::from_secs(OAUTH_CALLBACK_TIMEOUT_SECS),
            await_callback_and_exchange(
                listener,
                &task_oauth,
                &redirect_path,
                &redirect_uri,
                &state_for_task,
                &verifier,
            ),
        )
        .await;
        let result: Result<(), String> = match outcome {
            Ok(Ok(tokens)) => store_oauth_tokens(
                &task_dir,
                &id_for_task,
                &tokens.access_token,
                tokens.refresh_token.as_deref(),
                tokens.expires_in,
                unix_now(),
            ),
            Ok(Err(reason)) => Err(reason),
            Err(_) => Err("authorization timed out before the browser redirected back".into()),
        };
        // Forget this attempt only if it is still the registered one.
        if let Ok(mut map) = pending_oauth().lock() {
            if map
                .get(&id_for_task)
                .is_some_and(|p| p.state == state_for_task)
            {
                map.remove(&id_for_task);
            }
        }
        let ok = result.is_ok();
        let error = result.err();
        let _ = task_app.emit(
            CONNECTOR_OAUTH_RESULT_EVENT,
            ConnectorOAuthResult {
                id: id_for_task,
                ok,
                error,
            },
        );
    });

    {
        let mut map = pending_oauth()
            .lock()
            .map_err(|_| "authorization state unavailable".to_string())?;
        map.insert(manifest.id.clone(), PendingOAuth { state, task });
    }
    if let Err(reason) = open_in_browser(&authorize_url) {
        // The listener stays armed; the URL is returned so the user can open it by hand.
        warn!("connector OAuth browser launch failed: {reason}");
    }
    Ok(authorize_url)
}

/// Refresh a connector's access token with its stored refresh token.
#[tauri::command]
pub async fn connectors_oauth_refresh(app: AppHandle, id: String) -> Result<(), String> {
    let dir = connectors_dir(&app)?;
    let manifest = ConnectorManifest::load(&dir.join(safe_id(&id)).join("connector.json"))?;
    let oauth = require_oauth2(&manifest)?;
    let creds = load_credentials_for_update(&dir, &manifest.id)?;
    let stored = creds
        .get("refresh_token")
        .filter(|t| !t.is_empty())
        .cloned()
        .ok_or_else(|| "no refresh token is stored; authorize the connector again".to_string())?;
    let tokens = refresh_access_token(&oauth, &stored).await?;
    let next_refresh = merged_refresh_token(&tokens, &stored);
    store_oauth_tokens(
        &dir,
        &manifest.id,
        &tokens.access_token,
        Some(&next_refresh),
        tokens.expires_in,
        unix_now(),
    )
}

#[cfg(test)]
mod oauth_exchange_tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    fn oauth_for(token_url: &str, redirect: &str) -> ConnectorOAuth {
        ConnectorOAuth {
            authorize_url: "https://auth.example.test/authorize".into(),
            token_url: token_url.into(),
            client_id: "agentz-client".into(),
            scopes: vec!["read".into()],
            redirect: redirect.into(),
        }
    }

    fn request_is_complete(raw: &[u8]) -> bool {
        let text = String::from_utf8_lossy(raw);
        let Some((head, body)) = text.split_once("\r\n\r\n") else {
            return false;
        };
        let length = head
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-length:")
                    .and_then(|v| v.trim().parse::<usize>().ok())
            })
            .unwrap_or(0);
        body.len() >= length
    }

    /// One-shot HTTP token endpoint. Resolves to the raw request it received.
    async fn fake_token_endpoint(body: &'static str) -> (String, JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/token", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut raw = Vec::new();
            let mut chunk = [0u8; 4096];
            while !request_is_complete(&raw) {
                let n = socket.read(&mut chunk).await.unwrap();
                if n == 0 {
                    break;
                }
                raw.extend_from_slice(&chunk[..n]);
            }
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            let _ = socket.shutdown().await;
            String::from_utf8_lossy(&raw).into_owned()
        });
        (url, task)
    }

    #[tokio::test]
    async fn code_exchange_sends_pkce_verifier_and_parses_tokens() {
        let (token_url, captured) = fake_token_endpoint(
            r#"{"access_token":"at-1","refresh_token":"rt-1","expires_in":3600}"#,
        )
        .await;
        let redirect = "http://127.0.0.1:53682/callback";
        let oauth = oauth_for(&token_url, redirect);
        let tokens = exchange_authorization_code(&oauth, "code-1", "verifier-1", redirect)
            .await
            .unwrap();
        let request = captured.await.unwrap();
        assert!(request.starts_with("POST /token "));
        assert!(request.contains("grant_type=authorization_code"));
        assert!(request.contains("code=code-1"));
        assert!(request.contains("code_verifier=verifier-1"));
        assert!(request.contains("client_id=agentz-client"));
        assert!(request.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A53682%2Fcallback"));
        assert_eq!(tokens.access_token, "at-1");
        assert_eq!(tokens.refresh_token.as_deref(), Some("rt-1"));
        assert_eq!(tokens.expires_in, Some(3600));
    }

    #[tokio::test]
    async fn refresh_sends_stored_token_and_keeps_it_when_provider_omits_a_new_one() {
        let (token_url, captured) =
            fake_token_endpoint(r#"{"access_token":"at-new","expires_in":60}"#).await;
        let oauth = oauth_for(&token_url, "http://127.0.0.1:53682/callback");
        let tokens = refresh_access_token(&oauth, "rt-old").await.unwrap();
        let request = captured.await.unwrap();
        assert!(request.contains("grant_type=refresh_token"));
        assert!(request.contains("refresh_token=rt-old"));
        assert_eq!(tokens.access_token, "at-new");
        assert_eq!(merged_refresh_token(&tokens, "rt-old"), "rt-old");
    }

    #[tokio::test]
    async fn callback_with_matching_state_exchanges_the_code() {
        let (token_url, captured) = fake_token_endpoint(r#"{"access_token":"at-2"}"#).await;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let redirect = format!("http://{addr}/callback");
        let oauth = oauth_for(&token_url, &redirect);
        let server_redirect = redirect.clone();
        let server = tokio::spawn(async move {
            await_callback_and_exchange(
                listener,
                &oauth,
                "/callback",
                &server_redirect,
                "state-ok",
                "verifier-1",
            )
            .await
        });
        let mut client = TcpStream::connect(addr).await.unwrap();
        client
            .write_all(b"GET /callback?code=code-1&state=state-ok HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .await
            .unwrap();
        let mut page = String::new();
        client.read_to_string(&mut page).await.unwrap();
        assert!(page.starts_with("HTTP/1.1 200 OK"));
        let tokens = server.await.unwrap().unwrap();
        assert_eq!(tokens.access_token, "at-2");
        assert!(captured.await.unwrap().contains("code=code-1"));
    }

    #[tokio::test]
    async fn callback_with_wrong_state_is_rejected_before_any_exchange() {
        // Port 9 (discard) would refuse a connection; reaching it would fail with a
        // token-request error instead of the state error asserted below.
        let oauth = oauth_for("http://127.0.0.1:9/token", "http://127.0.0.1/callback");
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            await_callback_and_exchange(
                listener,
                &oauth,
                "/callback",
                "http://127.0.0.1/callback",
                "state-ok",
                "verifier-1",
            )
            .await
        });
        let mut client = TcpStream::connect(addr).await.unwrap();
        client
            .write_all(b"GET /callback?code=code-1&state=attacker HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .await
            .unwrap();
        let mut page = String::new();
        client.read_to_string(&mut page).await.unwrap();
        assert!(page.starts_with("HTTP/1.1 400"));
        let error = server.await.unwrap().unwrap_err();
        assert!(error.contains("state mismatch"), "{error}");
    }

    #[test]
    fn callback_request_must_be_get_on_the_registered_path() {
        assert!(parse_callback_request("POST /callback?code=x HTTP/1.1", "/callback").is_err());
        assert!(parse_callback_request("GET /other?code=x HTTP/1.1", "/callback").is_err());
        let params =
            parse_callback_request("GET /callback?code=a%20b&state=s HTTP/1.1", "/callback")
                .unwrap();
        assert_eq!(params["code"], "a b");
        assert_eq!(params["state"], "s");
    }

    #[test]
    fn redirect_must_be_loopback_with_a_port() {
        assert!(loopback_redirect("http://example.test:53682/callback").is_err());
        assert!(loopback_redirect("https://127.0.0.1:53682/callback").is_err());
        assert!(loopback_redirect("http://127.0.0.1/callback").is_err());
        let (bind, path, uri) = loopback_redirect("http://localhost:53682/callback").unwrap();
        assert_eq!(bind, "127.0.0.1:53682");
        assert_eq!(path, "/callback");
        assert_eq!(uri, "http://localhost:53682/callback");
    }

    #[test]
    fn token_endpoint_must_be_https_outside_tests() {
        assert!(validate_token_url("https://auth.example.test/token").is_ok());
        assert!(validate_token_url("http://auth.example.test/token").is_err());
    }

    #[test]
    fn malformed_token_response_never_echoes_its_body() {
        let body = r#"{"access_token":"   "}"#;
        let error = parse_token_response(body).unwrap_err();
        assert_eq!(error, "token response has no access_token");
        let leaky = "not json but contains sk-live-leak";
        let error = parse_token_response(leaky).unwrap_err();
        assert!(!error.contains("sk-live-leak"));
    }
}

#[cfg(test)]
mod oauth_flow_tests {
    use super::*;

    fn oauth() -> ConnectorOAuth {
        ConnectorOAuth {
            authorize_url: "https://auth.example.test/authorize".into(),
            token_url: "https://auth.example.test/token".into(),
            client_id: "agentz-client".into(),
            scopes: vec!["read".into(), "write".into()],
            redirect: "http://127.0.0.1:53682/callback".into(),
        }
    }

    fn creds(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn pkce_challenge_matches_rfc7636_vector() {
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn generated_verifier_and_state_are_fresh_and_well_formed() {
        let (a, b) = (new_pkce_verifier(), new_pkce_verifier());
        assert_ne!(a, b);
        assert!((43..=128).contains(&a.len()));
        assert_ne!(new_oauth_state(), new_oauth_state());
    }

    #[test]
    fn state_must_match_exactly() {
        assert!(oauth_state_matches("abc123", "abc123"));
        assert!(!oauth_state_matches("abc123", "abc124"));
        assert!(!oauth_state_matches("abc123", "abc1234"));
        assert!(!oauth_state_matches("", ""));
    }

    #[test]
    fn authorize_url_carries_code_flow_pkce_and_state() {
        let url = oauth_authorize_url(&oauth(), "state-1", "challenge-1").unwrap();
        let parsed = reqwest::Url::parse(&url).unwrap();
        let q: HashMap<String, String> = parsed.query_pairs().into_owned().collect();
        assert_eq!(q["response_type"], "code");
        assert_eq!(q["client_id"], "agentz-client");
        assert_eq!(q["state"], "state-1");
        assert_eq!(q["code_challenge"], "challenge-1");
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["scope"], "read write");
    }

    #[test]
    fn authorize_url_refuses_cleartext_or_missing_client() {
        let mut plain = oauth();
        plain.authorize_url = "http://auth.example.test/authorize".into();
        assert!(oauth_authorize_url(&plain, "s", "c").is_err());
        let mut anonymous = oauth();
        anonymous.client_id = "  ".into();
        assert!(oauth_authorize_url(&anonymous, "s", "c").is_err());
    }

    #[test]
    fn expired_token_is_not_authorized_and_undated_token_is_not_expired() {
        let now = 1_000_000;
        assert!(!oauth_token_expired(
            &creds(&[("expires_at", "1000600")]),
            now
        ));
        // Within the skew window the token already counts as expired.
        assert!(oauth_token_expired(
            &creds(&[("expires_at", "1000030")]),
            now
        ));
        assert!(oauth_token_expired(
            &creds(&[("expires_at", "999000")]),
            now
        ));
        assert!(!oauth_token_expired(&creds(&[]), now));
        assert!(!oauth_token_expired(&creds(&[("expires_at", "soon")]), now));
    }

    #[test]
    fn store_and_clear_tokens_keep_unrelated_fields() {
        let dir = std::env::temp_dir().join(format!("agentz-oauth-{}", uuid::Uuid::new_v4()));
        let id = "demo";
        std::fs::create_dir_all(dir.join(id)).unwrap();
        std::fs::write(credentials_path(&dir, id), r#"{"unrelated":"kept"}"#).unwrap();

        store_oauth_tokens(&dir, id, "tok-1", Some("ref-1"), Some(3600), 1_000).unwrap();
        let stored = load_credentials(&dir, id);
        assert_eq!(stored["access_token"], "tok-1");
        assert_eq!(stored["refresh_token"], "ref-1");
        assert_eq!(stored["expires_at"], "4600");
        assert_eq!(stored["unrelated"], "kept");

        // A later response without expiry must not keep the old expiry.
        store_oauth_tokens(&dir, id, "tok-2", None, None, 2_000).unwrap();
        let stored = load_credentials(&dir, id);
        assert!(!stored.contains_key("expires_at"));
        assert!(!stored.contains_key("refresh_token"));

        clear_oauth_tokens(&dir, id).unwrap();
        let cleared = load_credentials(&dir, id);
        assert!(!cleared.contains_key("access_token"));
        assert_eq!(cleared["unrelated"], "kept");
        assert!(store_oauth_tokens(&dir, id, "  ", None, None, 0).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn corrupt_credentials_are_not_overwritten_by_token_update() {
        let dir = std::env::temp_dir().join(format!("agentz-oauth-{}", uuid::Uuid::new_v4()));
        let id = "demo";
        std::fs::create_dir_all(dir.join(id)).unwrap();
        let path = credentials_path(&dir, id);
        std::fs::write(&path, "{broken").unwrap();
        assert!(store_oauth_tokens(&dir, id, "tok", None, None, 0).is_err());
        assert!(clear_oauth_tokens(&dir, id).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{broken");
        std::fs::remove_dir_all(dir).unwrap();
    }
}

/// Replace `${cred:<id>.<field>}` placeholders in `input` with credential
/// values for this connector.
fn resolve_placeholders(input: &str, id: &str, creds: &HashMap<String, String>) -> String {
    let mut out = input.to_string();
    let prefix = format!("${{cred:{}.", id);
    while let Some(start) = out.find(&prefix) {
        if let Some(rel_end) = out[start..].find('}') {
            let end = start + rel_end;
            let field = &out[start + prefix.len()..end];
            let value = creds.get(field).cloned().unwrap_or_default();
            out.replace_range(start..=end, &value);
        } else {
            break;
        }
    }
    out
}

fn list_manifest_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() && p.join("connector.json").exists() {
                dirs.push(p);
            }
        }
    }
    dirs
}

fn connector_info_from(
    manifest: &ConnectorManifest,
    creds: &HashMap<String, String>,
) -> ConnectorInfo {
    let api_extra = manifest.kind == "api";
    ConnectorInfo {
        authorized: is_authorized(manifest, creds),
        id: manifest.id.clone(),
        name: manifest.name.clone(),
        icon: manifest.icon.clone(),
        color: manifest.color.clone(),
        category: manifest.category.clone(),
        description: manifest.description.clone(),
        kind: manifest.kind.clone(),
        transport: manifest.transport.clone(),
        auth_method: manifest.auth.method.clone(),
        fields: manifest.auth.fields.clone(),
        enabled: manifest.enabled,
        url: api_extra
            .then(|| manifest.url.clone())
            .filter(|u| !u.is_empty()),
        use_case: api_extra
            .then(|| manifest.use_case.clone())
            .filter(|u| !u.is_empty()),
        parameters: api_extra
            .then(|| manifest.parameters.clone())
            .filter(|p| !p.is_empty()),
    }
}

/// List enabled + authorized `kind: api` connectors for the agent tool.
pub fn list_api_connectors(config_dir: &Path) -> Vec<ApiConnectorEntry> {
    let dir = config_dir.join("connectors");
    let mut out = Vec::new();
    for mdir in list_manifest_dirs(&dir) {
        let manifest = match ConnectorManifest::load(&mdir.join("connector.json")) {
            Ok(m) => m,
            Err(e) => {
                warn!("skip connector at {}: {}", mdir.display(), e);
                continue;
            }
        };
        if manifest.kind != "api" || !manifest.enabled {
            continue;
        }
        let creds = load_credentials(&dir, &manifest.id);
        if !is_authorized(&manifest, &creds) || manifest.url.trim().is_empty() {
            continue;
        }
        let api_key = creds.get("api_key").cloned().unwrap_or_default();
        out.push(ApiConnectorEntry {
            id: manifest.id.clone(),
            name: manifest.name.clone(),
            url: manifest.url.clone(),
            method: if manifest.api_method.trim().is_empty() {
                "POST".into()
            } else {
                manifest.api_method.clone()
            },
            use_case: manifest.use_case.clone(),
            parameters: manifest.parameters.clone(),
            api_key,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Call an enabled API connector. `body` is sent as JSON for POST/PUT.
pub async fn call_api_connector(
    config_dir: &Path,
    connector_id: &str,
    body: Option<serde_json::Value>,
) -> Result<String, String> {
    let entry = list_api_connectors(config_dir)
        .into_iter()
        .find(|c| c.id == connector_id)
        .ok_or_else(|| format!("API connector '{connector_id}' not found or not enabled"))?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;

    let method = entry.method.to_uppercase();
    let mut req = match method.as_str() {
        "GET" => client.get(&entry.url),
        "PUT" => client.put(&entry.url),
        "PATCH" => client.patch(&entry.url),
        "DELETE" => client.delete(&entry.url),
        _ => client.post(&entry.url),
    };
    req = req
        .header("User-Agent", "AgentZ/1.0")
        .header("Accept", "application/json, text/plain, */*");
    if !entry.api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", entry.api_key));
    }
    if method != "GET" && method != "DELETE" {
        req = req.json(&body.unwrap_or(serde_json::json!({})));
    }

    let resp = req
        .send()
        .await
        .map_err(|_| "connector request failed".to_string())?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("connector HTTP {status}"));
    }
    let text = resp
        .text()
        .await
        .map_err(|_| "read connector body failed".to_string())?;
    Ok(text)
}

// ─── Public resolver (used by chat turn + IM loop) ──────────────────────────

/// Resolve every enabled + authorized connector into a kernel MCP config so the
/// caller can register their tools alongside regular MCP servers. `config_dir`
/// is the global config directory (`{config}`).
/// Turn one authorized `kind: mcp` connector manifest into a kernel MCP config.
/// Returns `None` for non-MCP or unauthorized connectors.
fn connector_to_mcp_config(dir: &Path, manifest: &ConnectorManifest) -> Option<McpServerConfig> {
    if manifest.kind != "mcp" {
        return None;
    }
    let creds = load_credentials(dir, &manifest.id);
    if !is_authorized(manifest, &creds) {
        return None;
    }
    let env: HashMap<String, String> = manifest
        .env
        .iter()
        .map(|(k, v)| (k.clone(), resolve_placeholders(v, &manifest.id, &creds)))
        .collect();
    let mut headers: HashMap<String, String> = manifest
        .headers
        .iter()
        .map(|(k, v)| (k.clone(), resolve_placeholders(v, &manifest.id, &creds)))
        .collect();
    // For OAuth2 connectors, auto-inject the stored access token as a Bearer
    // header unless the manifest declares its own Authorization header.
    if manifest.auth.method == "oauth2"
        && !headers
            .keys()
            .any(|k| k.eq_ignore_ascii_case("authorization"))
    {
        if let Some(token) = creds.get("access_token").filter(|v| !v.is_empty()) {
            headers.insert("Authorization".into(), format!("Bearer {token}"));
        }
    }
    let url = resolve_placeholders(&manifest.url, &manifest.id, &creds);
    Some(McpServerConfig {
        name: manifest.id.clone(),
        transport: manifest.transport.clone(),
        command: manifest.command.clone(),
        args: manifest.args.clone(),
        url,
        env,
        headers,
        enabled: true,
    })
}

pub fn resolve_connector_mcp_configs(config_dir: &Path) -> Vec<McpServerConfig> {
    let dir = config_dir.join("connectors");
    let mut out = Vec::new();
    for mdir in list_manifest_dirs(&dir) {
        let manifest = match ConnectorManifest::load(&mdir.join("connector.json")) {
            Ok(m) => m,
            Err(e) => {
                warn!("skip connector at {}: {}", mdir.display(), e);
                continue;
            }
        };
        if !manifest.enabled {
            continue;
        }
        if let Some(cfg) = connector_to_mcp_config(&dir, &manifest) {
            out.push(cfg);
        }
    }
    out
}

/// Resolve specific connectors by id into MCP configs, regardless of their
/// global `enabled` flag (an agent explicitly opted into them). Authorization
/// is still required. Used to bind an agent's own connectors for its turn.
pub fn resolve_named_connector_mcp_configs(
    config_dir: &Path,
    ids: &[String],
) -> Vec<McpServerConfig> {
    let wanted: std::collections::HashSet<&str> = ids
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if wanted.is_empty() {
        return Vec::new();
    }
    let dir = config_dir.join("connectors");
    let mut out = Vec::new();
    for mdir in list_manifest_dirs(&dir) {
        let manifest = match ConnectorManifest::load(&mdir.join("connector.json")) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if !wanted.contains(manifest.id.as_str()) {
            continue;
        }
        if let Some(cfg) = connector_to_mcp_config(&dir, &manifest) {
            out.push(cfg);
        }
    }
    out
}

/// System-prompt section for user-selected connectors (generic agent mode).
pub fn connectors_prompt_context(config_dir: &Path, ids: &[String]) -> Option<String> {
    let wanted: std::collections::HashSet<&str> = ids
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if wanted.is_empty() {
        return None;
    }
    let dir = config_dir.join("connectors");
    let mut lines = Vec::new();
    for mdir in list_manifest_dirs(&dir) {
        let manifest = match ConnectorManifest::load(&mdir.join("connector.json")) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if !wanted.contains(manifest.id.as_str()) {
            continue;
        }
        let creds = load_credentials(&dir, &manifest.id);
        let auth_note = if is_authorized(&manifest, &creds) {
            ""
        } else {
            " (not authorized — ask the user to complete setup in Settings → Connectors)"
        };
        let usage = if manifest.kind == "api" {
            "Use the `api_connector` tool with this connector_id."
        } else {
            "Its MCP tools are registered for this turn."
        };
        lines.push(format!(
            "- **{}** (id: `{}`, kind: {}){} — {}\n  {}",
            manifest.name,
            manifest.id,
            manifest.kind,
            auth_note,
            manifest.description.trim(),
            usage
        ));
    }
    if lines.is_empty() {
        return None;
    }
    Some(format!(
        "## Enabled connectors (user-selected for this conversation)\n\
         The user explicitly enabled these connectors. Use only them:\n{}",
        lines.join("\n")
    ))
}

// ─── Commands ──────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn connectors_list(app: AppHandle) -> Result<Vec<ConnectorInfo>, String> {
    let dir = connectors_dir(&app)?;
    let mut infos = Vec::new();
    for mdir in list_manifest_dirs(&dir) {
        let manifest = match ConnectorManifest::load(&mdir.join("connector.json")) {
            Ok(m) => m,
            Err(e) => {
                warn!("skip connector at {}: {}", mdir.display(), e);
                continue;
            }
        };
        let creds = load_credentials(&dir, &manifest.id);
        infos.push(connector_info_from(&manifest, &creds));
    }
    infos.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(infos)
}

/// Install a connector from a local `connector.json` path, a directory holding
/// one, or an HTTPS URL to a raw `connector.json`.
#[tauri::command]
pub async fn connectors_install(app: AppHandle, source: String) -> Result<ConnectorInfo, String> {
    let dir = connectors_dir(&app)?;
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| e.to_string())?;

    let manifest_text = if source.starts_with("http://") || source.starts_with("https://") {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|e| e.to_string())?;
        client
            .get(&source)
            .header("User-Agent", "AgentZ-Desktop/1.0")
            .send()
            .await
            .map_err(|e| format!("Download error: {e}"))?
            .text()
            .await
            .map_err(|e| format!("Read error: {e}"))?
    } else {
        let p = PathBuf::from(&source);
        let file = if p.is_dir() {
            p.join("connector.json")
        } else {
            p
        };
        std::fs::read_to_string(&file).map_err(|e| e.to_string())?
    };

    let mut manifest: ConnectorManifest =
        serde_json::from_str(&manifest_text).map_err(|e| format!("invalid connector.json: {e}"))?;
    if manifest.id.trim().is_empty() {
        return Err("connector.json must declare a non-empty 'id'".into());
    }
    // Installed connectors start disabled until the user authorizes + enables.
    manifest.enabled = false;

    let target = dir.join(safe_id(&manifest.id));
    tokio::fs::create_dir_all(&target)
        .await
        .map_err(|e| e.to_string())?;
    let pretty = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    tokio::fs::write(target.join("connector.json"), pretty)
        .await
        .map_err(|e| e.to_string())?;

    info!("Installed connector '{}'", manifest.id);
    let creds = load_credentials(&dir, &manifest.id);
    Ok(connector_info_from(&manifest, &creds))
}

#[derive(Debug, Deserialize)]
pub struct CreateApiConnectorRequest {
    pub id: String,
    pub name: String,
    pub url: String,
    pub api_key: String,
    #[serde(default)]
    pub use_case: String,
    #[serde(default)]
    pub parameters: String,
    #[serde(default = "default_api_method")]
    pub method: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub description: String,
}

/// Create an inline HTTP API connector (video / ASR / TTS / OCR / custom).
#[tauri::command]
pub async fn connectors_create_api(
    app: AppHandle,
    req: CreateApiConnectorRequest,
) -> Result<ConnectorInfo, String> {
    let dir = connectors_dir(&app)?;
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| e.to_string())?;

    let id = safe_id(req.id.trim());
    if id.is_empty() {
        return Err("connector id is required".into());
    }
    if req.url.trim().is_empty() {
        return Err("API URL is required".into());
    }
    if req.api_key.trim().is_empty() {
        return Err("API key is required".into());
    }

    let manifest = ConnectorManifest {
        id: id.clone(),
        name: req.name.trim().to_string(),
        icon: if req.icon.trim().is_empty() {
            "🔌".into()
        } else {
            req.icon.trim().to_string()
        },
        color: String::new(),
        category: if req.category.trim().is_empty() {
            "api".into()
        } else {
            req.category.trim().to_string()
        },
        description: req.description.trim().to_string(),
        kind: "api".into(),
        transport: "http".into(),
        command: String::new(),
        args: Vec::new(),
        url: req.url.trim().to_string(),
        env: HashMap::new(),
        headers: HashMap::new(),
        auth: ConnectorAuth {
            method: "api_key".into(),
            fields: vec![ConnectorAuthField {
                key: "api_key".into(),
                label: "API Key".into(),
                secret: true,
                placeholder: String::new(),
            }],
            oauth: None,
        },
        enabled: false,
        api_method: req.method.trim().to_uppercase(),
        use_case: req.use_case.trim().to_string(),
        parameters: req.parameters.trim().to_string(),
    };

    let target = dir.join(&id);
    tokio::fs::create_dir_all(&target)
        .await
        .map_err(|e| e.to_string())?;
    let pretty = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    tokio::fs::write(target.join("connector.json"), pretty)
        .await
        .map_err(|e| e.to_string())?;

    let creds = HashMap::from([("api_key".into(), req.api_key.trim().to_string())]);
    let cred_text = serde_json::to_string_pretty(&creds).map_err(|e| e.to_string())?;
    tokio::fs::write(credentials_path(&dir, &id), cred_text)
        .await
        .map_err(|e| e.to_string())?;

    info!("Created API connector '{}'", id);
    Ok(connector_info_from(&manifest, &creds))
}

#[tauri::command]
pub async fn connectors_uninstall(app: AppHandle, id: String) -> Result<(), String> {
    let dir = connectors_dir(&app)?;
    let target = dir.join(safe_id(&id));
    if !target.exists() {
        return Err(format!("connector '{id}' not found"));
    }
    let canonical = target.canonicalize().map_err(|e| e.to_string())?;
    let base = dir.canonicalize().unwrap_or_else(|_| dir.clone());
    if !canonical.starts_with(&base) {
        return Err("path traversal blocked".into());
    }
    tokio::fs::remove_dir_all(&target)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn connectors_set_enabled(
    app: AppHandle,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    let dir = connectors_dir(&app)?;
    let path = dir.join(safe_id(&id)).join("connector.json");
    let mut manifest = ConnectorManifest::load(&path)?;
    // Refuse to enable an unauthorized connector.
    if enabled {
        let creds = load_credentials_for_update(&dir, &manifest.id)?;
        if !is_authorized(&manifest, &creds) {
            return Err("connector is not authorized yet (missing credentials)".into());
        }
    }
    manifest.enabled = enabled;
    let pretty = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    std::fs::write(&path, pretty).map_err(|e| e.to_string())?;
    // Disabling an OAuth connector drops its tokens; re-enabling requires a new authorization.
    if !enabled && manifest.auth.method == "oauth2" {
        clear_oauth_tokens(&dir, &manifest.id)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn connectors_save_credentials(
    app: AppHandle,
    id: String,
    credentials: HashMap<String, String>,
) -> Result<(), String> {
    let dir = connectors_dir(&app)?;
    let target = dir.join(safe_id(&id));
    if !target.join("connector.json").exists() {
        return Err(format!("connector '{id}' not found"));
    }
    // Merge with existing so masked/blank fields don't clobber stored secrets.
    let mut existing = load_credentials_for_update(&dir, &id)?;
    for (k, v) in credentials {
        if v.is_empty() || v == "••••••••" {
            continue;
        }
        existing.insert(k, v);
    }
    let text = serde_json::to_string_pretty(&existing).map_err(|e| e.to_string())?;
    std::fs::write(credentials_path(&dir, &id), text).map_err(|e| e.to_string())?;
    Ok(())
}

/// Return stored credential keys with secret values masked.
#[tauri::command]
pub async fn connectors_get_credentials(
    app: AppHandle,
    id: String,
) -> Result<HashMap<String, String>, String> {
    let dir = connectors_dir(&app)?;
    let creds = load_credentials_for_update(&dir, &id)?;
    let manifest = ConnectorManifest::load(&dir.join(safe_id(&id)).join("connector.json"))?;
    let public_keys: std::collections::HashSet<&str> = manifest
        .auth
        .fields
        .iter()
        .filter(|f| !f.secret)
        .map(|f| f.key.as_str())
        .collect();
    Ok(mask_credentials(creds, &public_keys))
}

fn mask_credentials(
    creds: HashMap<String, String>,
    public_keys: &std::collections::HashSet<&str>,
) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for (k, v) in creds {
        if (!public_keys.contains(k.as_str()) && k != "expires_at")
            || matches!(k.as_str(), "access_token" | "refresh_token")
        {
            out.insert(
                k,
                if v.is_empty() {
                    String::new()
                } else {
                    "••••••••".into()
                },
            );
        } else {
            out.insert(k, v);
        }
    }
    out
}

#[cfg(test)]
mod credential_projection_tests {
    use super::*;

    #[test]
    fn oauth_tokens_are_masked_even_without_manifest_secret_fields() {
        let creds = HashMap::from([
            ("access_token".into(), "access-marker".into()),
            ("refresh_token".into(), "refresh-marker".into()),
            ("expires_at".into(), "12345".into()),
            ("api_key".into(), "api-marker".into()),
            ("custom_secret".into(), "custom-marker".into()),
            ("username".into(), "alice".into()),
        ]);
        let public_keys = std::collections::HashSet::from(["username", "access_token"]);
        let projected = mask_credentials(creds, &public_keys);
        assert_eq!(projected["access_token"], "••••••••");
        assert_eq!(projected["refresh_token"], "••••••••");
        assert_eq!(projected["api_key"], "••••••••");
        assert_eq!(projected["custom_secret"], "••••••••");
        assert_eq!(projected["username"], "alice");
        assert_eq!(projected["expires_at"], "12345");
    }
}
