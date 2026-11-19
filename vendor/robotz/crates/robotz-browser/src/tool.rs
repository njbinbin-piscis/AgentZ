use crate::manager::SharedBrowserManager;
use anyhow::Result;
use async_trait::async_trait;
use base64::Engine;
use chromiumoxide::cdp::browser_protocol::network::{CookieParam, DeleteCookiesParams};
use futures::StreamExt;
use once_cell::sync::Lazy;
/// Browser automation tool via Chrome DevTools Protocol (CDP).
/// Uses Chrome for Testing (auto-downloaded) or system Chrome.
use robotz_core::{ImageData, Tool, ToolContext, ToolResult};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;
use uuid::Uuid;

const DEFAULT_TIMEOUT_MS: u64 = 15000;
const MAX_CONTENT_BYTES: usize = 50 * 1024; // 50 KB
const DOWNLOAD_POLL_MS: u64 = 400;
/// Longest edge (px) of a viewport/element screenshot sent to a vision model.
const SCREENSHOT_MAX_EDGE: f64 = 1568.0;
/// Longest edge (px) for `full_page` screenshots.
const SCREENSHOT_FULL_PAGE_MAX_EDGE: f64 = 2000.0;
/// Page height (CSS px) captured by `full_page`; taller pages are truncated.
const FULL_PAGE_MAX_HEIGHT: f64 = 6000.0;
/// Settle delay before an automatic post-action screenshot.
const AUTO_SHOT_SETTLE_MS: u64 = 250;
const JPEG_QUALITY: i64 = 75;

/// Last auto-screenshot hash per tab, used to skip identical frames.
static LAST_SHOT_HASH: Lazy<std::sync::Mutex<HashMap<String, u64>>> =
    Lazy::new(|| std::sync::Mutex::new(HashMap::new()));

/// Scale factor so that the longest edge of `w`x`h` fits `max_edge` (never upscales).
fn fit_scale(w: f64, h: f64, max_edge: f64) -> f64 {
    let longest = w.max(h);
    if longest <= 0.0 || longest <= max_edge {
        1.0
    } else {
        max_edge / longest
    }
}

/// Actions after which an automatic screenshot is useful (they change what is visible).
fn is_visual_action(action: &str) -> bool {
    matches!(
        action,
        "navigate"
            | "go_back"
            | "go_forward"
            | "reload"
            | "click"
            | "double_click"
            | "right_click"
            | "hover"
            | "click_coords"
            | "type_text"
            | "fill"
            | "clear"
            | "press_key"
            | "scroll"
            | "select"
            | "check"
            | "uncheck"
            | "new_tab"
            | "switch_tab"
    )
}

fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut h = DefaultHasher::new();
    bytes.hash(&mut h);
    h.finish()
}

struct Shot {
    bytes: Vec<u8>,
    width: u32,
    height: u32,
    /// Captured area in CSS px (what click coordinates refer to).
    css_w: f64,
    css_h: f64,
    truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
enum DownloadStatus {
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
struct DownloadEntry {
    id: String,
    url: String,
    save_path: String,
    status: DownloadStatus,
    bytes_received: u64,
    bytes_total: Option<u64>,
    error: Option<String>,
}

static DOWNLOADS: Lazy<Arc<RwLock<HashMap<String, DownloadEntry>>>> =
    Lazy::new(|| Arc::new(RwLock::new(HashMap::new())));

pub struct BrowserTool {
    manager: SharedBrowserManager,
}

impl BrowserTool {
    pub fn new(manager: SharedBrowserManager) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for BrowserTool {
    fn name(&self) -> &str {
        "browser"
    }

    fn description(&self) -> &str {
        "Control a Chrome browser via CDP. Navigate pages, click elements, type text, \
         take screenshots (viewport, full page, single element via ref/selector, or x/y/width/height region; downscaled to <=1568px; returned to Vision AI), execute JavaScript, manage tabs, \
         and interact with web content.\n\
         \n\
         E2E workflow (recommended):\n\
         1. `navigate` → `lock` → `snapshot` — get accessibility-style tree with refs (e1, e2…)\n\
         2. Interact via `ref` (click/type/fill/hover/assert) or CSS `selector`\n\
         3. `assert_url` / `assert_visible` / `assert_text` for verification; `screenshot` with save_path for evidence\n\
         4. `unlock` when done\n\
         \n\
         Fallback element discovery:\n\
         - `get_interactive_elements` — flat table with selectors and coordinates\n\
         - `annotate_screenshot` — Set-of-Mark overlay for vision models → `click_coords`\n\
         \n\
         Static docs: prefer kernel `web_fetch`; use `browser` for JS/SPA/login/E2E."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "navigate", "go_back", "go_forward", "reload",
                        "click", "double_click", "right_click", "hover", "click_coords",
                        "type_text", "fill", "clear", "press_key",
                        "screenshot", "annotate_screenshot", "get_interactive_elements",
                        "snapshot", "lock", "unlock",
                        "get_content", "get_text", "get_attribute",
                        "eval_js", "wait_for", "wait_for_text", "scroll",
                        "select", "check", "uncheck",
                        "assert_text", "assert_visible", "assert_url", "assert_title",
                        "list_tabs", "new_tab", "close_tab", "switch_tab",
                        "get_cookies", "set_cookie", "clear_cookies",
                        "get_url", "get_title", "detect_challenge",
                        "download_file", "list_downloads", "wait_download",
                        "launch", "close"
                    ],
                    "description": "Action to perform"
                },
                "url": {
                    "type": "string",
                    "description": "URL for navigate action"
                },
                "selector": {
                    "type": "string",
                    "description": "CSS selector for element actions (alternative to ref)"
                },
                "ref": {
                    "type": "string",
                    "description": "Opaque element ref from snapshot (e.g. e3). Preferred for E2E after snapshot."
                },
                "text": {
                    "type": "string",
                    "description": "Text to type or option value to select"
                },
                "key": {
                    "type": "string",
                    "description": "Key name for press_key (e.g. 'Enter', 'Tab', 'Escape', 'ArrowDown')"
                },
                "js": {
                    "type": "string",
                    "description": "JavaScript code to execute (for eval_js)"
                },
                "attribute": {
                    "type": "string",
                    "description": "Attribute name to get (for get_attribute)"
                },
                "tab_id": {
                    "type": "string",
                    "description": "Tab identifier (default: active tab)"
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "Timeout in milliseconds (default: 15000)"
                },
                "full_page": {
                    "type": "boolean",
                    "description": "Capture full page screenshot (default: false). Capped at 6000 CSS px height; pass ref/selector to capture a single element instead."
                },
                "save_path": {
                    "type": "string",
                    "description": "For screenshot: absolute file path to persist the image (e.g. '<workspace>/.agentz/screenshots/browser_<timestamp>.jpg'). Written BEFORE the tool returns. If omitted, explicit screenshots are auto-saved to '<workspace>/.agentz/screenshots/' (disable with save=false). (Also used by download_file to set the target location.)"
                },
                "save": {
                    "type": "boolean",
                    "description": "For screenshot: set false to skip auto-saving to .agentz/screenshots (default true)"
                },
                "width": {
                    "type": "integer",
                    "description": "For screenshot: region width in CSS px (with x, y, height) to capture only that viewport region"
                },
                "height": {
                    "type": "integer",
                    "description": "For screenshot: region height in CSS px (with x, y, width)"
                },
                "auto_screenshot": {
                    "type": "boolean",
                    "description": "Attach a compressed viewport screenshot after visual actions (navigate/click/type/scroll...). Normally enabled by the host for vision models; identical consecutive frames are skipped."
                },
                "wait_condition": {
                    "type": "string",
                    "enum": ["navigation", "element", "element_hidden", "network_idle", "human_verification"],
                    "description": "Condition to wait for"
                },
                "scroll_direction": {
                    "type": "string",
                    "enum": ["up", "down", "left", "right", "top", "bottom"],
                    "description": "Scroll direction"
                },
                "scroll_amount": {
                    "type": "integer",
                    "description": "Scroll amount in pixels (default: 300)"
                },
                "cookie_name": {
                    "type": "string",
                    "description": "Cookie name"
                },
                "cookie_value": {
                    "type": "string",
                    "description": "Cookie value"
                },
                "headless": {
                    "type": "boolean",
                    "description": "Launch in headless mode (for 'launch' action). Omit or set false for a visible browser window. Set true only when you explicitly want a background/hidden browser."
                },
                "download_id": {
                    "type": "string",
                    "description": "Download task id for wait_download"
                },
                "x": {
                    "type": "integer",
                    "description": "X coordinate for click_coords action"
                },
                "y": {
                    "type": "integer",
                    "description": "Y coordinate for click_coords action"
                },
                "interactive": {
                    "type": "boolean",
                    "description": "For snapshot: only include interactive elements (default false)"
                },
                "max_depth": {
                    "type": "integer",
                    "description": "For snapshot: max tree depth (default 20)"
                },
                "compact": {
                    "type": "boolean",
                    "description": "For snapshot: omit indentation (default false)"
                },
                "include_diff": {
                    "type": "boolean",
                    "description": "For snapshot: note changes vs previous snapshot (default false)"
                },
                "match_mode": {
                    "type": "string",
                    "enum": ["substring", "regex"],
                    "description": "For assert_url/assert_title: match mode (default substring)"
                }
            },
            "required": ["action"]
        })
    }

    fn is_read_only(&self) -> bool {
        false
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let res = self.call_inner(input.clone(), ctx).await?;
        Ok(self.attach_auto_screenshot(&input, res).await)
    }
}

impl BrowserTool {
    async fn call_inner(&self, input: Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let action = match input["action"].as_str() {
            Some(a) => a,
            None => return Ok(ToolResult::err("Missing required parameter: action")),
        };

        match action {
            "launch" => self.launch_browser(&input).await,
            "close" => self.close_browser().await,
            "navigate" => self.navigate(&input).await,
            "go_back" => self.go_back(&input).await,
            "go_forward" => self.go_forward(&input).await,
            "reload" => self.reload(&input).await,
            "click" => self.click(&input).await,
            "double_click" => self.double_click(&input).await,
            "right_click" => self.right_click(&input).await,
            "hover" => self.hover(&input).await,
            "click_coords" => self.click_coords(&input).await,
            "type_text" => self.type_text(&input).await,
            "fill" => self.fill(&input).await,
            "clear" => self.clear(&input).await,
            "press_key" => self.press_key(&input).await,
            "screenshot" => self.screenshot(&input, _ctx).await,
            "annotate_screenshot" => self.annotate_screenshot(&input).await,
            "get_interactive_elements" => self.get_interactive_elements(&input).await,
            "snapshot" => self.snapshot(&input).await,
            "lock" => self.lock_session(&input).await,
            "unlock" => self.unlock_session(&input).await,
            "get_content" => self.get_content(&input).await,
            "get_text" => self.get_text(&input).await,
            "get_attribute" => self.get_attribute(&input).await,
            "eval_js" => self.eval_js(&input).await,
            "wait_for" => self.wait_for(&input).await,
            "wait_for_text" => self.wait_for_text(&input).await,
            "scroll" => self.scroll(&input).await,
            "select" => self.select(&input).await,
            "check" => self.set_checked(&input, true).await,
            "uncheck" => self.set_checked(&input, false).await,
            "assert_text" => self.assert_text(&input).await,
            "assert_visible" => self.assert_visible(&input).await,
            "assert_url" => self.assert_url(&input).await,
            "assert_title" => self.assert_title(&input).await,
            "list_tabs" => self.list_tabs().await,
            "new_tab" => self.new_tab(&input).await,
            "close_tab" => self.close_tab(&input).await,
            "switch_tab" => self.switch_tab(&input).await,
            "get_cookies" => self.get_cookies(&input).await,
            "set_cookie" => self.set_cookie(&input).await,
            "clear_cookies" => self.clear_cookies(&input).await,
            "get_url" => self.get_url(&input).await,
            "get_title" => self.get_title(&input).await,
            "detect_challenge" => self.detect_challenge(&input).await,
            "download_file" => self.download_file(&input, _ctx).await,
            "list_downloads" => self.list_downloads().await,
            "wait_download" => self.wait_download(&input).await,
            _ => Ok(ToolResult::err(format!("Unknown action: {}", action))),
        }
    }

    // ─── Browser lifecycle ────────────────────────────────────────────────────

    async fn launch_browser(&self, input: &Value) -> Result<ToolResult> {
        // Default to headed (visible) when the caller doesn't specify headless.
        // Agents should see browser activity; headless=true must be explicit.
        let requested_headless = input["headless"].as_bool().unwrap_or(false);
        let mut mgr = self.manager.lock().await;
        let current = mgr.headless();
        if current != requested_headless {
            if mgr.is_running() {
                mgr.close().await;
            }
            mgr.set_headless(requested_headless);
        }
        if mgr.is_running() {
            return Ok(ToolResult::ok(format!(
                "Browser already running (headless={})",
                mgr.headless()
            )));
        }

        // First attempt
        match mgr.launch().await {
            Ok(()) => {}
            Err(first_err) => {
                tracing::warn!(
                    "Browser launch attempt 1 failed: {}. Retrying after 1s...",
                    first_err
                );
                // Wait briefly and retry — Chrome may need time to fully exit
                drop(mgr);
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                let mut mgr2 = self.manager.lock().await;
                mgr2.launch().await.map_err(|e| {
                    anyhow::anyhow!(
                        "Browser launch failed after retry.\nFirst error: {}\nRetry error: {}",
                        first_err,
                        e
                    )
                })?;
                return Ok(ToolResult::ok(format!(
                    "Browser launched (headless={}, needed retry)",
                    mgr2.headless()
                )));
            }
        }

        Ok(ToolResult::ok(format!(
            "Browser launched (headless={})",
            mgr.headless()
        )))
    }

    async fn close_browser(&self) -> Result<ToolResult> {
        let mut mgr = self.manager.lock().await;
        mgr.close().await;
        Ok(ToolResult::ok("Browser closed"))
    }

    // ─── Navigation ───────────────────────────────────────────────────────────

    async fn navigate(&self, input: &Value) -> Result<ToolResult> {
        let url = match input["url"].as_str() {
            Some(u) => u.to_string(),
            None => return Ok(ToolResult::err("navigate requires url")),
        };

        let (page, tab_id) = self.page_ctx(input).await?;
        crate::snapshot::invalidate_tab(&tab_id).await;

        let _ = page
            .goto(&url)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        self.wait_until_navigation_ready(&page, DEFAULT_TIMEOUT_MS)
            .await?;

        let title = page
            .evaluate("document.title")
            .await
            .ok()
            .and_then(|r| r.into_value::<String>().ok())
            .unwrap_or_default();
        let current_url = page
            .evaluate("window.location.href")
            .await
            .ok()
            .and_then(|r| r.into_value::<String>().ok())
            .unwrap_or_default();
        if let Some(reason) = self.detect_challenge_hint(&page).await? {
            return Ok(ToolResult::ok(format!(
                "Navigated to: {}\nTitle: {}\n\n检测到可能验证码/人机校验: {}\n请人工完成校验后，再调用 browser.wait_for(wait_condition='human_verification') 继续。",
                current_url, title, reason
            )));
        }
        Ok(ToolResult::ok(format!(
            "Navigated to: {}\nTitle: {}",
            current_url, title
        )))
    }

    async fn go_back(&self, _input: &Value) -> Result<ToolResult> {
        let mut mgr = self.manager.lock().await;
        let page = mgr.active_page().await?;
        drop(mgr);
        page.evaluate("history.back()")
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        let _ = self
            .wait_until_navigation_ready(&page, DEFAULT_TIMEOUT_MS)
            .await;
        Ok(ToolResult::ok("Navigated back"))
    }

    async fn go_forward(&self, _input: &Value) -> Result<ToolResult> {
        let mut mgr = self.manager.lock().await;
        let page = mgr.active_page().await?;
        drop(mgr);
        page.evaluate("history.forward()")
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        let _ = self
            .wait_until_navigation_ready(&page, DEFAULT_TIMEOUT_MS)
            .await;
        Ok(ToolResult::ok("Navigated forward"))
    }

    async fn reload(&self, input: &Value) -> Result<ToolResult> {
        let (page, tab_id) = self.page_ctx(input).await?;
        crate::snapshot::invalidate_tab(&tab_id).await;
        page.reload().await.map_err(|e| anyhow::anyhow!("{}", e))?;
        let _ = self
            .wait_until_navigation_ready(&page, DEFAULT_TIMEOUT_MS)
            .await;
        Ok(ToolResult::ok("Page reloaded"))
    }

    // ─── Element interaction ──────────────────────────────────────────────────

    async fn click(&self, input: &Value) -> Result<ToolResult> {
        let (page, tab_id) = self.page_ctx(input).await?;
        let selector = self.resolve_target(input, &tab_id).await?;

        let element = page
            .find_element(&selector)
            .await
            .map_err(|e| anyhow::anyhow!("Element '{}' not found: {}", selector, e))?;
        element
            .click()
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(ToolResult::ok(format!("Clicked: {}", selector)))
    }

    async fn double_click(&self, input: &Value) -> Result<ToolResult> {
        let selector = self.require_selector(input)?;
        let mut mgr = self.manager.lock().await;
        let page = mgr.active_page().await?;
        drop(mgr);

        let js = format!(
            r#"
            const el = document.querySelector({});
            if (!el) throw new Error('Element not found');
            el.dispatchEvent(new MouseEvent('dblclick', {{bubbles: true}}));
            "#,
            Self::js_str(&selector)
        );
        page.evaluate(js)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(ToolResult::ok(format!("Double-clicked: {}", selector)))
    }

    async fn right_click(&self, input: &Value) -> Result<ToolResult> {
        let selector = self.require_selector(input)?;
        let mut mgr = self.manager.lock().await;
        let page = mgr.active_page().await?;
        drop(mgr);

        let js = format!(
            r#"
            const el = document.querySelector({});
            if (!el) throw new Error('Element not found');
            el.dispatchEvent(new MouseEvent('contextmenu', {{bubbles: true}}));
            "#,
            Self::js_str(&selector)
        );
        page.evaluate(js)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(ToolResult::ok(format!("Right-clicked: {}", selector)))
    }

    async fn hover(&self, input: &Value) -> Result<ToolResult> {
        let selector = self.require_selector(input)?;
        let mut mgr = self.manager.lock().await;
        let page = mgr.active_page().await?;
        drop(mgr);

        let js = format!(
            r#"
            const el = document.querySelector({});
            if (!el) throw new Error('Element not found');
            el.dispatchEvent(new MouseEvent('mouseover', {{bubbles: true}}));
            el.dispatchEvent(new MouseEvent('mouseenter', {{bubbles: true}}));
            "#,
            Self::js_str(&selector)
        );
        page.evaluate(js)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(ToolResult::ok(format!("Hovered: {}", selector)))
    }

    async fn type_text(&self, input: &Value) -> Result<ToolResult> {
        let (page, tab_id) = self.page_ctx(input).await?;
        let selector = self.resolve_target(input, &tab_id).await?;
        let text = match input["text"].as_str() {
            Some(t) => t.to_string(),
            None => return Ok(ToolResult::err("type_text requires text")),
        };

        let element = page
            .find_element(&selector)
            .await
            .map_err(|e| anyhow::anyhow!("Element '{}' not found: {}", selector, e))?;
        element
            .click()
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        // Set value via JS for reliability
        let js = format!(
            r#"
            const el = document.querySelector({});
            if (el) {{
                el.value = {};
                el.dispatchEvent(new Event('input', {{bubbles: true}}));
                el.dispatchEvent(new Event('change', {{bubbles: true}}));
            }}
            "#,
            Self::js_str(&selector),
            Self::js_str(&text)
        );
        page.evaluate(js)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(ToolResult::ok(format!(
            "Typed into {}: {}",
            selector,
            &text.chars().take(50).collect::<String>()
        )))
    }

    async fn clear(&self, input: &Value) -> Result<ToolResult> {
        let selector = self.require_selector(input)?;
        let mut mgr = self.manager.lock().await;
        let page = mgr.active_page().await?;
        drop(mgr);

        let js = format!(
            r#"
            const el = document.querySelector({});
            if (!el) throw new Error('Element not found');
            el.value = '';
            el.dispatchEvent(new Event('input', {{bubbles: true}}));
            "#,
            Self::js_str(&selector)
        );
        page.evaluate(js)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(ToolResult::ok(format!("Cleared: {}", selector)))
    }

    async fn press_key(&self, input: &Value) -> Result<ToolResult> {
        let key = match input["key"].as_str() {
            Some(k) => k.to_string(),
            None => {
                return Ok(ToolResult::err(
                    "press_key requires key (e.g. 'Enter', 'Tab')",
                ))
            }
        };

        let mut mgr = self.manager.lock().await;
        let page = mgr.active_page().await?;
        drop(mgr);

        // If selector provided, focus element first
        if let Some(selector) = input["selector"].as_str() {
            if let Ok(element) = page.find_element(selector).await {
                let _ = element.click().await;
            }
        }

        // Use keyboard API
        page.evaluate(format!(
            "document.dispatchEvent(new KeyboardEvent('keydown', {{key: '{}', bubbles: true}})); \
             document.dispatchEvent(new KeyboardEvent('keyup', {{key: '{}', bubbles: true}}))",
            key.replace('\'', "\\'"),
            key.replace('\'', "\\'")
        ))
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(ToolResult::ok(format!("Pressed key: {}", key)))
    }

    // ─── Screenshot ───────────────────────────────────────────────────────────

    /// Capture a downscaled JPEG. `region` is `(x, y, w, h)` in *page* CSS px;
    /// `None` captures the current viewport (or the whole page when `full_page`).
    async fn capture_shot(
        &self,
        page: &chromiumoxide::Page,
        full_page: bool,
        region: Option<(f64, f64, f64, f64)>,
    ) -> Result<Shot> {
        let metrics: Value = page
            .evaluate(
                "JSON.stringify({w:window.innerWidth,h:window.innerHeight,\
                 sx:window.scrollX,sy:window.scrollY,\
                 sw:Math.max(document.documentElement.scrollWidth,window.innerWidth),\
                 sh:Math.max(document.documentElement.scrollHeight,window.innerHeight)})",
            )
            .await
            .ok()
            .and_then(|r| r.into_value::<String>().ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(Value::Null);
        let f = |k: &str, d: f64| metrics[k].as_f64().unwrap_or(d);

        let mut truncated = false;
        let (x, y, w, h, max_edge, beyond) = if let Some((x, y, w, h)) = region {
            (x, y, w, h, SCREENSHOT_MAX_EDGE, true)
        } else if full_page {
            let h = f("sh", 720.0);
            if h > FULL_PAGE_MAX_HEIGHT {
                truncated = true;
            }
            (
                0.0,
                0.0,
                f("sw", 1280.0),
                h.min(FULL_PAGE_MAX_HEIGHT),
                SCREENSHOT_FULL_PAGE_MAX_EDGE,
                true,
            )
        } else {
            (f("sx", 0.0), f("sy", 0.0), f("w", 1280.0), f("h", 720.0), SCREENSHOT_MAX_EDGE, false)
        };
        let w = w.max(1.0);
        let h = h.max(1.0);
        let scale = fit_scale(w, h, max_edge);

        let params = chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotParams {
            format: Some(chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat::Jpeg),
            quality: Some(JPEG_QUALITY),
            capture_beyond_viewport: Some(beyond),
            clip: Some(chromiumoxide::cdp::browser_protocol::page::Viewport {
                x,
                y,
                width: w,
                height: h,
                scale,
            }),
            ..Default::default()
        };
        let bytes = page
            .screenshot(params)
            .await
            .map_err(|e| anyhow::anyhow!("Screenshot failed: {}", e))?;
        Ok(Shot {
            bytes,
            width: (w * scale).round() as u32,
            height: (h * scale).round() as u32,
            css_w: w,
            css_h: h,
            truncated,
        })
    }

    /// Page-coordinate rect (x, y, w, h) of an element, scrolled into view first.
    async fn element_region(
        &self,
        page: &chromiumoxide::Page,
        selector: &str,
    ) -> Result<(f64, f64, f64, f64)> {
        let js = format!(
            "(()=>{{const e=document.querySelector({});if(!e)return null;\
             e.scrollIntoView({{block:'center',inline:'center'}});\
             const r=e.getBoundingClientRect();\
             return JSON.stringify([r.left+window.scrollX,r.top+window.scrollY,r.width,r.height]);}})()",
            Self::js_str(selector)
        );
        let raw = page
            .evaluate(js)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?
            .into_value::<String>()
            .map_err(|_| anyhow::anyhow!("Element '{}' not found", selector))?;
        let v: Vec<f64> = serde_json::from_str(&raw)?;
        if v.len() != 4 || v[2] < 1.0 || v[3] < 1.0 {
            return Err(anyhow::anyhow!("Element '{}' has no visible area", selector));
        }
        Ok((v[0], v[1], v[2], v[3]))
    }

    async fn screenshot(&self, input: &Value, ctx: &ToolContext) -> Result<ToolResult> {
        let full_page = input["full_page"].as_bool().unwrap_or(false);
        let (page, tab_id) = self.page_ctx(input).await?;

        // Target: element (ref/selector) > explicit region > viewport/full page.
        let mut target_note = String::new();
        let region = if input["ref"].as_str().is_some() || input["selector"].as_str().is_some() {
            let selector = self.resolve_target(input, &tab_id).await?;
            target_note = format!("\nElement: {selector}");
            Some(self.element_region(&page, &selector).await?)
        } else if let (Some(x), Some(y), Some(w), Some(h)) = (
            input["x"].as_f64(),
            input["y"].as_f64(),
            input["width"].as_f64(),
            input["height"].as_f64(),
        ) {
            // Region coordinates are viewport-relative (same space as click_coords).
            let off: Value = page
                .evaluate("JSON.stringify([window.scrollX,window.scrollY])")
                .await
                .ok()
                .and_then(|r| r.into_value::<String>().ok())
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or(Value::Null);
            let sx = off[0].as_f64().unwrap_or(0.0);
            let sy = off[1].as_f64().unwrap_or(0.0);
            target_note = format!("\nRegion: x={x} y={y} w={w} h={h}");
            Some((x + sx, y + sy, w, h))
        } else {
            None
        };

        let shot = self.capture_shot(&page, full_page && region.is_none(), region).await?;
        let size_kb = shot.bytes.len() / 1024;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&shot.bytes);

        let url = page
            .evaluate("window.location.href")
            .await
            .ok()
            .and_then(|r| r.into_value::<String>().ok())
            .unwrap_or_default();
        let title = page
            .evaluate("document.title")
            .await
            .ok()
            .and_then(|r| r.into_value::<String>().ok())
            .unwrap_or_default();

        // Persist: explicit save_path, else default `.agentz/screenshots/` (opt out with save=false).
        let explicit = input["save_path"].as_str().map(str::trim).filter(|s| !s.is_empty());
        let auto_save = explicit.is_none() && input["save"].as_bool().unwrap_or(true);
        let out_path: Option<PathBuf> = match explicit {
            Some(p) => Some(PathBuf::from(p)),
            None if auto_save => Some(ctx.workspace_root.join(".agentz").join("screenshots").join(
                format!("browser_{}.jpg", chrono_stamp()),
            )),
            None => None,
        };
        let save_note = match out_path {
            Some(path) => {
                let res = match std::fs::create_dir_all(
                    path.parent().unwrap_or_else(|| Path::new(".")),
                ) {
                    Err(e) => Err(format!("could not create parent directory: {e}")),
                    Ok(()) => std::fs::write(&path, &shot.bytes).map_err(|e| format!("write failed: {e}")),
                };
                match res {
                    Ok(()) if explicit.is_some() => format!(
                        "\nSaved to disk: {} ({} KB). You MUST now call `app_control(action=\"artifact_submit\", artifact_name=\"<short label>\", path=\"{}\", artifact_type=\"image\", content_summary=\"<1-line description>\")` so the file appears in the user's Artifacts panel.",
                        path.display(), size_kb, path.display()
                    ),
                    Ok(()) => format!("\nSaved to disk: {} ({} KB).", path.display(), size_kb),
                    Err(e) => format!(
                        "\nWARNING: failed to save screenshot to '{}': {}. The image is still returned inline.",
                        path.display(), e
                    ),
                }
            }
            None => String::new(),
        };

        let trunc = if shot.truncated {
            format!("\nNote: page taller than {FULL_PAGE_MAX_HEIGHT} CSS px; truncated. Use scroll or ref/selector for the rest.")
        } else {
            String::new()
        };
        Ok(ToolResult::ok(format!(
            "Screenshot captured: {} KB, image {}x{} px (covers {:.0}x{:.0} CSS px)\nURL: {}\nTitle: {}\nFull page: {}{}{}{}",
            size_kb, shot.width, shot.height, shot.css_w, shot.css_h, url, title, full_page, target_note, trunc, save_note
        ))
        .with_image(ImageData::jpeg(b64)))
    }

    /// Append a compressed viewport screenshot to the result of a visual action when
    /// `auto_screenshot` is true. Skips identical consecutive frames per tab.
    async fn attach_auto_screenshot(&self, input: &Value, mut res: ToolResult) -> ToolResult {
        let action = input["action"].as_str().unwrap_or("");
        if res.is_error
            || res.image.is_some()
            || input["auto_screenshot"].as_bool() != Some(true)
            || !is_visual_action(action)
        {
            return res;
        }
        if !matches!(action, "navigate" | "go_back" | "go_forward" | "reload" | "new_tab") {
            tokio::time::sleep(Duration::from_millis(AUTO_SHOT_SETTLE_MS)).await;
        }
        let (page, tab_id) = {
            let mut mgr = self.manager.lock().await;
            let tab_id = mgr.current_tab_id();
            match mgr.active_page().await {
                Ok(p) => (p, tab_id),
                Err(_) => return res,
            }
        };
        let shot = match self.capture_shot(&page, false, None).await {
            Ok(s) => s,
            Err(e) => {
                res.content.push_str(&format!("\n[auto-screenshot failed: {e}]"));
                return res;
            }
        };
        let hash = hash_bytes(&shot.bytes);
        let unchanged = {
            let mut map = LAST_SHOT_HASH.lock().unwrap_or_else(|e| e.into_inner());
            map.insert(tab_id, hash) == Some(hash)
        };
        if unchanged {
            res.content.push_str("\n[auto-screenshot: page visually unchanged since last frame]");
        } else {
            res.content.push_str(&format!(
                "\n[auto-screenshot: {}x{} px viewport, {} KB]",
                shot.width,
                shot.height,
                shot.bytes.len() / 1024
            ));
            let b64 = base64::engine::general_purpose::STANDARD.encode(&shot.bytes);
            res.image = Some(ImageData::jpeg(b64));
        }
        res
    }

    // ─── Element discovery ────────────────────────────────────────────────────

    /// Returns a structured list of every visible, interactive element on the current page.
    /// Each entry includes: index, tag, role, text/label, CSS selector, center x/y.
    /// The agent should call this first to get selectors, then use click/type_text with them.
    async fn get_interactive_elements(&self, _input: &Value) -> Result<ToolResult> {
        let mut mgr = self.manager.lock().await;
        let page = mgr.active_page().await?;
        drop(mgr);

        // JS: collect all visible interactive elements, generate stable selectors
        let js = r#"
        (function() {
            const SELECTORS = [
                'a[href]','button','input:not([type="hidden"])',
                'select','textarea',
                '[role="button"]','[role="link"]','[role="checkbox"]',
                '[role="radio"]','[role="menuitem"]','[role="tab"]',
                '[role="option"]','[role="combobox"]','[role="textbox"]',
                '[role="searchbox"]','[role="switch"]',
                '[onclick]','[tabindex]:not([tabindex="-1"])'
            ].join(',');

            function bestSelector(el) {
                if (el.id) return '#' + CSS.escape(el.id);
                const dt = el.getAttribute('data-testid');
                if (dt) return `[data-testid="${dt}"]`;
                const name = el.getAttribute('name');
                if (name) return `${el.tagName.toLowerCase()}[name="${name}"]`;
                const al = el.getAttribute('aria-label');
                if (al) return `${el.tagName.toLowerCase()}[aria-label="${al.replace(/"/g,'\\"')}"]`;
                const ph = el.getAttribute('placeholder');
                if (ph) return `${el.tagName.toLowerCase()}[placeholder="${ph.replace(/"/g,'\\"')}"]`;
                // nth-child fallback
                const tag = el.tagName.toLowerCase();
                const parent = el.parentElement;
                if (parent) {
                    const siblings = Array.from(parent.children).filter(c => c.tagName === el.tagName);
                    const n = siblings.indexOf(el) + 1;
                    const firstCls = (el.className && typeof el.className === 'string')
                        ? el.className.trim().split(/\s+/)[0] : '';
                    return `${tag}${firstCls ? '.' + firstCls : ''}:nth-of-type(${n})`;
                }
                return tag;
            }

            const seen = new WeakSet();
            const result = [];
            let idx = 1;
            const vw = window.innerWidth, vh = window.innerHeight;

            document.querySelectorAll(SELECTORS).forEach(el => {
                if (seen.has(el)) return;
                seen.add(el);
                const r = el.getBoundingClientRect();
                if (r.width === 0 || r.height === 0) return;
                // Must overlap viewport
                if (r.bottom < 0 || r.top > vh || r.right < 0 || r.left > vw) return;

                const text = (el.textContent || el.value || '').trim().replace(/\s+/g,' ').slice(0,80);
                const label = el.getAttribute('aria-label') || el.getAttribute('title') || '';
                const ph    = el.getAttribute('placeholder') || '';
                const itype = el.getAttribute('type') || '';
                const role  = el.getAttribute('role') || el.tagName.toLowerCase();

                result.push({
                    index: idx++,
                    tag: el.tagName.toLowerCase(),
                    role, type: itype,
                    text, label, placeholder: ph,
                    selector: bestSelector(el),
                    cx: Math.round(r.x + r.width / 2),
                    cy: Math.round(r.y + r.height / 2)
                });
            });
            return JSON.stringify(result);
        })()
        "#;

        let raw = page
            .evaluate(js)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        let json_str = raw
            .into_value::<String>()
            .unwrap_or_else(|_| "[]".to_string());
        let elements: Vec<Value> = serde_json::from_str(&json_str).unwrap_or_default();

        if elements.is_empty() {
            return Ok(ToolResult::ok(
                "No interactive elements found on current page.",
            ));
        }

        let url = page
            .evaluate("window.location.href")
            .await
            .ok()
            .and_then(|r| r.into_value::<String>().ok())
            .unwrap_or_default();

        let mut lines = vec![
            format!("URL: {}", url),
            format!("Found {} interactive elements (use selector or click_coords x/y):", elements.len()),
            String::from("INDEX | TAG           | TEXT/LABEL                                       | SELECTOR"),
            String::from("------|---------------|--------------------------------------------------|-------------------------"),
        ];

        for el in &elements {
            let idx = el["index"].as_i64().unwrap_or(0);
            let tag = el["tag"].as_str().unwrap_or("");
            let itype = el["type"].as_str().unwrap_or("");
            let text = el["text"].as_str().unwrap_or("");
            let lbl = el["label"].as_str().unwrap_or("");
            let ph = el["placeholder"].as_str().unwrap_or("");
            let sel = el["selector"].as_str().unwrap_or("");
            let cx = el["cx"].as_i64().unwrap_or(0);
            let cy = el["cy"].as_i64().unwrap_or(0);

            let display = if !text.is_empty() {
                text
            } else if !lbl.is_empty() {
                lbl
            } else if !ph.is_empty() {
                ph
            } else {
                "(no label)"
            };

            let tag_str = if !itype.is_empty() && itype != tag {
                format!("{}<{}>", tag, itype)
            } else {
                tag.to_string()
            };

            lines.push(format!(
                "{:>5} | {:<13} | {:<48} | {}  [cx={} cy={}]",
                idx,
                tag_str,
                display.chars().take(48).collect::<String>(),
                sel,
                cx,
                cy
            ));
        }

        Ok(ToolResult::ok(lines.join("\n")))
    }

    /// Set-of-Mark: overlays numbered colored boxes on all interactive elements,
    /// captures a screenshot, then returns the annotated image + element index table.
    /// Designed for use with vision-capable LLMs: describe which element to interact with
    /// by index number, then use click_coords with the reported cx/cy.
    async fn annotate_screenshot(&self, _input: &Value) -> Result<ToolResult> {
        let mut mgr = self.manager.lock().await;
        let page = mgr.active_page().await?;
        drop(mgr);

        // Inject a temporary canvas overlay with numbered element boxes
        let inject_js = r#"
        (function() {
            const existing = document.getElementById('__piscis_som_overlay__');
            if (existing) existing.remove();

            const SELECTORS = [
                'a[href]','button','input:not([type="hidden"])',
                'select','textarea',
                '[role="button"]','[role="link"]','[role="checkbox"]',
                '[role="radio"]','[role="menuitem"]','[role="tab"]',
                '[role="option"]','[role="combobox"]','[role="textbox"]',
                '[onclick]','[tabindex]:not([tabindex="-1"])'
            ].join(',');

            const W = window.innerWidth, H = window.innerHeight;
            const canvas = document.createElement('canvas');
            canvas.id = '__piscis_som_overlay__';
            canvas.width  = W; canvas.height = H;
            canvas.style.cssText = `
                position:fixed;top:0;left:0;
                width:${W}px;height:${H}px;
                z-index:2147483647;pointer-events:none;`;
            document.documentElement.appendChild(canvas);
            const ctx = canvas.getContext('2d');

            const PALETTE = [
                '#E74C3C','#2980B9','#27AE60','#F39C12',
                '#8E44AD','#16A085','#D35400','#2C3E50'
            ];

            const seen = new WeakSet();
            const map  = [];
            let   idx  = 1;

            document.querySelectorAll(SELECTORS).forEach(el => {
                if (seen.has(el)) return;
                seen.add(el);
                const r = el.getBoundingClientRect();
                if (r.width === 0 || r.height === 0) return;
                if (r.bottom < 0 || r.top > H || r.right < 0 || r.left > W) return;

                const color = PALETTE[(idx - 1) % PALETTE.length];
                // Box
                ctx.strokeStyle = color;
                ctx.lineWidth   = 2;
                ctx.strokeRect(r.x + 1, r.y + 1, r.width - 2, r.height - 2);

                // Label pill
                const label = String(idx);
                ctx.font = 'bold 11px Arial,sans-serif';
                const tw = ctx.measureText(label).width;
                const lw = tw + 8, lh = 16;
                const lx = Math.max(0, Math.min(r.x, W - lw - 2));
                const ly = r.y >= lh + 2 ? r.y - lh - 1 : r.y + 1;
                ctx.fillStyle = color;
                ctx.beginPath();
                ctx.roundRect(lx, ly, lw, lh, 3);
                ctx.fill();
                ctx.fillStyle = '#fff';
                ctx.fillText(label, lx + 4, ly + lh - 4);

                const text = (el.textContent || el.value || el.getAttribute('placeholder') || el.getAttribute('aria-label') || '')
                    .trim().replace(/\s+/g,' ').slice(0, 60);

                map.push({
                    index: idx,
                    tag: el.tagName.toLowerCase(),
                    text,
                    cx: Math.round(r.x + r.width  / 2),
                    cy: Math.round(r.y + r.height / 2)
                });
                idx++;
            });

            return JSON.stringify(map);
        })()
        "#;

        let raw = page
            .evaluate(inject_js)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        let json_str = raw
            .into_value::<String>()
            .unwrap_or_else(|_| "[]".to_string());
        let elements: Vec<Value> = serde_json::from_str(&json_str).unwrap_or_default();

        // Capture screenshot with overlay visible (downscaled; cx/cy in the table stay CSS px)
        let shot = self.capture_shot(&page, false, None).await?;
        let img_bytes = shot.bytes;

        // Remove overlay immediately
        let _ = page
            .evaluate("document.getElementById('__piscis_som_overlay__')?.remove()")
            .await;

        let b64 = base64::engine::general_purpose::STANDARD.encode(&img_bytes);

        // Build text index
        let mut lines = vec![
            format!("Set-of-Mark: {} elements annotated in screenshot.", elements.len()),
            String::from("To interact: use click_coords with cx/cy, or get_interactive_elements for full selectors."),
            String::new(),
            String::from("INDEX | TAG      | TEXT/LABEL                                   | CX    CY"),
            String::from("------|----------|----------------------------------------------|----------"),
        ];
        for el in &elements {
            let i = el["index"].as_i64().unwrap_or(0);
            let tag = el["tag"].as_str().unwrap_or("");
            let text = el["text"].as_str().unwrap_or("(no label)");
            let cx = el["cx"].as_i64().unwrap_or(0);
            let cy = el["cy"].as_i64().unwrap_or(0);
            lines.push(format!(
                "{:>5} | {:<8} | {:<44} | {:>4}  {:>4}",
                i,
                tag,
                text.chars().take(44).collect::<String>(),
                cx,
                cy
            ));
        }

        Ok(ToolResult::ok(lines.join("\n")).with_image(ImageData::jpeg(b64)))
    }

    /// Click at absolute pixel coordinates (viewport-relative).
    /// Use after annotate_screenshot: pick the cx/cy of the desired element.
    async fn click_coords(&self, input: &Value) -> Result<ToolResult> {
        let x = match input["x"].as_i64() {
            Some(v) => v as f64,
            None => return Ok(ToolResult::err("click_coords requires x")),
        };
        let y = match input["y"].as_i64() {
            Some(v) => v as f64,
            None => return Ok(ToolResult::err("click_coords requires y")),
        };

        let mut mgr = self.manager.lock().await;
        let page = mgr.active_page().await?;
        drop(mgr);

        // Dispatch a synthetic click via JS mouse events for maximum compatibility
        let js = format!(
            r#"
            (function() {{
                const el = document.elementFromPoint({x}, {y});
                if (!el) return "no element at ({x},{y})";
                el.dispatchEvent(new MouseEvent('mousedown', {{bubbles:true, clientX:{x}, clientY:{y}}}));
                el.dispatchEvent(new MouseEvent('mouseup',   {{bubbles:true, clientX:{x}, clientY:{y}}}));
                el.dispatchEvent(new MouseEvent('click',     {{bubbles:true, clientX:{x}, clientY:{y}}}));
                return el.tagName.toLowerCase() + (el.id ? '#'+el.id : '') + ' | ' + (el.textContent||'').trim().slice(0,40);
            }})()
            "#,
            x = x,
            y = y
        );

        let result = page
            .evaluate(js)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        let desc = result.into_value::<String>().unwrap_or_default();
        Ok(ToolResult::ok(format!(
            "Clicked ({}, {}): {}",
            x as i64, y as i64, desc
        )))
    }

    // ─── Content extraction ───────────────────────────────────────────────────

    async fn get_content(&self, _input: &Value) -> Result<ToolResult> {
        let mut mgr = self.manager.lock().await;
        let page = mgr.active_page().await?;
        drop(mgr);

        let result = page
            .evaluate("document.documentElement.outerHTML")
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        let content = result.into_value::<String>().unwrap_or_default();

        // Truncate large pages
        let truncated = if content.len() > MAX_CONTENT_BYTES {
            format!(
                "{}\n\n... [{} bytes truncated] ...",
                &content[..MAX_CONTENT_BYTES],
                content.len() - MAX_CONTENT_BYTES
            )
        } else {
            content
        };

        Ok(ToolResult::ok(truncated))
    }

    async fn get_text(&self, input: &Value) -> Result<ToolResult> {
        let mut mgr = self.manager.lock().await;
        let page = mgr.active_page().await?;
        drop(mgr);

        // Characters to extract in JS before serialization — prevents sending multi-MB strings over CDP
        let char_limit = MAX_CONTENT_BYTES;

        if let Some(selector) = input["selector"].as_str() {
            let js = format!(
                r#"
                (function() {{
                    const el = document.querySelector({sel});
                    if (!el) return null;
                    const t = (el.innerText || el.textContent || '').trim();
                    return t.length > {lim} ? t.slice(0, {lim}) + '\n...[截断]' : t;
                }})()
                "#,
                sel = Self::js_str(selector),
                lim = char_limit,
            );
            let result = page
                .evaluate(js)
                .await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            let text = result
                .into_value::<Option<String>>()
                .unwrap_or(None)
                .unwrap_or_else(|| "Element not found".to_string());
            return Ok(ToolResult::ok(text));
        }

        // No selector: smart main-content extraction, limited in JS itself.
        // Tries common article/main selectors first, falls back to body.
        // IMPORTANT: truncation happens INSIDE JS to avoid serializing huge strings over CDP.
        let js = format!(
            r#"
            (function() {{
                const LIM = {lim};
                // Try to find the primary content container
                const candidates = [
                    'article', 'main', '[role="main"]',
                    '#content', '#main-content', '#article', '#J_content',
                    '.article', '.content', '.main-content', '.post-content',
                    '.lemma-summary', '.para',  // Baidu Baike
                    '.entry-content', '.post-body',
                ].map(s => document.querySelector(s))
                 .filter(Boolean);

                let target = candidates[0] || document.body;

                // Collect text nodes, skipping script/style/nav/footer
                function extractText(node, buf) {{
                    if (buf.length >= LIM) return;
                    if (!node) return;
                    const tag = node.tagName ? node.tagName.toLowerCase() : '';
                    if (['script','style','nav','footer','header','noscript','iframe','svg'].includes(tag)) return;
                    if (node.nodeType === 3) {{
                        const t = node.textContent.replace(/\s+/g,' ').trim();
                        if (t) buf.push(t);
                        return;
                    }}
                    for (const child of node.childNodes) {{
                        extractText(child, buf);
                        if (buf.join(' ').length >= LIM) break;
                    }}
                }}

                const parts = [];
                extractText(target, parts);
                const full = parts.join('\n').slice(0, LIM);
                const total = (document.body.innerText || '').length;
                const suffix = total > LIM ? `\n\n...[已截断，页面总长约 ${{total}} 字符]` : '';
                return full + suffix;
            }})()
            "#,
            lim = char_limit,
        );

        let result = tokio::time::timeout(std::time::Duration::from_secs(30), page.evaluate(js))
            .await
            .map_err(|_| anyhow::anyhow!("get_text timed out (30s) — page may be too complex"))?
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        let text = result.into_value::<String>().unwrap_or_default();
        Ok(ToolResult::ok(if text.is_empty() {
            "Page appears to have no readable text.".into()
        } else {
            text
        }))
    }

    async fn get_attribute(&self, input: &Value) -> Result<ToolResult> {
        let selector = self.require_selector(input)?;
        let attr = match input["attribute"].as_str() {
            Some(a) => a.to_string(),
            None => return Ok(ToolResult::err("get_attribute requires attribute")),
        };

        let mut mgr = self.manager.lock().await;
        let page = mgr.active_page().await?;
        drop(mgr);

        let js = format!(
            r#"
            const el = document.querySelector({});
            el ? el.getAttribute({}) : null
            "#,
            Self::js_str(&selector),
            Self::js_str(&attr)
        );
        let result = page
            .evaluate(js)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        let value = result
            .into_value::<Option<String>>()
            .unwrap_or(None)
            .unwrap_or_else(|| "null".to_string());
        Ok(ToolResult::ok(format!(
            "{}[{}] = {}",
            selector, attr, value
        )))
    }

    // ─── JavaScript execution ─────────────────────────────────────────────────

    async fn eval_js(&self, input: &Value) -> Result<ToolResult> {
        let js = match input["js"].as_str() {
            Some(j) => j.to_string(),
            None => return Ok(ToolResult::err("eval_js requires js")),
        };

        let mut mgr = self.manager.lock().await;
        let page = mgr.active_page().await?;
        drop(mgr);

        let result = page
            .evaluate(js)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        let json_val = result
            .into_value::<serde_json::Value>()
            .unwrap_or(Value::String("(non-serializable result)".into()));
        Ok(ToolResult::ok(
            serde_json::to_string_pretty(&json_val).unwrap_or_else(|_| format!("{:?}", json_val)),
        ))
    }

    // ─── Wait ─────────────────────────────────────────────────────────────────

    async fn wait_for(&self, input: &Value) -> Result<ToolResult> {
        let condition = input["wait_condition"].as_str().unwrap_or("navigation");
        let timeout_ms = input["timeout_ms"].as_u64().unwrap_or(DEFAULT_TIMEOUT_MS);

        let mut mgr = self.manager.lock().await;
        let page = mgr.active_page().await?;
        drop(mgr);

        match condition {
            "navigation" => {
                self.wait_until_navigation_ready(&page, timeout_ms).await?;
                Ok(ToolResult::ok("Navigation complete"))
            }
            "element" => {
                let selector = self.require_selector(input)?;
                let start = std::time::Instant::now();
                loop {
                    if page.find_element(&selector).await.is_ok() {
                        return Ok(ToolResult::ok(format!("Element found: {}", selector)));
                    }
                    if start.elapsed().as_millis() as u64 >= timeout_ms {
                        return Ok(ToolResult::err(format!(
                            "Timeout: element '{}' not found",
                            selector
                        )));
                    }
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
            "element_hidden" => {
                let selector = self.require_selector(input)?;
                let start = std::time::Instant::now();
                loop {
                    if page.find_element(&selector).await.is_err() {
                        return Ok(ToolResult::ok(format!("Element hidden: {}", selector)));
                    }
                    if start.elapsed().as_millis() as u64 >= timeout_ms {
                        return Ok(ToolResult::err(format!(
                            "Timeout: element '{}' still visible",
                            selector
                        )));
                    }
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
            "network_idle" => {
                self.wait_until_network_idle(&page, timeout_ms).await?;
                Ok(ToolResult::ok("Network idle reached"))
            }
            "human_verification" => {
                let cleared = self.wait_until_no_challenge(&page, timeout_ms).await?;
                if cleared {
                    Ok(ToolResult::ok(
                        "Human verification cleared, automation can continue",
                    ))
                } else {
                    Ok(ToolResult::err(
                        "Timeout waiting for human verification to be cleared",
                    ))
                }
            }
            _ => Ok(ToolResult::err(format!(
                "Unknown wait_condition: {}",
                condition
            ))),
        }
    }

    // ─── Scroll ───────────────────────────────────────────────────────────────

    async fn scroll(&self, input: &Value) -> Result<ToolResult> {
        let direction = input["scroll_direction"].as_str().unwrap_or("down");
        let amount = input["scroll_amount"].as_i64().unwrap_or(300);

        let mut mgr = self.manager.lock().await;
        let page = mgr.active_page().await?;
        drop(mgr);

        let js = match direction {
            "up" => format!("window.scrollBy(0, -{})", amount),
            "down" => format!("window.scrollBy(0, {})", amount),
            "left" => format!("window.scrollBy(-{}, 0)", amount),
            "right" => format!("window.scrollBy({}, 0)", amount),
            "top" => "window.scrollTo(0, 0)".to_string(),
            "bottom" => "window.scrollTo(0, document.body.scrollHeight)".to_string(),
            _ => format!("window.scrollBy(0, {})", amount),
        };

        page.evaluate(js)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(ToolResult::ok(format!(
            "Scrolled {} by {} px",
            direction, amount
        )))
    }

    // ─── Form controls ────────────────────────────────────────────────────────

    async fn select(&self, input: &Value) -> Result<ToolResult> {
        let selector = self.require_selector(input)?;
        let value = match input["text"].as_str() {
            Some(v) => v.to_string(),
            None => {
                return Ok(ToolResult::err(
                    "select requires text (option value or label)",
                ))
            }
        };

        let mut mgr = self.manager.lock().await;
        let page = mgr.active_page().await?;
        drop(mgr);

        let js = format!(
            r#"
            const sel = document.querySelector({});
            if (!sel) throw new Error('Select element not found');
            const opts = Array.from(sel.options);
            const opt = opts.find(o => o.value === {} || o.text === {});
            if (opt) {{
                sel.value = opt.value;
                sel.dispatchEvent(new Event('change', {{bubbles: true}}));
                opt.value;
            }} else {{
                throw new Error('Option not found: ' + {});
            }}
            "#,
            Self::js_str(&selector),
            Self::js_str(&value),
            Self::js_str(&value),
            Self::js_str(&value),
        );
        let result = page
            .evaluate(js)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        let selected = result.into_value::<String>().unwrap_or_default();
        Ok(ToolResult::ok(format!(
            "Selected '{}' in {}",
            selected, selector
        )))
    }

    async fn set_checked(&self, input: &Value, checked: bool) -> Result<ToolResult> {
        let selector = self.require_selector(input)?;
        let mut mgr = self.manager.lock().await;
        let page = mgr.active_page().await?;
        drop(mgr);

        let js = format!(
            r#"
            const el = document.querySelector({});
            if (!el) throw new Error('Element not found');
            if (el.checked !== {}) {{
                el.click();
            }}
            el.checked
            "#,
            Self::js_str(&selector),
            checked
        );
        page.evaluate(js)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        let action = if checked { "Checked" } else { "Unchecked" };
        Ok(ToolResult::ok(format!("{}: {}", action, selector)))
    }

    // ─── Tab management ───────────────────────────────────────────────────────

    async fn list_tabs(&self) -> Result<ToolResult> {
        let mgr = self.manager.lock().await;
        let tabs = mgr.list_tabs();
        let active = mgr.active_tab.clone().unwrap_or_default();
        drop(mgr);

        if tabs.is_empty() {
            return Ok(ToolResult::ok("No open tabs"));
        }
        let list: Vec<String> = tabs
            .iter()
            .map(|t| {
                if t == &active {
                    format!("* {} (active)", t)
                } else {
                    t.clone()
                }
            })
            .collect();
        Ok(ToolResult::ok(format!("Open tabs:\n{}", list.join("\n"))))
    }

    async fn new_tab(&self, input: &Value) -> Result<ToolResult> {
        let tab_id = input["tab_id"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("tab_{}", Uuid::new_v4().simple()))
            .to_string();
        let url = input["url"].as_str().unwrap_or("about:blank").to_string();

        let mut mgr = self.manager.lock().await;
        let page = mgr.create_page(&tab_id).await?;
        drop(mgr);

        if url != "about:blank" {
            let _ = page
                .goto(&url)
                .await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
        }
        Ok(ToolResult::ok(format!("New tab created: {}", tab_id)))
    }

    async fn close_tab(&self, input: &Value) -> Result<ToolResult> {
        let tab_id = match input["tab_id"].as_str() {
            Some(t) => t.to_string(),
            None => return Ok(ToolResult::err("close_tab requires tab_id")),
        };
        let mut mgr = self.manager.lock().await;
        mgr.close_tab(&tab_id).await?;
        Ok(ToolResult::ok(format!("Closed tab: {}", tab_id)))
    }

    async fn switch_tab(&self, input: &Value) -> Result<ToolResult> {
        let tab_id = match input["tab_id"].as_str() {
            Some(t) => t.to_string(),
            None => return Ok(ToolResult::err("switch_tab requires tab_id")),
        };
        let mut mgr = self.manager.lock().await;
        let current = mgr.current_tab_id();
        crate::snapshot::assert_not_locked_for_switch(&current).await?;
        mgr.switch_tab(&tab_id)?;
        Ok(ToolResult::ok(format!("Switched to tab: {}", tab_id)))
    }

    // ─── Cookies ─────────────────────────────────────────────────────────────

    async fn get_cookies(&self, _input: &Value) -> Result<ToolResult> {
        let mut mgr = self.manager.lock().await;
        let page = mgr.active_page().await?;
        drop(mgr);

        let cookies = page
            .get_cookies()
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        if cookies.is_empty() {
            return Ok(ToolResult::ok("No cookies"));
        }
        Ok(ToolResult::ok(
            serde_json::to_string_pretty(&cookies).unwrap_or_else(|_| format!("{:#?}", cookies)),
        ))
    }

    async fn set_cookie(&self, input: &Value) -> Result<ToolResult> {
        let name = match input["cookie_name"].as_str() {
            Some(n) => n.to_string(),
            None => return Ok(ToolResult::err("set_cookie requires cookie_name")),
        };
        let value = input["cookie_value"].as_str().unwrap_or("").to_string();

        let mut mgr = self.manager.lock().await;
        let page = mgr.active_page().await?;
        drop(mgr);

        let current_url = page
            .url()
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?
            .unwrap_or_default();
        let cookie = CookieParam {
            name: name.clone(),
            value,
            url: if current_url.is_empty() {
                None
            } else {
                Some(current_url)
            },
            domain: None,
            path: Some("/".to_string()),
            secure: None,
            http_only: None,
            same_site: None,
            expires: None,
            priority: None,
            same_party: None,
            source_scheme: None,
            source_port: None,
            partition_key: None,
        };
        page.set_cookie(cookie)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(ToolResult::ok(format!("Cookie set: {}", name)))
    }

    async fn clear_cookies(&self, _input: &Value) -> Result<ToolResult> {
        let mut mgr = self.manager.lock().await;
        let page = mgr.active_page().await?;
        drop(mgr);
        let cookies = page
            .get_cookies()
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        if cookies.is_empty() {
            return Ok(ToolResult::ok("No cookies to clear"));
        }
        let deletes: Vec<DeleteCookiesParams> = cookies
            .into_iter()
            .map(|c| DeleteCookiesParams {
                name: c.name,
                url: None,
                domain: Some(c.domain),
                path: Some(c.path),
                partition_key: None,
            })
            .collect();
        page.delete_cookies(deletes)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(ToolResult::ok("Cookies cleared via CDP"))
    }

    // ─── Page info ────────────────────────────────────────────────────────────

    async fn get_url(&self, _input: &Value) -> Result<ToolResult> {
        let mut mgr = self.manager.lock().await;
        let page = mgr.active_page().await?;
        drop(mgr);
        let url = page
            .evaluate("window.location.href")
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?
            .into_value::<String>()
            .unwrap_or_default();
        Ok(ToolResult::ok(url))
    }

    async fn get_title(&self, _input: &Value) -> Result<ToolResult> {
        let mut mgr = self.manager.lock().await;
        let page = mgr.active_page().await?;
        drop(mgr);
        let title = page
            .evaluate("document.title")
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?
            .into_value::<String>()
            .unwrap_or_default();
        Ok(ToolResult::ok(title))
    }

    async fn detect_challenge(&self, _input: &Value) -> Result<ToolResult> {
        let mut mgr = self.manager.lock().await;
        let page = mgr.active_page().await?;
        drop(mgr);
        match self.detect_challenge_hint(&page).await? {
            Some(reason) => Ok(ToolResult::ok(format!(
                "Detected possible human verification: {}\n请人工完成后调用 browser.wait_for(wait_condition='human_verification').",
                reason
            ))),
            None => Ok(ToolResult::ok("No obvious challenge detected")),
        }
    }

    async fn download_file(&self, input: &Value, ctx: &ToolContext) -> Result<ToolResult> {
        let url = match input["url"].as_str() {
            Some(v) => v.to_string(),
            None => return Ok(ToolResult::err("download_file requires url")),
        };
        let download_id = format!("dl_{}", Uuid::new_v4().simple());
        let save_path = self.resolve_download_path(
            input["save_path"].as_str(),
            &download_id,
            &ctx.workspace_root,
        );
        let save_path_display = save_path.to_string_lossy().to_string();

        {
            let mut map = DOWNLOADS.write().await;
            map.insert(
                download_id.clone(),
                DownloadEntry {
                    id: download_id.clone(),
                    url: url.clone(),
                    save_path: save_path_display.clone(),
                    status: DownloadStatus::Running,
                    bytes_received: 0,
                    bytes_total: None,
                    error: None,
                },
            );
        }

        let downloads = DOWNLOADS.clone();
        let spawn_download_id = download_id.clone();
        let spawn_url = url.clone();
        tokio::spawn(async move {
            if let Some(parent) = save_path.parent() {
                if let Err(e) = fs::create_dir_all(parent).await {
                    let mut map = downloads.write().await;
                    if let Some(item) = map.get_mut(&spawn_download_id) {
                        item.status = DownloadStatus::Failed;
                        item.error = Some(format!("Failed to create download directory: {}", e));
                    }
                    return;
                }
            }
            match BrowserTool::download_to_path(
                &spawn_url,
                &save_path,
                downloads.clone(),
                &spawn_download_id,
            )
            .await
            {
                Ok(_) => {}
                Err(e) => {
                    let mut map = downloads.write().await;
                    if let Some(item) = map.get_mut(&spawn_download_id) {
                        item.status = DownloadStatus::Failed;
                        item.error = Some(e.to_string());
                    }
                }
            }
        });

        Ok(ToolResult::ok(format!(
            "Download started.\nid: {}\nurl: {}\nsave_path: {}",
            download_id, url, save_path_display
        )))
    }

    async fn list_downloads(&self) -> Result<ToolResult> {
        let map = DOWNLOADS.read().await;
        let items = map.values().cloned().collect::<Vec<_>>();
        Ok(ToolResult::ok(
            serde_json::to_string_pretty(&items).unwrap_or_else(|_| "[]".to_string()),
        ))
    }

    async fn wait_download(&self, input: &Value) -> Result<ToolResult> {
        let download_id = match input["download_id"].as_str() {
            Some(v) => v.to_string(),
            None => return Ok(ToolResult::err("wait_download requires download_id")),
        };
        let timeout_ms = input["timeout_ms"].as_u64().unwrap_or(120_000);
        let start = std::time::Instant::now();
        loop {
            {
                let map = DOWNLOADS.read().await;
                if let Some(item) = map.get(&download_id) {
                    match item.status {
                        DownloadStatus::Completed => {
                            return Ok(ToolResult::ok(format!(
                                "Download completed: {}\nbytes_received: {}\nsave_path: {}",
                                download_id, item.bytes_received, item.save_path
                            )));
                        }
                        DownloadStatus::Failed => {
                            return Ok(ToolResult::err(format!(
                                "Download failed: {}\nerror: {}",
                                download_id,
                                item.error
                                    .clone()
                                    .unwrap_or_else(|| "unknown error".to_string())
                            )));
                        }
                        DownloadStatus::Running => {}
                    }
                } else {
                    return Ok(ToolResult::err(format!(
                        "Download id not found: {}",
                        download_id
                    )));
                }
            }
            if start.elapsed().as_millis() as u64 >= timeout_ms {
                return Ok(ToolResult::err(format!(
                    "Timeout waiting for download: {}",
                    download_id
                )));
            }
            tokio::time::sleep(Duration::from_millis(DOWNLOAD_POLL_MS)).await;
        }
    }

    // ─── Snapshot / lock / assert ───────────────────────────────────────────

    async fn snapshot(&self, input: &Value) -> Result<ToolResult> {
        let (page, tab_id) = self.page_ctx(input).await?;
        let opts = crate::snapshot::SnapshotOptions {
            interactive: input["interactive"].as_bool().unwrap_or(false),
            max_depth: input["max_depth"].as_u64().unwrap_or(20) as u32,
            compact: input["compact"].as_bool().unwrap_or(false),
            selector: input["selector"].as_str().map(|s| s.to_string()),
            include_diff: input["include_diff"].as_bool().unwrap_or(false),
        };
        let text = crate::snapshot::capture_snapshot(&page, &tab_id, opts).await?;
        Ok(ToolResult::ok(text))
    }

    async fn lock_session(&self, input: &Value) -> Result<ToolResult> {
        let tab_id = if let Some(t) = input["tab_id"].as_str() {
            t.to_string()
        } else {
            let mgr = self.manager.lock().await;
            mgr.current_tab_id()
        };
        crate::snapshot::lock_tab(&tab_id).await?;
        Ok(ToolResult::ok(format!(
            "Browser locked to tab '{}'. Call unlock when automation is complete.",
            tab_id
        )))
    }

    async fn unlock_session(&self, _input: &Value) -> Result<ToolResult> {
        crate::snapshot::unlock_tab().await?;
        Ok(ToolResult::ok("Browser unlocked"))
    }

    async fn fill(&self, input: &Value) -> Result<ToolResult> {
        let (page, tab_id) = self.page_ctx(input).await?;
        let selector = self.resolve_target(input, &tab_id).await?;
        let text = match input["text"].as_str() {
            Some(t) => t.to_string(),
            None => return Ok(ToolResult::err("fill requires text")),
        };
        let js = format!(
            r#"
            const el = document.querySelector({});
            if (!el) throw new Error('Element not found');
            el.focus();
            if ('value' in el) el.value = '';
            el.value = {};
            el.dispatchEvent(new Event('input', {{ bubbles: true }}));
            el.dispatchEvent(new Event('change', {{ bubbles: true }}));
            true
            "#,
            Self::js_str(&selector),
            Self::js_str(&text)
        );
        page.evaluate(js)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(ToolResult::ok(format!("Filled: {}", selector)))
    }

    async fn assert_text(&self, input: &Value) -> Result<ToolResult> {
        let expected = match input["text"].as_str() {
            Some(t) => t,
            None => return Ok(ToolResult::err("assert_text requires text")),
        };
        let (page, tab_id) = self.page_ctx(input).await?;
        let haystack = if input.get("selector").is_some() || input.get("ref").is_some() {
            let selector = self.resolve_target(input, &tab_id).await?;
            let js = format!(
                "document.querySelector({})?.innerText || ''",
                Self::js_str(&selector)
            );
            page.evaluate(js.as_str())
                .await
                .ok()
                .and_then(|r| r.into_value::<String>().ok())
                .unwrap_or_default()
        } else {
            page.evaluate("document.body?.innerText || ''")
                .await
                .ok()
                .and_then(|r| r.into_value::<String>().ok())
                .unwrap_or_default()
        };
        if haystack.contains(expected) {
            Ok(ToolResult::ok(format!("PASS: text contains '{}'", expected)))
        } else {
            Ok(ToolResult::err(format!(
                "FAIL: expected text '{}' not found",
                expected
            )))
        }
    }

    async fn assert_visible(&self, input: &Value) -> Result<ToolResult> {
        let (page, tab_id) = self.page_ctx(input).await?;
        let selector = self.resolve_target(input, &tab_id).await?;
        if page.find_element(&selector).await.is_ok() {
            Ok(ToolResult::ok(format!("PASS: visible '{}'", selector)))
        } else {
            Ok(ToolResult::err(format!(
                "FAIL: element not visible/found '{}'",
                selector
            )))
        }
    }

    async fn assert_url(&self, input: &Value) -> Result<ToolResult> {
        let expected = match input["text"].as_str().or(input["url"].as_str()) {
            Some(t) => t,
            None => return Ok(ToolResult::err("assert_url requires text or url")),
        };
        let (page, _tab_id) = self.page_ctx(input).await?;
        let current = page
            .evaluate("window.location.href")
            .await
            .ok()
            .and_then(|r| r.into_value::<String>().ok())
            .unwrap_or_default();
        let regex = input["match_mode"].as_str() == Some("regex");
        let ok = if regex {
            regex::Regex::new(expected)
                .map(|re| re.is_match(&current))
                .unwrap_or(false)
        } else {
            current.contains(expected)
        };
        if ok {
            Ok(ToolResult::ok(format!("PASS: URL matches '{}'", expected)))
        } else {
            Ok(ToolResult::err(format!(
                "FAIL: URL '{}' does not match '{}'",
                current, expected
            )))
        }
    }

    async fn assert_title(&self, input: &Value) -> Result<ToolResult> {
        let expected = match input["text"].as_str() {
            Some(t) => t,
            None => return Ok(ToolResult::err("assert_title requires text")),
        };
        let (page, _tab_id) = self.page_ctx(input).await?;
        let title = page
            .evaluate("document.title")
            .await
            .ok()
            .and_then(|r| r.into_value::<String>().ok())
            .unwrap_or_default();
        let regex = input["match_mode"].as_str() == Some("regex");
        let ok = if regex {
            regex::Regex::new(expected)
                .map(|re| re.is_match(&title))
                .unwrap_or(false)
        } else {
            title.contains(expected)
        };
        if ok {
            Ok(ToolResult::ok(format!("PASS: title matches '{}'", expected)))
        } else {
            Ok(ToolResult::err(format!(
                "FAIL: title '{}' does not match '{}'",
                title, expected
            )))
        }
    }

    async fn wait_for_text(&self, input: &Value) -> Result<ToolResult> {
        let expected = match input["text"].as_str() {
            Some(t) => t.to_string(),
            None => return Ok(ToolResult::err("wait_for_text requires text")),
        };
        let timeout_ms = input["timeout_ms"].as_u64().unwrap_or(DEFAULT_TIMEOUT_MS);
        let start = std::time::Instant::now();
        loop {
            let result = self.assert_text(input).await?;
            if !result.is_error {
                return Ok(result);
            }
            if start.elapsed().as_millis() as u64 >= timeout_ms {
                return Ok(ToolResult::err(format!(
                    "Timeout waiting for text '{}'",
                    expected
                )));
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    // ─── Helpers ─────────────────────────────────────────────────────────────

    async fn page_ctx(&self, input: &Value) -> Result<(Arc<chromiumoxide::Page>, String)> {
        let mut mgr = self.manager.lock().await;
        let tab_id_param = input["tab_id"].as_str();
        if tab_id_param.is_some() {
            let current = mgr.current_tab_id();
            crate::snapshot::assert_not_locked_for_switch(&current).await?;
        }
        let page = mgr
            .page_for_tab(tab_id_param)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let tab_id = tab_id_param
            .map(|s| s.to_string())
            .unwrap_or_else(|| mgr.current_tab_id());
        crate::snapshot::take_lock_loss(&tab_id).await?;
        Ok((page, tab_id))
    }

    async fn resolve_target(&self, input: &Value, tab_id: &str) -> Result<String> {
        if let Some(ref_id) = input["ref"].as_str() {
            let entry = crate::snapshot::resolve_ref(tab_id, ref_id).await?;
            return Ok(entry.selector);
        }
        self.require_selector(input)
    }

    fn require_selector(&self, input: &Value) -> Result<String> {
        match input["selector"].as_str() {
            Some(s) => Ok(s.to_string()),
            None => Err(anyhow::anyhow!(
                "This action requires 'selector' or 'ref' (from snapshot)"
            )),
        }
    }

    /// Safely encode a Rust string as a JSON string literal for embedding in JS code.
    /// Falls back to a manual escape if serde_json serialization somehow fails.
    fn js_str(s: &str) -> String {
        serde_json::to_string(s).unwrap_or_else(|_| {
            let escaped = s
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\r', "\\r")
                .replace('\t', "\\t");
            format!("\"{}\"", escaped)
        })
    }

    fn resolve_download_path(
        &self,
        provided_path: Option<&str>,
        download_id: &str,
        workspace_root: &Path,
    ) -> PathBuf {
        if let Some(p) = provided_path {
            return PathBuf::from(p);
        }
        let mut dir = workspace_root.to_path_buf();
        dir.push("downloads");
        dir.push(format!("{}.bin", download_id));
        dir
    }

    async fn download_to_path(
        url: &str,
        save_path: &Path,
        downloads: Arc<RwLock<HashMap<String, DownloadEntry>>>,
        download_id: &str,
    ) -> Result<()> {
        let client = reqwest::Client::new();
        let resp = client.get(url).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!(
                "HTTP {} while downloading {}",
                resp.status(),
                url
            ));
        }
        let total = resp.content_length();
        {
            let mut map = downloads.write().await;
            if let Some(item) = map.get_mut(download_id) {
                item.bytes_total = total;
            }
        }
        let mut file = File::create(save_path).await?;
        let mut stream = resp.bytes_stream();
        let mut received: u64 = 0;
        while let Some(chunk) = stream.next().await {
            let bytes = chunk?;
            file.write_all(&bytes).await?;
            received += bytes.len() as u64;
            let mut map = downloads.write().await;
            if let Some(item) = map.get_mut(download_id) {
                item.bytes_received = received;
            }
        }
        file.flush().await?;
        let mut map = downloads.write().await;
        if let Some(item) = map.get_mut(download_id) {
            item.status = DownloadStatus::Completed;
            item.bytes_received = received;
        }
        Ok(())
    }

    async fn wait_until_navigation_ready(
        &self,
        page: &chromiumoxide::Page,
        timeout_ms: u64,
    ) -> Result<()> {
        let start = std::time::Instant::now();
        loop {
            let state = page
                .evaluate("document.readyState")
                .await
                .ok()
                .and_then(|v| v.into_value::<String>().ok())
                .unwrap_or_default();
            if state == "complete" || state == "interactive" {
                return Ok(());
            }
            if start.elapsed().as_millis() as u64 >= timeout_ms {
                return Err(anyhow::anyhow!("Timeout waiting for navigation readiness"));
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
    }

    async fn wait_until_network_idle(
        &self,
        page: &chromiumoxide::Page,
        timeout_ms: u64,
    ) -> Result<()> {
        let start = std::time::Instant::now();
        let mut stable_rounds = 0u8;
        let mut last_count = -1i64;
        while (start.elapsed().as_millis() as u64) < timeout_ms {
            let count = page
                .evaluate("performance.getEntriesByType('resource').length")
                .await
                .ok()
                .and_then(|v| v.into_value::<i64>().ok())
                .unwrap_or(-1);
            let ready = page
                .evaluate("document.readyState")
                .await
                .ok()
                .and_then(|v| v.into_value::<String>().ok())
                .unwrap_or_default();
            if count == last_count && (ready == "complete" || ready == "interactive") {
                stable_rounds += 1;
            } else {
                stable_rounds = 0;
            }
            if stable_rounds >= 3 {
                return Ok(());
            }
            last_count = count;
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
        Err(anyhow::anyhow!("Timeout waiting for network idle"))
    }

    async fn detect_challenge_hint(&self, page: &chromiumoxide::Page) -> Result<Option<String>> {
        let js = r#"
(() => {
  const text = (document.body?.innerText || '').toLowerCase();
  const title = (document.title || '').toLowerCase();
  const markers = [
    'captcha', 'recaptcha', 'hcaptcha',
    'verify you are human', 'verification required',
    'robot check', 'security check'
  ];
  const hasText = markers.some(k => text.includes(k) || title.includes(k));
  const hasIframe = !!document.querySelector("iframe[src*='captcha'], iframe[src*='recaptcha'], iframe[src*='hcaptcha']");
  const hasChallengeWidget = !!document.querySelector("[id*='captcha'], [class*='captcha'], .g-recaptcha, [data-sitekey]");
  if (hasText || hasIframe || hasChallengeWidget) {
    return {
      hasChallenge: true,
      reason: `text=${hasText}, iframe=${hasIframe}, widget=${hasChallengeWidget}`
    };
  }
  return { hasChallenge: false, reason: '' };
})()
"#;
        let obj = page
            .evaluate(js)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        let val = obj.into_value::<Value>().unwrap_or(Value::Null);
        if val["hasChallenge"].as_bool().unwrap_or(false) {
            return Ok(Some(
                val["reason"]
                    .as_str()
                    .unwrap_or("captcha-like signals found")
                    .to_string(),
            ));
        }
        Ok(None)
    }

    async fn wait_until_no_challenge(
        &self,
        page: &chromiumoxide::Page,
        timeout_ms: u64,
    ) -> Result<bool> {
        let start = std::time::Instant::now();
        while (start.elapsed().as_millis() as u64) < timeout_ms {
            if self.detect_challenge_hint(page).await?.is_none() {
                return Ok(true);
            }
            tokio::time::sleep(Duration::from_millis(800)).await;
        }
        Ok(false)
    }
}

fn chrono_stamp() -> String {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("{ms}")
}

#[cfg(test)]
mod screenshot_tests {
    use super::*;

    #[test]
    fn fit_scale_never_upscales() {
        assert_eq!(fit_scale(800.0, 600.0, 1568.0), 1.0);
        assert_eq!(fit_scale(0.0, 0.0, 1568.0), 1.0);
    }

    #[test]
    fn fit_scale_caps_longest_edge() {
        let s = fit_scale(3136.0, 1000.0, 1568.0);
        assert!((s - 0.5).abs() < 1e-9);
        let s = fit_scale(1000.0, 6000.0, 2000.0);
        assert!((6000.0 * s - 2000.0).abs() < 1e-6);
    }

    #[test]
    fn visual_actions_exclude_reads() {
        assert!(is_visual_action("click"));
        assert!(is_visual_action("navigate"));
        assert!(!is_visual_action("get_text"));
        assert!(!is_visual_action("screenshot"));
        assert!(!is_visual_action("eval_js"));
    }

    #[test]
    fn hash_detects_identical_frames() {
        assert_eq!(hash_bytes(b"abc"), hash_bytes(b"abc"));
        assert_ne!(hash_bytes(b"abc"), hash_bytes(b"abd"));
    }
}
