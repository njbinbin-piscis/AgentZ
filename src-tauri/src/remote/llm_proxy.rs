//! Remote processes call `http://127.0.0.1:<port>/v1/...` on agentz-server.
//! The server forwards the body here. The API key stays on this machine and
//! the provider request leaves from here, not from the remote host.

use serde_json::{json, Value};
use tauri::AppHandle;

const MAX_BODY: usize = 1_500_000;

pub(crate) async fn forward(app: &AppHandle, path: &str, raw_body: &str) -> (u16, String) {
    if raw_body.len() > MAX_BODY {
        return error(413, "LLM request body is too large");
    }
    let path = path.split('?').next().unwrap_or(path);
    if !path.starts_with("/v1/") || path.contains("..") {
        return error(404, "only POST /v1/* is proxied through AgentZ");
    }
    let mut body: Value = match serde_json::from_str(raw_body) {
        Ok(value) => value,
        Err(_) => return error(400, "LLM request body is not JSON"),
    };
    if let Some(obj) = body.as_object_mut() {
        obj.insert("stream".into(), json!(false));
    }
    let route = match crate::commands::settings::active_llm_route(app) {
        Ok(route) => route,
        Err(message) => return error(401, &message),
    };
    let url = match provider_url(&route.base_url, path) {
        Ok(url) => url,
        Err(message) => return error(400, &message),
    };
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(170))
        .build()
    {
        Ok(client) => client,
        Err(e) => return error(500, &e.to_string()),
    };
    let mut request = client
        .post(&url)
        .header("content-type", "application/json")
        .body(body.to_string());
    request = if route.provider == "anthropic" || url.contains("anthropic.com") {
        request
            .header("x-api-key", &route.api_key)
            .header("anthropic-version", "2023-06-01")
    } else {
        request.bearer_auth(&route.api_key)
    };
    match request.send().await {
        Ok(response) => {
            let status = response.status().as_u16();
            match response.text().await {
                Ok(text) => (status, text),
                Err(e) => error(502, &format!("model response could not be read: {e}")),
            }
        }
        Err(e) => error(
            502,
            &format!("AgentZ could not reach the model endpoint: {e}"),
        ),
    }
}

pub(crate) fn provider_url(base: &str, path: &str) -> Result<String, String> {
    let suffix = path.trim_start_matches("/v1");
    if suffix.is_empty() || !suffix.starts_with('/') {
        return Err("missing model path".into());
    }
    let base = base.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err("model endpoint is empty".into());
    }
    if base.ends_with("/v1") {
        Ok(format!("{base}{suffix}"))
    } else {
        Ok(format!("{base}/v1{suffix}"))
    }
}

fn error(status: u16, message: &str) -> (u16, String) {
    (
        status,
        json!({ "error": { "message": message } }).to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::provider_url;

    #[test]
    fn joins_openai_and_anthropic_bases() {
        assert_eq!(
            provider_url("https://api.openai.com/v1", "/v1/chat/completions").unwrap(),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            provider_url("https://api.anthropic.com", "/v1/messages").unwrap(),
            "https://api.anthropic.com/v1/messages"
        );
    }
}
