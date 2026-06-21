//! Unified marketplace façade (Phase 4).
//!
//! Aggregates discovery + install across the layered tool system into a single
//! uniform surface so the UI can present Tools / Skills / Agents / Teams /
//! Connectors side by side. Each category resolves through one or more sources:
//!
//! - `clawhub` — remote skill registry (search + install), skills only.
//! - `local`   — install from a local path or a raw-manifest/zip URL (all
//!   categories that ship a manifest: tools / agents / teams / connectors).
//! - `builtin` — already-installed items surfaced for management.
//! - `remote`  — theAgentOS cloud marketplace (`/api/marketplace/*`).

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::commands::{agents, clawhub, connectors, teams, user_tools, workbench};

const DEFAULT_CLOUD_BASE: &str = "https://www.dimnuo.com";

#[derive(Debug, Deserialize)]
struct CloudAssetSummary {
    id: String,
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    version: String,
    #[serde(default, rename = "kind")]
    category: String,
    #[serde(default)]
    download_url: Option<String>,
    #[serde(default)]
    channel: Option<String>,
    #[serde(default)]
    signature: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CloudAssetsResponse {
    #[serde(default)]
    assets: Vec<CloudAssetSummary>,
}

fn cloud_base_url() -> Option<String> {
    std::env::var("AGENTZ_CLOUD_BASE_URL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| Some(DEFAULT_CLOUD_BASE.into()))
}

fn desktop_os_name() -> &'static str {
    match std::env::consts::OS {
        "linux" => "linux",
        "macos" => "macos",
        "windows" => "windows",
        _ => "unknown",
    }
}

fn client_profile_query(channel: &str) -> String {
    let os = desktop_os_name();
    let mut caps = vec!["mcp_stdio"];
    if os == "windows" {
        caps.push("com");
    }
    format!(
        "surface=desktop&os={os}&capabilities={}&channel={channel}",
        caps.join(",")
    )
}

fn cloud_kind_for_category(category: &str) -> Option<&'static str> {
    match category {
        "agent" => Some("expert"),
        "skill" => Some("skill"),
        "connector" => Some("connector"),
        "team" => Some("team"),
        "tool" => Some("tool"),
        _ => None,
    }
}

async fn fetch_cloud_asset_payload(asset_id: &str, channel: &str) -> Result<serde_json::Value, String> {
    let url = cloud_asset_url(asset_id)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .user_agent("AgentZ-Desktop/1.0")
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("asset fetch HTTP {}", resp.status()));
    }
    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let payload = body.get("payload").cloned().unwrap_or(body.clone());
    if let Some(sig) = body.get("signature").and_then(|v| v.as_str()) {
        verify_cloud_signature(&payload, sig)?;
    } else if let Some(sig) = payload.get("signature").and_then(|v| v.as_str()) {
        verify_cloud_signature(&payload, sig)?;
    }
    let _ = channel; // channel is selected at listing time via client_profile
    Ok(payload)
}

fn canonical_json(value: &serde_json::Value) -> Result<String, String> {
    match value {
        serde_json::Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut out = String::from("{");
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(k).map_err(|e| e.to_string())?);
                out.push(':');
                out.push_str(&canonical_json(&map[*k])?);
            }
            out.push('}');
            Ok(out)
        }
        serde_json::Value::Array(arr) => {
            let parts: Result<Vec<String>, String> =
                arr.iter().map(canonical_json).collect();
            Ok(format!("[{}]", parts?.join(",")))
        }
        other => serde_json::to_string(other).map_err(|e| e.to_string()),
    }
}

fn verify_cloud_signature(payload: &serde_json::Value, signature: &str) -> Result<(), String> {
    let secret = std::env::var("MARKETPLACE_SIGNING_SECRET")
        .ok()
        .filter(|s| !s.trim().is_empty());
    let Some(secret) = secret else {
        return Ok(());
    };
    let mut to_sign = payload.clone();
    if let Some(obj) = to_sign.as_object_mut() {
        obj.remove("signature");
    }
    let canonical = canonical_json(&to_sign)?;
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).map_err(|e| e.to_string())?;
    mac.update(canonical.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());
    if expected != signature {
        return Err("cloud asset signature mismatch".into());
    }
    Ok(())
}

async fn fetch_cloud_assets(category: &str, query: &str, channel: &str) -> Result<Vec<CloudAssetSummary>, String> {
    let base = cloud_base_url().ok_or_else(|| "cloud marketplace URL not configured".to_string())?;
    let kind = cloud_kind_for_category(category).ok_or_else(|| format!("no cloud kind for {category}"))?;
    let profile = client_profile_query(channel);
    let url = format!(
        "{}/api/marketplace/assets?kind={kind}&{profile}",
        base.trim_end_matches('/')
    );
    let _ = query; // assets endpoint has no text query; filter client-side below
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("AgentZ-Desktop/1.0")
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("cloud marketplace HTTP {}", resp.status()));
    }
    let body: CloudAssetsResponse = resp.json().await.map_err(|e| e.to_string())?;
    Ok(body.assets)
}

fn cloud_asset_url(asset_id: &str) -> Result<String, String> {
    let base = cloud_base_url().ok_or_else(|| "cloud marketplace URL not configured".to_string())?;
    Ok(format!(
        "{}/api/marketplace/asset/{}",
        base.trim_end_matches('/'),
        asset_id
    ))
}

async fn remote_install_connector(app: &AppHandle, asset_id: &str) -> Result<(), String> {
    let payload = fetch_cloud_asset_payload(asset_id, "stable").await?;
    let connector = payload
        .get("connector")
        .cloned()
        .unwrap_or(payload.clone());
    let pretty = serde_json::to_string_pretty(&connector).map_err(|e| e.to_string())?;
    let tmp = std::env::temp_dir().join(format!(
        "agentz-connector-{}.json",
        asset_id.replace('/', "_")
    ));
    tokio::fs::write(&tmp, pretty)
        .await
        .map_err(|e| e.to_string())?;
    connectors::connectors_install(app.clone(), tmp.to_string_lossy().into())
        .await
        .map(|_| ())
}

async fn remote_install_agent(app: &AppHandle, asset_id: &str) -> Result<(), String> {
    let payload = fetch_cloud_asset_payload(asset_id, "stable").await?;
    let manifest = if let Some(m) = payload.get("agent_manifest") {
        m.clone()
    } else {
        let slug = asset_id
            .split('/')
            .nth(2)
            .and_then(|s| s.split('@').next())
            .unwrap_or("agent");
        serde_json::json!({
            "id": slug,
            "name": payload.get("name").and_then(|v| v.as_str()).unwrap_or(slug),
            "description": payload.get("description").and_then(|v| v.as_str()).unwrap_or(""),
            "description_zh": payload.get("description_zh").and_then(|v| v.as_str()).unwrap_or(""),
            "system_prompt": payload.get("system_prompt").and_then(|v| v.as_str()).unwrap_or(""),
            "system_prompt_zh": payload.get("system_prompt_zh").and_then(|v| v.as_str()).unwrap_or(""),
            "icon": payload.get("icon").and_then(|v| v.as_str()).unwrap_or("🤖"),
            "color": payload.get("color").and_then(|v| v.as_str()).unwrap_or("#7c5cff"),
            "tools": payload.get("allowed_tools").cloned().unwrap_or(serde_json::json!([])),
            "skills": payload.get("allowed_skills").cloned().unwrap_or(serde_json::json!([])),
        })
    };
    let pretty = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    let slug = manifest
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("agent");
    let tmp = std::env::temp_dir().join(format!("agentz-agent-{}.json", slug));
    tokio::fs::write(&tmp, pretty)
        .await
        .map_err(|e| e.to_string())?;
    agents::agents_install(app.clone(), tmp.to_string_lossy().into())
        .await
        .map(|_| ())
}

async fn remote_install_skill(app: &AppHandle, asset_id: &str) -> Result<(), String> {
    let payload = fetch_cloud_asset_payload(asset_id, "stable").await?;
    let skill_md = payload
        .get("skill_md")
        .and_then(|v| v.as_str())
        .or_else(|| payload.get("content").and_then(|v| v.as_str()))
        .or_else(|| payload.get("instructions").and_then(|v| v.as_str()))
        .unwrap_or("");
    if skill_md.trim().is_empty() {
        return Err("cloud skill payload missing skill_md".into());
    }
    let config_dir = crate::commands::data_scope::resolve_global_config_dir(app)?;
    let skills_root =
        crate::skills::service::skills_root_from_config_dir(&config_dir);
    crate::skills::provenance::ensure_evolution_dirs(&skills_root).map_err(|e| e.to_string())?;
    let (global_db, _) =
        crate::commands::data_scope::open_global_kernel_state(app).map_err(|e| e.to_string())?;
    let db = global_db.lock().await;
    crate::skills::service::install_to_installed(
        &db,
        &skills_root,
        skill_md,
        "official",
        Some(asset_id.to_string()),
        None,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

async fn remote_install_team(app: &AppHandle, asset_id: &str) -> Result<(), String> {
    teams::teams_install(app.clone(), cloud_asset_url(asset_id)?)
        .await
        .map(|_| ())
}

/// A single, source-agnostic marketplace entry rendered as a card.
#[derive(Debug, Clone, Serialize)]
pub struct MarketItem {
    /// Stable identifier within the category (slug / id / name).
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    /// `tool` | `skill` | `agent` | `team` | `connector`.
    pub category: String,
    /// `clawhub` | `local` | `builtin` | `remote`.
    pub source: String,
    pub icon: String,
    /// Free-form sub-category / tag label (e.g. a connector's category).
    pub tag: String,
    pub stars: u64,
    pub installed: bool,
    /// Only meaningful for connectors; true for everything else.
    pub authorized: bool,
}

impl MarketItem {
    fn base(category: &str, source: &str) -> Self {
        MarketItem {
            id: String::new(),
            name: String::new(),
            description: String::new(),
            version: String::new(),
            category: category.into(),
            source: source.into(),
            icon: String::new(),
            tag: String::new(),
            stars: 0,
            installed: false,
            authorized: true,
        }
    }
}

/// Discover items for one category. `query` only applies to searchable sources
/// (currently ClawHub skills); other categories list what is installed.
#[tauri::command]
pub async fn marketplace_search(
    app: AppHandle,
    category: String,
    query: String,
) -> Result<Vec<MarketItem>, String> {
    match category.as_str() {
        "skill" => search_skills(app, query).await,
        "connector" => search_connectors(app, query).await,
        "tool" => search_remote_or_local(app, "tool", query, list_tools).await,
        "agent" => search_remote_or_local(app, "agent", query, list_agents).await,
        "team" => search_remote_or_local(app, "team", query, list_teams).await,
        other => Err(format!("unknown marketplace category: {other}")),
    }
}

/// Install an item. `source` selects the pipeline; `identifier` is the slug
/// (ClawHub) or the path/URL (local). `version` is only used by ClawHub.
#[tauri::command]
pub async fn marketplace_install(
    app: AppHandle,
    category: String,
    source: String,
    identifier: String,
    version: Option<String>,
) -> Result<(), String> {
    match (category.as_str(), source.as_str()) {
        ("skill", "clawhub") => clawhub::clawhub_install(app, identifier, version, None)
            .await
            .map(|_| ()),
        ("skill", "remote") => remote_install_skill(&app, &identifier).await,
        ("tool", "remote") => remote_install_tool(app, &identifier).await,
        ("agent", "remote") => remote_install_agent(&app, &identifier).await,
        ("team", "remote") => remote_install_team(&app, &identifier).await,
        ("tool", _) => user_tools::user_tools_install(app, identifier)
            .await
            .map(|_| ()),
        ("agent", _) => agents::agents_install(app, identifier).await.map(|_| ()),
        ("team", _) => teams::teams_install(app, identifier).await.map(|_| ()),
        ("connector", "local") => {
            connectors::connectors_install(app, identifier)
                .await
                .map(|_| ())
        }
        ("connector", "remote") => remote_install_connector(&app, &identifier).await,
        ("connector", _) => {
            // Built-in (already-installed) connectors: "install" = enable.
            connectors::connectors_set_enabled(app, identifier, true).await
        }
        (cat, src) => Err(format!("unsupported install: category={cat} source={src}")),
    }
}

/// Uninstall by category, routing to the owning command.
#[tauri::command]
pub async fn marketplace_uninstall(
    app: AppHandle,
    category: String,
    id: String,
) -> Result<(), String> {
    match category.as_str() {
        "skill" => workbench::skills_uninstall(app, id).await,
        "tool" => user_tools::user_tools_uninstall(app, id).await,
        "agent" => agents::agents_uninstall(app, id).await,
        "team" => teams::teams_uninstall(app, id).await,
        "connector" => connectors::connectors_uninstall(app, id).await,
        other => Err(format!("unknown marketplace category: {other}")),
    }
}

// ─── Per-category aggregation ───────────────────────────────────────────────

async fn search_skills(app: AppHandle, query: String) -> Result<Vec<MarketItem>, String> {
    let installed = workbench::skills_list_installed(app.clone())
        .await
        .unwrap_or_default();
    let installed_slugs: std::collections::HashSet<String> =
        installed.iter().map(|s| s.slug.clone()).collect();

    let res = clawhub::clawhub_search(query.clone(), Some(30), None).await?;
    let mut items: Vec<MarketItem> = res
        .items
        .into_iter()
        .map(|s| {
            let mut it = MarketItem::base("skill", "clawhub");
            it.installed = installed_slugs.contains(&s.slug);
            it.id = s.slug;
            it.name = s.name;
            it.description = s.description;
            it.version = s.version;
            it.stars = s.stars;
            it.icon = "🧩".into();
            it.tag = s.tags.first().cloned().unwrap_or_default();
            it
        })
        .collect();

    if let Ok(remote) = fetch_cloud_assets("skill", &query, "stable").await {
        let listed: std::collections::HashSet<String> = items.iter().map(|i| i.id.clone()).collect();
        for a in remote {
            if !query.is_empty()
                && !a.name.to_lowercase().contains(&query.to_lowercase())
                && !a.description.to_lowercase().contains(&query.to_lowercase())
            {
                continue;
            }
            if listed.contains(&a.id) {
                continue;
            }
            let mut it = MarketItem::base("skill", "remote");
            it.id = a.id.clone();
            it.name = a.name;
            it.description = a.description;
            it.version = a.version;
            it.icon = "🧩".into();
            it.tag = if a.category == "official" {
                "official".into()
            } else {
                a.category
            };
            items.push(it);
        }
    }

    // Surface locally-installed skills that aren't in the search results so the
    // user can always manage them from the same tab.
    let listed: std::collections::HashSet<String> = items.iter().map(|i| i.id.clone()).collect();
    for s in installed {
        if !listed.contains(&s.slug) {
            let mut it = MarketItem::base("skill", "local");
            it.installed = true;
            it.id = s.slug;
            it.name = s.name;
            it.description = s.description;
            it.icon = "🧩".into();
            items.push(it);
        }
    }
    Ok(items)
}

async fn search_remote_or_local<F, Fut>(
    app: AppHandle,
    category: &str,
    query: String,
    local_fn: F,
) -> Result<Vec<MarketItem>, String>
where
    F: FnOnce(AppHandle) -> Fut,
    Fut: std::future::Future<Output = Result<Vec<MarketItem>, String>>,
{
    let mut items = local_fn(app.clone()).await.unwrap_or_default();
    if let Ok(remote) = fetch_cloud_assets(category, &query, "stable").await {
        let local_ids: std::collections::HashSet<String> =
            items.iter().map(|i| i.id.clone()).collect();
        for a in remote {
            if query.is_empty()
                || a.name.to_lowercase().contains(&query.to_lowercase())
                || a.description.to_lowercase().contains(&query.to_lowercase())
            {
                if local_ids.contains(&a.id) {
                    continue;
                }
                let mut it = MarketItem::base(category, "remote");
                it.id = a.id.clone();
                it.name = a.name;
                it.description = a.description;
                it.version = a.version;
                it.icon = match category {
                    "agent" => "🤖".into(),
                    "team" => "👥".into(),
                    "tool" => "🛠️".into(),
                    _ => "📦".into(),
                };
                it.tag = a.category;
                items.push(it);
            }
        }
    }
    Ok(items)
}

async fn remote_install_tool(app: AppHandle, asset_id: &str) -> Result<(), String> {
    let payload = fetch_cloud_asset_payload(asset_id, "stable").await?;
    let is_slash = payload
        .get("agentz_kind")
        .and_then(|v| v.as_str())
        == Some("slash_command")
        || payload.get("slash_command").is_some();
    if is_slash {
        let cmd = payload
            .get("slash_command")
            .cloned()
            .unwrap_or(payload);
        let pretty = serde_json::to_string_pretty(&cmd).map_err(|e| e.to_string())?;
        return crate::commands::slash_commands::slash_commands_install(app.clone(), pretty)
            .await
            .map(|_| ());
    }
    user_tools::user_tools_install(app, cloud_asset_url(asset_id)?)
        .await
        .map(|_| ())
}

async fn search_connectors(app: AppHandle, query: String) -> Result<Vec<MarketItem>, String> {
    let mut items = list_connectors(app.clone()).await?;
    if let Ok(remote) = fetch_cloud_assets("connector", &query, "stable").await {
        let local_ids: std::collections::HashSet<String> =
            items.iter().map(|i| i.id.clone()).collect();
        for a in remote {
            if local_ids.contains(&a.id) {
                continue;
            }
            let mut it = MarketItem::base("connector", "remote");
            it.id = a.id;
            it.name = a.name;
            it.description = a.description;
            it.version = a.version;
            it.icon = "🔌".into();
            it.tag = a.category;
            items.push(it);
        }
    }
    Ok(items)
}

async fn list_connectors(app: AppHandle) -> Result<Vec<MarketItem>, String> {
    let infos = connectors::connectors_list(app).await?;
    Ok(infos
        .into_iter()
        .map(|c| {
            let mut it = MarketItem::base("connector", "builtin");
            it.installed = c.enabled;
            it.authorized = c.authorized;
            it.id = c.id;
            it.name = c.name;
            it.description = c.description;
            it.icon = if c.icon.is_empty() {
                "🔌".into()
            } else {
                c.icon
            };
            it.tag = c.category;
            it
        })
        .collect())
}

async fn list_tools(app: AppHandle) -> Result<Vec<MarketItem>, String> {
    let tools = user_tools::user_tools_list(app).await?;
    Ok(tools
        .into_iter()
        .map(|tdef| {
            let mut it = MarketItem::base("tool", "local");
            it.installed = true;
            it.id = tdef.name.clone();
            it.name = tdef.name;
            it.description = tdef.description;
            it.version = tdef.version;
            it.icon = "🛠️".into();
            it.tag = tdef.runtime;
            it
        })
        .collect())
}

async fn list_agents(app: AppHandle) -> Result<Vec<MarketItem>, String> {
    let list = agents::agents_list(app).await?;
    Ok(list
        .into_iter()
        .map(|a| {
            let mut it = MarketItem::base("agent", "local");
            it.installed = true;
            it.id = a.id;
            it.name = a.name;
            it.description = a.description;
            it.icon = if a.icon.is_empty() {
                "🤖".into()
            } else {
                a.icon
            };
            it.tag = a.role;
            it
        })
        .collect())
}

async fn list_teams(app: AppHandle) -> Result<Vec<MarketItem>, String> {
    let list = teams::teams_list(app).await?;
    Ok(list
        .into_iter()
        .map(|tm| {
            let mut it = MarketItem::base("team", "local");
            it.installed = true;
            it.id = tm.id;
            it.name = tm.name;
            it.description = tm.description;
            it.icon = "👥".into();
            it.tag = if tm.mode == "workflow" {
                "workflow".to_string()
            } else {
                tm.workflow_hint
            };
            it
        })
        .collect())
}
