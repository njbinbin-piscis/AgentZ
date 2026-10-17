//! Per-panel filesystem resource access for sandboxed extension webviews.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use tauri::{http, State};
use uuid::Uuid;

use crate::state::AppState;

const MAX_PANEL_ROOTS: usize = 64;
const MAX_RESOURCE_BYTES: u64 = 32 * 1024 * 1024;

fn canonical_roots(roots: Vec<String>) -> Result<Vec<PathBuf>, String> {
    if roots.len() > MAX_PANEL_ROOTS {
        return Err("webview resource roots cannot exceed 64 paths".into());
    }
    let mut canonical = Vec::new();
    for root in roots {
        let path = PathBuf::from(root);
        if !path.is_absolute() {
            return Err("webview resource roots must be absolute paths".into());
        }
        let path =
            std::fs::canonicalize(path).map_err(|_| "webview resource root is unavailable")?;
        if !path.is_dir() {
            return Err("webview resource roots must be directories".into());
        }
        if !canonical.contains(&path) {
            canonical.push(path);
        }
    }
    Ok(canonical)
}

#[tauri::command]
pub fn webview_resources_register(
    state: State<'_, AppState>,
    token: String,
    roots: Vec<String>,
) -> Result<(), String> {
    Uuid::parse_str(&token).map_err(|_| "invalid webview resource token")?;
    let roots = canonical_roots(roots)?;
    let mut registry = state
        .webview_resource_roots
        .lock()
        .map_err(|_| "webview resource registry is unavailable")?;
    if registry.contains_key(&token) {
        return Err("webview resource token is already registered".into());
    }
    registry.insert(token, roots);
    Ok(())
}

#[tauri::command]
pub fn webview_resources_unregister(
    state: State<'_, AppState>,
    token: String,
) -> Result<(), String> {
    Uuid::parse_str(&token).map_err(|_| "invalid webview resource token")?;
    state
        .webview_resource_roots
        .lock()
        .map_err(|_| "webview resource registry is unavailable")?
        .remove(&token);
    Ok(())
}

fn response(
    status: http::StatusCode,
    content_type: &str,
    body: Vec<u8>,
) -> http::Response<Vec<u8>> {
    http::Response::builder()
        .status(status)
        .header(http::header::CONTENT_TYPE, content_type)
        .header(http::header::CACHE_CONTROL, "no-store")
        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header("cross-origin-resource-policy", "cross-origin")
        .body(body)
        .expect("valid resource response")
}

fn denied() -> http::Response<Vec<u8>> {
    response(
        http::StatusCode::NOT_FOUND,
        "text/plain; charset=utf-8",
        Vec::new(),
    )
}

pub fn protocol_error_response() -> http::Response<Vec<u8>> {
    denied()
}

fn decode_path(encoded: &str) -> Option<PathBuf> {
    if encoded.is_empty()
        || encoded.len() > 64 * 1024
        || !encoded
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return None;
    }
    let bytes = URL_SAFE_NO_PAD.decode(encoded).ok()?;
    let path = String::from_utf8(bytes).ok()?;
    let path = PathBuf::from(path);
    path.is_absolute().then_some(path)
}

fn allowed_path(path: &Path, roots: &[PathBuf]) -> Option<PathBuf> {
    let canonical = std::fs::canonicalize(path).ok()?;
    if !canonical.is_file() || !roots.iter().any(|root| canonical.starts_with(root)) {
        return None;
    }
    let metadata = std::fs::metadata(&canonical).ok()?;
    (metadata.len() <= MAX_RESOURCE_BYTES).then_some(canonical)
}

fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "css" => "text/css; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}

fn serve_resource(token: &str, encoded_path: &str, roots: &[PathBuf]) -> http::Response<Vec<u8>> {
    if Uuid::parse_str(token).is_err() {
        return denied();
    }
    let Some(path) = decode_path(encoded_path) else {
        return denied();
    };
    let Some(path) = allowed_path(&path, roots) else {
        return denied();
    };
    match read_bounded(&path) {
        Ok(data) => response(http::StatusCode::OK, content_type(&path), data),
        Err(_) => denied(),
    }
}

fn read_bounded(path: &Path) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    if file.metadata()?.len() > MAX_RESOURCE_BYTES {
        return Err(std::io::Error::other("webview resource exceeds limit"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_RESOURCE_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_RESOURCE_BYTES {
        return Err(std::io::Error::other("webview resource exceeds limit"));
    }
    Ok(bytes)
}

pub fn handle_protocol_request(
    roots: &std::sync::Mutex<HashMap<String, Vec<PathBuf>>>,
    request: &http::Request<Vec<u8>>,
) -> http::Response<Vec<u8>> {
    if request.method() != http::Method::GET || request.uri().query().is_some() {
        return denied();
    }
    let Some(host) = request.uri().host() else {
        return denied();
    };
    if host != "localhost" && host != "agentz-resource.localhost" {
        return denied();
    }
    let Some(path) = request.uri().path().strip_prefix('/') else {
        return denied();
    };
    let Some((token, encoded_path)) = path.split_once('/') else {
        return denied();
    };
    let Ok(scopes) = roots.lock() else {
        return denied();
    };
    let Some(panel_roots) = scopes.get(token).cloned() else {
        return denied();
    };
    drop(scopes);
    serve_resource(token, encoded_path, &panel_roots)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("agentz-webview-{name}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path).expect("create fixture directory");
        path
    }

    #[test]
    fn serves_only_files_under_the_panel_specific_registered_root() {
        let allowed_root = temp_dir("allowed");
        let other_root = temp_dir("other");
        let allowed_file = allowed_root.join("panel.css");
        let other_file = other_root.join("private.css");
        std::fs::write(&allowed_file, "body { color: red; }").expect("write allowed file");
        std::fs::write(&other_file, "secret").expect("write private file");
        let token = Uuid::new_v4().to_string();
        let other_token = Uuid::new_v4().to_string();
        let scopes = HashMap::from([
            (
                token.clone(),
                vec![std::fs::canonicalize(&allowed_root).unwrap()],
            ),
            (
                other_token,
                vec![std::fs::canonicalize(&other_root).unwrap()],
            ),
        ]);

        let encode = |path: &Path| URL_SAFE_NO_PAD.encode(path.to_string_lossy().as_bytes());
        let allowed = serve_resource(&token, &encode(&allowed_file), &scopes[&token]);
        assert_eq!(allowed.status(), http::StatusCode::OK);
        assert_eq!(
            allowed.headers()[http::header::CONTENT_TYPE],
            "text/css; charset=utf-8"
        );
        assert_eq!(allowed.body(), b"body { color: red; }");

        let registry = std::sync::Mutex::new(scopes.clone());
        let request = http::Request::builder()
            .uri(format!(
                "agentz-resource://localhost/{token}/{}",
                encode(&allowed_file)
            ))
            .body(Vec::new())
            .expect("build protocol request");
        assert_eq!(
            handle_protocol_request(&registry, &request).status(),
            http::StatusCode::OK
        );
        let invalid_host = http::Request::builder()
            .uri(format!(
                "agentz-resource://attacker.localhost/{token}/{}",
                encode(&allowed_file)
            ))
            .body(Vec::new())
            .expect("build invalid host request");
        assert_eq!(
            handle_protocol_request(&registry, &invalid_host).status(),
            http::StatusCode::NOT_FOUND
        );

        let sibling = serve_resource(&token, &encode(&other_file), &scopes[&token]);
        assert_eq!(sibling.status(), http::StatusCode::NOT_FOUND);
        let traversal = allowed_root.join("..\\other\\private.css");
        let traversal = serve_resource(&token, &encode(&traversal), &scopes[&token]);
        assert_eq!(traversal.status(), http::StatusCode::NOT_FOUND);

        let _ = std::fs::remove_dir_all(allowed_root);
        let _ = std::fs::remove_dir_all(other_root);
    }
}
