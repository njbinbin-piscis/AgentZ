//! ClawHub marketplace — search and install SKILL.md packages (from openpiscis).

use std::io::Read;

use serde::Serialize;
use tauri::AppHandle;
use tracing::{info, warn};

use crate::commands::chat::resolve_config_dir;

const CLAWHUB_API: &str = "https://clawhub.ai";
const SKILLHUB_API: &str = "https://api.skillhub.cn";
const MAX_SKILL_MD_BYTES: u64 = 1_048_576;

fn resolve_registry(registry: Option<String>) -> Result<&'static str, String> {
    match registry
        .unwrap_or_else(|| "clawhub".into())
        .trim()
        .to_lowercase()
        .as_str()
    {
        "clawhub" => Ok(CLAWHUB_API),
        "skillhub" => Ok(SKILLHUB_API),
        other => Err(format!("unknown skill registry: {other}")),
    }
}

fn resolve_source(registry: Option<String>) -> Result<&'static str, String> {
    match registry
        .unwrap_or_else(|| "clawhub".into())
        .trim()
        .to_lowercase()
        .as_str()
    {
        "clawhub" => Ok("clawhub"),
        "skillhub" => Ok("skillhub"),
        other => Err(format!("unknown skill registry: {other}")),
    }
}

fn registry_label(base: &str) -> &'static str {
    if base == SKILLHUB_API {
        "SkillHub"
    } else {
        "ClawHub"
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ClawHubSkill {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub stars: u64,
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ClawHubSearchResult {
    pub items: Vec<ClawHubSkill>,
    pub total: usize,
    pub query: String,
}

#[derive(Debug, Serialize)]
pub struct ClawHubInstallResult {
    pub slug: String,
    pub name: String,
    pub skill_dir: String,
}

async fn clawhub_get_with_retry(
    client: &reqwest::Client,
    url: &str,
    max_retries: u32,
) -> Result<reqwest::Response, String> {
    let base_delay_ms: u64 = 1000;
    let mut attempt = 0u32;
    loop {
        let resp = client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("network request failed: {e}"))?;
        let status = resp.status();
        if status.is_success() || (status.is_client_error() && status.as_u16() != 429) {
            return Ok(resp);
        }
        if attempt >= max_retries {
            return Ok(resp);
        }
        let retry_after_ms = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .map(|secs| secs * 1000)
            .unwrap_or(0);
        let backoff_ms = if retry_after_ms > 0 {
            retry_after_ms.min(30_000)
        } else {
            (base_delay_ms * (1u64 << attempt.min(4))).min(16_000)
        };
        warn!("skill registry {status} for '{url}', retry in {backoff_ms}ms");
        tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
        attempt += 1;
    }
}

fn sanitize_slug(slug: &str) -> Result<String, String> {
    if !slug
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err(format!("invalid skill slug: '{slug}'"));
    }
    Ok(slug.to_string())
}

fn extract_skill_md_from_zip(zip_bytes: &[u8]) -> Result<String, String> {
    let cursor = std::io::Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
        let file = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = file.name().to_lowercase();
        if name == "skill.md" || name.ends_with("/skill.md") {
            let mut bytes = Vec::new();
            file.take(MAX_SKILL_MD_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() as u64 > MAX_SKILL_MD_BYTES {
                return Err("SKILL.md exceeds the 1 MiB install limit".into());
            }
            let content =
                String::from_utf8(bytes).map_err(|e| format!("SKILL.md is not UTF-8: {e}"))?;
            return Ok(content);
        }
    }
    Err("SKILL.md not found in zip archive".to_string())
}

fn parse_search_hit(r: &serde_json::Value) -> Option<ClawHubSkill> {
    let slug = r["slug"].as_str()?.to_string();
    let stars = r["stars"]
        .as_u64()
        .or_else(|| r["stats"]["stars"].as_u64())
        .unwrap_or(0);
    Some(ClawHubSkill {
        name: r["displayName"]
            .as_str()
            .or_else(|| r["name"].as_str())
            .unwrap_or(&slug)
            .to_string(),
        description: r["summary"]
            .as_str()
            .or_else(|| r["description"].as_str())
            .unwrap_or("")
            .to_string(),
        version: r["version"].as_str().unwrap_or("").to_string(),
        stars,
        tags: vec![],
        slug,
    })
}

fn parse_list_hit(item: &serde_json::Value) -> Option<ClawHubSkill> {
    let slug = item["slug"].as_str()?.to_string();
    let tags: Vec<String> = item["tags"]
        .as_object()
        .map(|obj| obj.keys().cloned().collect())
        .unwrap_or_default();
    Some(ClawHubSkill {
        name: item["displayName"].as_str().unwrap_or(&slug).to_string(),
        description: item["summary"].as_str().unwrap_or("").to_string(),
        version: item["latestVersion"]["version"]
            .as_str()
            .unwrap_or("latest")
            .to_string(),
        stars: item["stats"]["stars"].as_u64().unwrap_or(0),
        tags,
        slug,
    })
}

/// Search a skill registry (ClawHub or SkillHub) for skills.
#[tauri::command]
pub async fn clawhub_search(
    query: String,
    limit: Option<u32>,
    registry: Option<String>,
) -> Result<ClawHubSearchResult, String> {
    let limit = limit.unwrap_or(20).min(50);
    let api_base = resolve_registry(registry.clone())?;
    let label = registry_label(api_base);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("AgentZ-Desktop/1.0")
        .build()
        .map_err(|e| e.to_string())?;

    let q = query.trim().to_string();
    let is_skillhub = api_base == SKILLHUB_API;
    let (url, use_search) = if q.is_empty() {
        if is_skillhub {
            (format!("{api_base}/api/v1/search?q=&limit={limit}"), true)
        } else {
            (
                format!("{api_base}/api/v1/skills?sort=stars&limit={limit}"),
                false,
            )
        }
    } else {
        (
            format!(
                "{api_base}/api/v1/search?q={}&limit={limit}",
                urlencoding::encode(&q)
            ),
            true,
        )
    };

    let resp = clawhub_get_with_retry(&client, &url, 3)
        .await
        .map_err(|e| format!("cannot reach {label}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("{label} HTTP {}", resp.status()));
    }
    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;

    let items: Vec<ClawHubSkill> = if use_search {
        body["results"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(parse_search_hit)
            .collect()
    } else {
        body["items"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(parse_list_hit)
            .collect()
    };

    let total = items.len();
    Ok(ClawHubSearchResult {
        items,
        total,
        query: q,
    })
}

/// Install a skill from ClawHub or SkillHub into `{config_dir}/skills/installed/{slug}/`.
#[tauri::command]
pub async fn clawhub_install(
    app: AppHandle,
    slug: String,
    version: Option<String>,
    registry: Option<String>,
) -> Result<ClawHubInstallResult, String> {
    let slug = sanitize_slug(slug.trim())?;
    let api_base = resolve_registry(registry.clone())?;
    let source = resolve_source(registry)?;
    let label = registry_label(api_base);
    let config_dir = resolve_config_dir(&app)?;
    let skills_root = crate::skills::service::skills_root_from_config_dir(&config_dir);
    crate::skills::provenance::ensure_evolution_dirs(&skills_root).map_err(|e| e.to_string())?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("AgentZ-Desktop/1.0")
        .build()
        .map_err(|e| e.to_string())?;

    let ver = version
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty() && *v != "latest");
    let file_url = if let Some(v) = ver {
        format!("{api_base}/api/v1/skills/{slug}/file?path=SKILL.md&version={v}")
    } else {
        format!("{api_base}/api/v1/skills/{slug}/file?path=SKILL.md")
    };
    info!("{label} install: {file_url}");

    let resp = clawhub_get_with_retry(&client, &file_url, 3).await?;
    let content = if resp.status().is_success() {
        let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_SKILL_MD_BYTES {
            return Err("SKILL.md exceeds the 1 MiB install limit".into());
        }
        String::from_utf8(bytes.to_vec()).map_err(|e| format!("SKILL.md is not UTF-8: {e}"))?
    } else {
        let zip_url = if let Some(v) = ver {
            format!("{api_base}/api/v1/download?slug={slug}&version={v}")
        } else {
            format!("{api_base}/api/v1/download?slug={slug}")
        };
        let zip_resp = clawhub_get_with_retry(&client, &zip_url, 3).await?;
        if !zip_resp.status().is_success() {
            return Err(format!(
                "{label} install failed for '{slug}': HTTP {}",
                zip_resp.status()
            ));
        }
        let bytes = zip_resp.bytes().await.map_err(|e| e.to_string())?;
        if bytes.len() > 8 * 1_048_576 {
            return Err("skill archive exceeds the 8 MiB install limit".into());
        }
        extract_skill_md_from_zip(&bytes)?
    };

    let source_url = Some(format!("{api_base}/api/v1/skills/{slug}/file"));
    let (global_db, _) =
        crate::commands::data_scope::open_global_kernel_state(&app).map_err(|e| e.to_string())?;
    let db = global_db.lock().await;
    let (installed_id, display_name) = crate::skills::service::install_to_installed(
        &db,
        &skills_root,
        &content,
        source,
        source_url,
        ver.map(str::to_string),
        None,
    )
    .map_err(|e| e.to_string())?;
    drop(db);
    let final_dir = crate::skills::provenance::installed_dir(&skills_root).join(&installed_id);
    Ok(ClawHubInstallResult {
        name: display_name,
        slug: installed_id,
        skill_dir: final_dir.display().to_string(),
    })
}

#[cfg(test)]
mod install_archive_tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn archive_install_reads_only_bounded_skill_markdown_without_extracting_files() {
        let cursor = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(cursor);
        zip.start_file("../outside.txt", zip::write::SimpleFileOptions::default())
            .expect("start outside entry");
        zip.write_all(b"must never be written to disk")
            .expect("write outside entry");
        zip.start_file("package/SKILL.md", zip::write::SimpleFileOptions::default())
            .expect("start skill entry");
        zip.write_all(b"---\nname: Example\n---\nSafe.")
            .expect("write skill entry");
        let archive = zip.finish().expect("finish archive").into_inner();

        assert_eq!(
            extract_skill_md_from_zip(&archive).expect("read only skill file"),
            "---\nname: Example\n---\nSafe."
        );
    }

    #[test]
    fn archive_install_rejects_oversized_skill_markdown() {
        let cursor = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(cursor);
        zip.start_file("SKILL.md", zip::write::SimpleFileOptions::default())
            .expect("start skill entry");
        zip.write_all(&vec![b'a'; MAX_SKILL_MD_BYTES as usize + 1])
            .expect("write oversized skill");
        let archive = zip.finish().expect("finish archive").into_inner();

        assert!(extract_skill_md_from_zip(&archive)
            .expect_err("oversize must fail closed")
            .contains("1 MiB"));
    }
}
