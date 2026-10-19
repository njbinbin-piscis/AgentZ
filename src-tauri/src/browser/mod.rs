//! IDE Browser panel driver (CDP) backed by [`robotz_browser`].
//!
//! The panel streams PNG screenshots and forwards pointer events. The same
//! [`SharedBrowserManager`] is registered as the agent `browser` tool so
//! automation and the user see one page.

pub mod activity;
pub mod events;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat;
use chromiumoxide::page::{Page, ScreenshotParams};
use piscis_kernel::agent::tool::{Tool, ToolContext, ToolResult};
use robotz_browser::{create_browser_manager, BrowserOptions, SharedBrowserManager};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

/// Tracks page navigation generations and snapshot-bound element refs (AZ-053 / R10-04).
#[derive(Clone, Debug)]
pub struct BrowserGenerationTracker {
    navigation_generation: Arc<AtomicU64>,
    last_snapshot_generation: Arc<AtomicU64>,
}

impl Default for BrowserGenerationTracker {
    fn default() -> Self {
        Self {
            navigation_generation: Arc::new(AtomicU64::new(1)),
            last_snapshot_generation: Arc::new(AtomicU64::new(0)),
        }
    }
}

impl BrowserGenerationTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current_generation(&self) -> u64 {
        self.navigation_generation.load(Ordering::SeqCst)
    }

    pub fn last_snapshot_generation(&self) -> u64 {
        self.last_snapshot_generation.load(Ordering::SeqCst)
    }

    /// Synchronously advances the navigation generation and invalidates prior snapshots.
    pub fn advance_navigation(&self) -> u64 {
        self.navigation_generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Records that a fresh DOM snapshot was captured for the current page generation.
    pub fn record_snapshot(&self) -> u64 {
        let current = self.current_generation();
        self.last_snapshot_generation.store(current, Ordering::SeqCst);
        current
    }

    /// Validates an element `ref` against the current page generation.
    /// Supports explicit `snapshot_generation` parameter, `ref@gen` syntax, or the latest snapshot generation.
    pub fn validate_element_ref(
        &self,
        raw_ref: &str,
        explicit_generation: Option<u64>,
    ) -> std::result::Result<(String, u64), String> {
        let trimmed = raw_ref.trim();
        if trimmed.is_empty() {
            return Err("StaleElementRef: empty element ref".to_string());
        }
        let (clean_ref, inline_gen) = match trimmed.rsplit_once('@') {
            Some((base, gen_str)) if !base.is_empty() => match gen_str.parse::<u64>() {
                Ok(g) => (base.to_string(), Some(g)),
                Err(_) => (trimmed.to_string(), None),
            },
            _ => (trimmed.to_string(), None),
        };
        let current = self.current_generation();
        let expected = explicit_generation
            .or(inline_gen)
            .unwrap_or_else(|| self.last_snapshot_generation());
        if expected == 0 {
            return Err(format!(
                "StaleElementRef: ref '{clean_ref}' used before any snapshot was taken on generation {current}; call browser snapshot first"
            ));
        }
        if expected != current {
            return Err(format!(
                "StaleElementRef: ref '{clean_ref}' belongs to snapshot_generation {expected}, but page is at generation {current} after navigation; call browser snapshot first"
            ));
        }
        Ok((clean_ref, current))
    }
}

/// Generation-guarded wrapper around [`robotz_browser::BrowserTool`].
pub struct GuardedBrowserTool {
    inner: robotz_browser::BrowserTool,
    generations: BrowserGenerationTracker,
}

impl GuardedBrowserTool {
    pub fn new(manager: SharedBrowserManager, generations: BrowserGenerationTracker) -> Self {
        Self {
            inner: robotz_browser::BrowserTool::new(manager),
            generations,
        }
    }
}

#[async_trait]
impl Tool for GuardedBrowserTool {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    fn input_schema(&self) -> serde_json::Value {
        self.inner.input_schema()
    }

    async fn call(&self, mut input: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult> {
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        if matches!(
            action.as_str(),
            "navigate" | "goto" | "open" | "reload" | "back" | "forward" | "close"
        ) {
            self.generations.advance_navigation();
        } else if let Some(raw_ref) = input
            .get("ref")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
        {
            let explicit_gen = input.get("snapshot_generation").and_then(|v| v.as_u64());
            match self.generations.validate_element_ref(raw_ref, explicit_gen) {
                Ok((clean_ref, _)) => {
                    if let Some(obj) = input.as_object_mut() {
                        obj.insert("ref".to_string(), serde_json::Value::String(clean_ref));
                    }
                }
                Err(msg) => return Ok(ToolResult::err(msg)),
            }
        }

        let mut res = self.inner.call(input, ctx).await?;
        if action == "snapshot" && !res.is_error {
            let gen = self.generations.record_snapshot();
            res.content = format!("{}\n[snapshot_generation={gen}]", res.content);
        }
        Ok(res)
    }
}

/// Page scroll geometry, returned to the panel so it can drive a synthetic
/// scrollbar (the screenshot-based view has no native one).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct ScrollInfo {
    pub scroll_x: f64,
    pub scroll_y: f64,
    pub scroll_width: f64,
    pub scroll_height: f64,
    pub client_width: f64,
    pub client_height: f64,
}

/// A picked / inspected DOM element returned to the frontend / agent.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PickedElement {
    pub selector: String,
    pub tag: String,
    pub text: String,
    pub html: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub class_name: String,
    #[serde(default)]
    pub rect_x: f64,
    #[serde(default)]
    pub rect_y: f64,
    #[serde(default)]
    pub rect_width: f64,
    #[serde(default)]
    pub rect_height: f64,
    #[serde(default)]
    pub dom_path: String,
    #[serde(default)]
    pub react_component: String,
}

/// Live browser session snapshot for frontend hydration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BrowserState {
    pub open: bool,
    pub url: String,
    pub viewport_width: u32,
    pub viewport_height: u32,
    #[serde(default)]
    pub snapshot_generation: u64,
}

/// Cloneable handle to the live Chromium session. Stored in `AppState` and
/// shared with the agent `browser` tool (RobotZ).
#[derive(Clone)]
pub struct BrowserManager {
    inner: SharedBrowserManager,
    viewport: Arc<Mutex<(u32, u32)>>,
    generations: BrowserGenerationTracker,
}

impl Default for BrowserManager {
    fn default() -> Self {
        let options = BrowserOptions {
            headless: true,
            ..BrowserOptions::default()
        };
        Self {
            inner: create_browser_manager(options),
            viewport: Arc::new(Mutex::new((1280, 800))),
            generations: BrowserGenerationTracker::new(),
        }
    }
}

impl BrowserManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Shared handle for [`robotz_browser::BrowserTool`] registration.
    pub fn shared(&self) -> SharedBrowserManager {
        self.inner.clone()
    }

    /// Returns the shared generation tracker for DOM snapshot / navigation validation.
    pub fn generations(&self) -> BrowserGenerationTracker {
        self.generations.clone()
    }

    /// Builds a generation-guarded [`GuardedBrowserTool`] bound to this manager.
    pub fn guarded_tool(&self) -> GuardedBrowserTool {
        GuardedBrowserTool::new(self.inner.clone(), self.generations.clone())
    }

    /// Run a closure with the active page, launching Chrome first if needed.
    async fn with_page<T, F, Fut>(&self, f: F) -> Result<T>
    where
        F: FnOnce(Page) -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let mut mgr = self.inner.lock().await;
        mgr.launch().await.context("failed to launch browser")?;
        let page = mgr.active_page().await.context("no active browser page")?;
        f(page.as_ref().clone()).await
    }

    /// Resize the Chromium viewport to match the IDE browser panel (CSS pixels).
    /// Keeps screenshot coordinates 1:1 with the panel for accurate clicks/picks.
    pub async fn set_viewport(&self, width: u32, height: u32) -> Result<(u32, u32)> {
        use chromiumoxide::cdp::browser_protocol::emulation::{
            ScreenOrientation, ScreenOrientationType, SetDeviceMetricsOverrideParams,
        };

        let w = width.clamp(320, 3840);
        let h = height.clamp(200, 2160);
        *self.viewport.lock().await = (w, h);
        self.with_page(|page| async move {
            // Keep screen.width/height in sync with innerWidth/innerHeight — otherwise
            // CSS media queries and min-width layouts still assume a desktop screen
            // (~1280px) and the page overflows the panel by ~1/3.
            let params = SetDeviceMetricsOverrideParams::builder()
                .width(w as i64)
                .height(h as i64)
                .screen_width(w as i64)
                .screen_height(h as i64)
                .device_scale_factor(1.0)
                .mobile(false)
                .screen_orientation(ScreenOrientation {
                    angle: 0,
                    r#type: ScreenOrientationType::LandscapePrimary,
                })
                .build()
                .map_err(|e| anyhow::anyhow!(e))?;
            page.execute(params)
                .await
                .context("set viewport failed")?;
            // Force landscape orientation so sites that check orientation.type
            // don't fall back to mobile layout.
            let _ = page
                .evaluate(r#"(() => {
                    Object.defineProperty(screen.orientation, 'type', { value: 'landscape-primary', configurable: true });
                    Object.defineProperty(screen.orientation, 'angle', { value: 0, configurable: true });
                    window.dispatchEvent(new Event('resize'));
                })()"#)
                .await;
            Ok(())
        })
        .await?;
        Ok((w, h))
    }

    /// Current viewport size (width, height) in CSS pixels.
    pub async fn viewport_size(&self) -> (u32, u32) {
        *self.viewport.lock().await
    }

    /// Read the page's current scroll position and content/viewport extents.
    pub async fn scroll_info(&self) -> Result<ScrollInfo> {
        let val = self.eval(SCROLL_INFO_JS).await?;
        Ok(serde_json::from_value(val).unwrap_or_default())
    }

    /// Scroll the page by a wheel delta (CSS pixels) and report new geometry.
    pub async fn scroll_by(&self, dx: f64, dy: f64) -> Result<ScrollInfo> {
        let js = format!("(() => {{ window.scrollBy({dx}, {dy}); {SCROLL_INFO_JS_BODY} }})()");
        let val = self.eval(&js).await?;
        Ok(serde_json::from_value(val).unwrap_or_default())
    }

    /// Scroll the page to an absolute offset (CSS pixels) and report geometry.
    pub async fn scroll_to(&self, x: f64, y: f64) -> Result<ScrollInfo> {
        let js = format!("(() => {{ window.scrollTo({x}, {y}); {SCROLL_INFO_JS_BODY} }})()");
        let val = self.eval(&js).await?;
        Ok(serde_json::from_value(val).unwrap_or_default())
    }

    /// Navigate to `url` and wait for the load to settle. Returns the final URL.
    pub async fn navigate(&self, url: &str) -> Result<String> {
        self.generations.advance_navigation();
        let target = normalise_url(url);
        self.with_page(|page| async move {
            page.goto(target.as_str())
                .await
                .context("navigation failed")?;
            let _ = page.wait_for_navigation().await;
            current_url(&page).await
        })
        .await
    }

    /// Capture the current page as a base64-encoded PNG.
    pub async fn screenshot_png_base64(&self) -> Result<String> {
        use base64::Engine;
        let bytes = self
            .with_page(|page| async move {
                page.screenshot(
                    ScreenshotParams::builder()
                        .format(CaptureScreenshotFormat::Png)
                        .build(),
                )
                .await
                .context("screenshot failed")
            })
            .await?;
        Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
    }

    /// Capture the current page as raw PNG bytes (for the agent tool to save).
    pub async fn screenshot_png_bytes(&self) -> Result<Vec<u8>> {
        self.with_page(|page| async move {
            page.screenshot(
                ScreenshotParams::builder()
                    .format(CaptureScreenshotFormat::Png)
                    .build(),
            )
            .await
            .context("screenshot failed")
        })
        .await
    }

    /// Evaluate a JS expression and return its JSON value.
    pub async fn eval(&self, script: &str) -> Result<serde_json::Value> {
        let script = script.to_string();
        self.with_page(|page| async move {
            let res = page
                .evaluate(script.as_str())
                .await
                .context("evaluate failed")?;
            Ok(res.into_value().unwrap_or(serde_json::Value::Null))
        })
        .await
    }

    /// Click whatever element is at viewport coordinates (x, y) — used to
    /// forward clicks from the panel screenshot.
    pub async fn click_at(&self, x: f64, y: f64) -> Result<bool> {
        let js = format!(
            "(() => {{ const el = document.elementFromPoint({x}, {y}); if (!el) return false; el.scrollIntoView({{block:'center',inline:'center'}}); el.click(); return true; }})()"
        );
        Ok(self.eval(&js).await?.as_bool().unwrap_or(false))
    }

    /// Identify the element at viewport coordinates (x, y) for pick / inspect.
    pub async fn pick_at(&self, x: f64, y: f64) -> Result<Option<PickedElement>> {
        let js = element_at_point_js(x, y);
        let val = self.eval(&js).await?;
        if val.is_null() {
            return Ok(None);
        }
        Ok(serde_json::from_value(val).ok())
    }

    /// Alias for [`Self::pick_at`] — used for hover-highlight in the panel.
    pub async fn inspect_at(&self, x: f64, y: f64) -> Result<Option<PickedElement>> {
        self.pick_at(x, y).await
    }

    /// Resolve a CSS selector against the live page (for chat `@browser-element` expansion).
    pub async fn query_selector(&self, selector: &str) -> Result<Option<PickedElement>> {
        let sel = serde_json::to_string(selector).context("encode selector")?;
        let js = element_info_js(&format!("document.querySelector({sel})"));
        let val = self.eval(&js).await?;
        if val.is_null() {
            return Ok(None);
        }
        Ok(serde_json::from_value(val).ok())
    }

    /// Click the first element matching a CSS selector (agent automation).
    pub async fn click_selector(&self, selector: &str) -> Result<()> {
        let selector = selector.to_string();
        self.with_page(|page| async move {
            let el = page
                .find_element(selector.as_str())
                .await
                .with_context(|| format!("no element matches '{selector}'"))?;
            el.click().await.context("click failed")?;
            Ok(())
        })
        .await
    }

    /// Type text into the element matching `selector`, optionally pressing Enter.
    pub async fn type_into(&self, selector: &str, text: &str, submit: bool) -> Result<()> {
        let selector = selector.to_string();
        let text = text.to_string();
        self.with_page(|page| async move {
            let el = page
                .find_element(selector.as_str())
                .await
                .with_context(|| format!("no element matches '{selector}'"))?;
            el.click().await.context("focus failed")?;
            el.type_str(&text).await.context("type failed")?;
            if submit {
                el.press_key("Enter").await.context("press Enter failed")?;
            }
            Ok(())
        })
        .await
    }

    /// Return the innerText of `selector` (or the whole body when omitted).
    pub async fn get_text(&self, selector: Option<&str>) -> Result<String> {
        let js = match selector {
            Some(sel) => format!(
                "(() => {{ const e = document.querySelector({sel}); return e ? (e.innerText || '') : ''; }})()",
                sel = serde_json::to_string(sel).unwrap_or_else(|_| "\"\"".into())
            ),
            None => "document.body ? document.body.innerText : ''".to_string(),
        };
        Ok(self.eval(&js).await?.as_str().unwrap_or("").to_string())
    }

    /// The page's current URL.
    pub async fn current_url(&self) -> Result<String> {
        self.with_page(|page| async move { current_url(&page).await })
            .await
    }

    /// Whether a Chromium session is currently live.
    pub async fn is_open(&self) -> bool {
        self.inner.lock().await.is_running()
    }

    /// Current browser session snapshot for the IDE panel.
    pub async fn state(&self) -> BrowserState {
        let open = self.is_open().await;
        let (width, height) = *self.viewport.lock().await;
        let url = if open {
            self.current_url().await.unwrap_or_default()
        } else {
            String::new()
        };
        BrowserState {
            open,
            url,
            viewport_width: width,
            viewport_height: height,
            snapshot_generation: self.generations.current_generation(),
        }
    }

    /// Close the browser.
    pub async fn close(&self) -> Result<()> {
        self.generations.advance_navigation();
        self.inner.lock().await.close().await;
        Ok(())
    }
}

/// Body that returns the page's scroll geometry as a plain object.
const SCROLL_INFO_JS_BODY: &str = r#"
    const se = document.scrollingElement || document.documentElement || document.body;
    if (!se) return { scroll_x: 0, scroll_y: 0, scroll_width: 0, scroll_height: 0, client_width: 0, client_height: 0 };
    return {
        scroll_x: se.scrollLeft || 0,
        scroll_y: se.scrollTop || 0,
        scroll_width: se.scrollWidth || 0,
        scroll_height: se.scrollHeight || 0,
        client_width: se.clientWidth || window.innerWidth || 0,
        client_height: se.clientHeight || window.innerHeight || 0,
    };
"#;

/// Full expression form of [`SCROLL_INFO_JS_BODY`].
const SCROLL_INFO_JS: &str = r#"(() => {
    const se = document.scrollingElement || document.documentElement || document.body;
    if (!se) return { scroll_x: 0, scroll_y: 0, scroll_width: 0, scroll_height: 0, client_width: 0, client_height: 0 };
    return {
        scroll_x: se.scrollLeft || 0,
        scroll_y: se.scrollTop || 0,
        scroll_width: se.scrollWidth || 0,
        scroll_height: se.scrollHeight || 0,
        client_width: se.clientWidth || window.innerWidth || 0,
        client_height: se.clientHeight || window.innerHeight || 0,
    };
})()"#;

async fn current_url(page: &Page) -> Result<String> {
    let res = page.evaluate("window.location.href").await;
    match res {
        Ok(v) => Ok(v.into_value().unwrap_or_default()),
        Err(_) => Ok(String::new()),
    }
}

/// Prepend `https://` when the user typed a bare host, and pass through
/// `about:` / `file:` / explicit schemes untouched.
fn element_at_point_js(x: f64, y: f64) -> String {
    element_info_js(&format!("document.elementFromPoint({x}, {y})"))
}

fn element_info_js(el_expr: &str) -> String {
    format!(
        r#"(() => {{
            const el = {el_expr};
            if (!el) return null;
            const sel = (e) => {{
                if (e.id) return '#' + CSS.escape(e.id);
                let s = e.tagName.toLowerCase();
                if (e.classList && e.classList.length) {{
                    s += '.' + Array.from(e.classList).map(c => CSS.escape(c)).join('.');
                }}
                const p = e.parentElement;
                if (p && p.tagName !== 'BODY' && p.tagName !== 'HTML') {{
                    const same = Array.from(p.children).filter(c => c.tagName === e.tagName);
                    if (same.length > 1) s += ':nth-of-type(' + (same.indexOf(e) + 1) + ')';
                }}
                return s;
            }};
            const domPath = (node) => {{
                const parts = [];
                let cur = node;
                while (cur && cur.nodeType === 1) {{
                    let seg = cur.tagName.toLowerCase();
                    if (cur.id) seg += '#' + CSS.escape(cur.id);
                    else if (cur.classList && cur.classList.length) {{
                        seg += '.' + Array.from(cur.classList).slice(0, 3).map(c => CSS.escape(c)).join('.');
                    }}
                    const parent = cur.parentElement;
                    if (parent && parent.children.length > 1) {{
                        const idx = Array.from(parent.children).indexOf(cur) + 1;
                        seg += `:nth-child(${{idx}})`;
                    }}
                    parts.unshift(seg);
                    if (cur.id === 'root' || cur === document.body || cur === document.documentElement) break;
                    cur = cur.parentElement;
                }}
                return parts.join(' > ');
            }};
            const reactComponent = (node) => {{
                const fiberKey = Object.keys(node).find(k =>
                    k.startsWith('__reactFiber$') || k.startsWith('__reactInternalInstance$')
                );
                let fiber = fiberKey ? node[fiberKey] : null;
                while (fiber) {{
                    const t = fiber.type;
                    if (t) {{
                        if (typeof t === 'string') {{
                            fiber = fiber.return;
                            continue;
                        }}
                        const name = t.displayName || t.name;
                        if (name) return name;
                    }}
                    fiber = fiber.return;
                }}
                return '';
            }};
            const r = el.getBoundingClientRect();
            return {{
                selector: sel(el),
                tag: el.tagName.toLowerCase(),
                id: el.id || '',
                class_name: typeof el.className === 'string' ? el.className : '',
                text: (el.innerText || el.value || '').slice(0, 300),
                html: el.outerHTML.slice(0, 800),
                rect_x: r.x,
                rect_y: r.y,
                rect_width: r.width,
                rect_height: r.height,
                dom_path: domPath(el),
                react_component: reactComponent(el)
            }};
        }})()"#
    )
}

fn normalise_url(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return "about:blank".to_string();
    }
    if trimmed.contains("://") || trimmed.starts_with("about:") {
        return trimmed.to_string();
    }
    format!("https://{trimmed}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_generation_rejects_stale_element_refs_after_navigation() {
        let tracker = BrowserGenerationTracker::new();
        assert_eq!(tracker.current_generation(), 1);
        assert_eq!(tracker.last_snapshot_generation(), 0);

        // Using a ref before any snapshot fails closed with StaleElementRef.
        let err_before = tracker.validate_element_ref("e12", None).unwrap_err();
        assert!(err_before.contains("StaleElementRef"), "{err_before}");

        // Take a snapshot on generation 1 -> ref is valid.
        let snap_gen = tracker.record_snapshot();
        assert_eq!(snap_gen, 1);
        let (clean_ref, validated_gen) = tracker.validate_element_ref("e12@1", None).unwrap();
        assert_eq!(clean_ref, "e12");
        assert_eq!(validated_gen, 1);

        // User or agent navigates -> generation advances to 2, invalidating generation 1 refs.
        let next_gen = tracker.advance_navigation();
        assert_eq!(next_gen, 2);

        let stale_implicit = tracker.validate_element_ref("e12", None).unwrap_err();
        assert!(stale_implicit.contains("StaleElementRef"), "{stale_implicit}");
        assert!(stale_implicit.contains("generation 2"), "{stale_implicit}");

        let stale_explicit = tracker.validate_element_ref("e12", Some(1)).unwrap_err();
        assert!(stale_explicit.contains("StaleElementRef"), "{stale_explicit}");

        // Fresh snapshot on generation 2 restores validity for generation 2 refs.
        assert_eq!(tracker.record_snapshot(), 2);
        let (clean2, gen2) = tracker.validate_element_ref("e42@2", None).unwrap();
        assert_eq!(clean2, "e42");
        assert_eq!(gen2, 2);
    }

    #[tokio::test]
    async fn guarded_browser_tool_fails_closed_on_stale_ref_without_launching_browser() {
        let mgr = BrowserManager::new();
        let tool = mgr.guarded_tool();
        mgr.generations().record_snapshot(); // snapshot at gen 1
        mgr.generations().advance_navigation(); // navigate to gen 2

        let ctx = ToolContext {
            session_id: "test".to_string(),
            workspace_root: std::path::PathBuf::from("."),
            bypass_permissions: false,
            settings: Arc::new(piscis_kernel::agent::tool::ToolSettings::default()),
            max_iterations: None,
            memory_owner_id: "test".to_string(),
            pool_session_id: None,
            tool_use_id: None,
            cancel: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            loop_halt: None,
        };
        let res = tool
            .call(
                serde_json::json!({
                    "action": "click",
                    "ref": "e7@1"
                }),
                &ctx,
            )
            .await
            .expect("tool call should not panic");
        assert!(res.is_error);
        assert!(res.content.contains("StaleElementRef"), "{}", res.content);
    }

    /// Requires a local Chromium install; CI runners do not ship one.
    #[tokio::test]
    #[ignore = "requires local Chromium"]
    async fn launch_with_unique_profile() {
        let mgr = BrowserManager::new();
        mgr.set_viewport(800, 600).await.unwrap();
        mgr.navigate("about:blank")
            .await
            .expect("browser should launch");
        assert!(mgr.is_open().await);
        mgr.close().await.unwrap();
        assert!(!mgr.is_open().await);
    }
}
