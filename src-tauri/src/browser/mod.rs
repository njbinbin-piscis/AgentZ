//! IDE Browser panel driver (CDP) backed by [`robotz_browser`].
//!
//! One Chromium process is shared, but each chat session gets its own tab.
//! The panel and that session's `browser` tool both drive that tab, so
//! parallel CodeZ sessions and WorkZ sessions do not navigate or click on
//! each other's pages. Cookies still belong to the shared browser profile.

pub mod activity;
pub mod events;

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use anyhow::{Context, Result};
use async_trait::async_trait;
use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat;
use chromiumoxide::page::{Page, ScreenshotParams};
use piscis_kernel::agent::tool::{Tool, ToolContext, ToolResult};
use robotz_browser::{create_browser_manager, BrowserOptions, SharedBrowserManager};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

/// Longest a single page operation may hold the browser lock.
const PAGE_OP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(45);
/// Longest a caller waits to get the browser lock before reporting it busy.
const PAGE_LOCK_WAIT: std::time::Duration = std::time::Duration::from_secs(20);
/// Longest a single agent `browser` tool action may run.
const AGENT_ACTION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(90);
/// How long navigation waits for the load event before returning anyway.
const NAV_SETTLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(12);

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
    manager: BrowserManager,
}

impl GuardedBrowserTool {
    pub fn new(manager: BrowserManager) -> Self {
        Self {
            inner: robotz_browser::BrowserTool::new(manager.shared()),
            manager,
        }
    }
}

/// Tab id of a session's primary page. Extra tabs created inside the session
/// use the same prefix plus `#`.
pub fn session_tab_id(session_id: &str) -> String {
    let key = session_id.trim();
    let key = if key.is_empty() { "default" } else { key };
    format!("session:{key}")
}

fn tab_belongs_to_session(session_id: &str, tab: &str) -> bool {
    let primary = session_tab_id(session_id);
    tab == primary || tab.starts_with(&format!("{primary}#"))
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
        // Hold the session gate for the whole action so another session cannot
        // retarget the shared "active tab" while RobotZ runs.
        let Ok(_gate) = tokio::time::timeout(PAGE_LOCK_WAIT, self.manager.lock_ops()).await else {
            return Ok(ToolResult::err(
                "browser is busy with another session; try again".to_string(),
            ));
        };

        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        if action == "close" {
            self.manager.close_session_locked(&ctx.session_id).await?;
            return Ok(ToolResult::ok(
                "Closed this session's browser page. Other sessions were left open.",
            ));
        }
        if action == "list_tabs" {
            return Ok(ToolResult::ok(self.manager.session_tab_report(&ctx.session_id)));
        }

        let mut created: Option<String> = None;
        if action == "new_tab" {
            let id = format!(
                "{}#{}",
                session_tab_id(&ctx.session_id),
                &uuid_suffix()
            );
            if let Some(obj) = input.as_object_mut() {
                obj.insert("tab_id".into(), serde_json::Value::String(id.clone()));
            }
            created = Some(id);
        } else if matches!(action.as_str(), "switch_tab" | "close_tab") {
            let requested = input.get("tab_id").and_then(|v| v.as_str()).unwrap_or("");
            if !tab_belongs_to_session(&ctx.session_id, requested) {
                return Ok(ToolResult::err(format!(
                    "tab '{requested}' belongs to another session"
                )));
            }
        } else if let Some(obj) = input.as_object_mut() {
            // Drop a caller-supplied tab id so the action hits the page this
            // session is bound to, not some other session's tab.
            obj.remove("tab_id");
        }

        let tab = self.manager.current_tab(&ctx.session_id);
        let generations = self.manager.tracker(&tab);
        if created.is_none()
            && matches!(
                action.as_str(),
                "navigate" | "goto" | "open" | "reload" | "back" | "forward"
            )
        {
            generations.advance_navigation();
        } else if created.is_none() && action != "close_tab" && action != "switch_tab" {
            if let Some(raw_ref) = input
                .get("ref")
                .and_then(|v| v.as_str())
                .filter(|s| !s.trim().is_empty())
            {
                let explicit_gen = input.get("snapshot_generation").and_then(|v| v.as_u64());
                match generations.validate_element_ref(raw_ref, explicit_gen) {
                    Ok((clean_ref, _)) => {
                        if let Some(obj) = input.as_object_mut() {
                            obj.insert("ref".to_string(), serde_json::Value::String(clean_ref));
                        }
                    }
                    Err(msg) => return Ok(ToolResult::err(msg)),
                }
            }
        }

        if self.manager.auto_screenshot_for(&ctx.session_id) {
            if let Some(obj) = input.as_object_mut() {
                obj.entry("auto_screenshot")
                    .or_insert(serde_json::Value::Bool(true));
            }
        }

        // Launch (or focus) this session's tab before RobotZ reads the active page.
        if let Err(e) = self.manager.bind(&ctx.session_id).await {
            return Ok(ToolResult::err(e.to_string()));
        }

        // RobotZ holds the shared browser lock for the whole action; a page that
        // never settles (open JS dialog, load that never fires) would otherwise
        // hang this turn and every later browser call. Dropping the future
        // releases the lock.
        let cancel = ctx.cancel.clone();
        let cancelled = async move {
            while !cancel.load(std::sync::atomic::Ordering::Relaxed) {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
        };
        let mut res = tokio::select! {
            r = tokio::time::timeout(AGENT_ACTION_TIMEOUT, self.inner.call(input, ctx)) => match r {
                Ok(r) => r?,
                Err(_) => return Ok(ToolResult::err(format!(
                    "browser action '{action}' timed out after {}s (page may be blocked by a dialog or never finished loading); retry, or take a snapshot to inspect the page",
                    AGENT_ACTION_TIMEOUT.as_secs()
                ))),
            },
            _ = cancelled => return Ok(ToolResult::err("browser action cancelled")),
        };
        if action == "snapshot" && !res.is_error {
            let gen = generations.record_snapshot();
            res.content = format!("{}\n[snapshot_generation={gen}]", res.content);
        }
        if !res.is_error {
            if let Some(id) = created {
                self.manager.remember(&ctx.session_id, &id);
            } else if action == "switch_tab" {
                if let Some(id) = input_tab_id_from_result(&res.content) {
                    self.manager.remember(&ctx.session_id, &id);
                }
            } else if action == "close_tab" {
                if let Some(closed) = input_tab_id_from_result(&res.content) {
                    self.manager.forget_tab(&ctx.session_id, &closed);
                }
            }
        }
        Ok(res)
    }
}

fn uuid_suffix() -> String {
    let u = uuid_simple();
    u
}

fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{nanos:x}")
}

/// `Switched to tab: <id>` / unused for new_tab (we already know the id).
fn input_tab_id_from_result(content: &str) -> Option<String> {
    content
        .rsplit_once(": ")
        .map(|(_, id)| id.trim().to_string())
        .filter(|id| !id.is_empty())
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
/// shared with the agent `browser` tool (RobotZ). Tabs are per chat session.
#[derive(Clone)]
pub struct BrowserManager {
    inner: SharedBrowserManager,
    /// Serializes "focus this session's tab, then operate". RobotZ itself only
    /// has one active tab, so concurrent sessions must take turns.
    ops: Arc<Mutex<()>>,
    /// session key -> tab id the panel and tool currently show.
    session_tabs: Arc<StdMutex<HashMap<String, String>>>,
    /// session key -> every tab id that session created.
    owned_tabs: Arc<StdMutex<HashMap<String, HashSet<String>>>>,
    trackers: Arc<StdMutex<HashMap<String, BrowserGenerationTracker>>>,
    viewports: Arc<StdMutex<HashMap<String, (u32, u32)>>>,
    auto_screenshot: Arc<StdMutex<HashMap<String, bool>>>,
}

impl Default for BrowserManager {
    fn default() -> Self {
        let options = BrowserOptions {
            headless: true,
            ..BrowserOptions::default()
        };
        Self {
            inner: create_browser_manager(options),
            ops: Arc::new(Mutex::new(())),
            session_tabs: Arc::new(StdMutex::new(HashMap::new())),
            owned_tabs: Arc::new(StdMutex::new(HashMap::new())),
            trackers: Arc::new(StdMutex::new(HashMap::new())),
            viewports: Arc::new(StdMutex::new(HashMap::new())),
            auto_screenshot: Arc::new(StdMutex::new(HashMap::new())),
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

    pub async fn lock_ops(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.ops.lock().await
    }

    /// Tab the session will use, without launching Chromium.
    pub fn current_tab(&self, session_id: &str) -> String {
        let key = session_tab_id(session_id);
        self.session_tabs
            .lock()
            .unwrap()
            .get(&key)
            .cloned()
            .unwrap_or(key)
    }

    pub fn tracker(&self, tab: &str) -> BrowserGenerationTracker {
        self.trackers
            .lock()
            .unwrap()
            .entry(tab.to_string())
            .or_default()
            .clone()
    }

    fn forget_tab(&self, session_id: &str, tab: &str) {
        let key = session_tab_id(session_id);
        if let Some(set) = self.owned_tabs.lock().unwrap().get_mut(&key) {
            set.remove(tab);
        }
        let mut current = self.session_tabs.lock().unwrap();
        if current.get(&key).map(|t| t == tab).unwrap_or(false) {
            current.insert(key.clone(), key);
        }
    }

    fn remember(&self, session_id: &str, tab: &str) {
        let key = session_tab_id(session_id);
        self.session_tabs
            .lock()
            .unwrap()
            .insert(key.clone(), tab.to_string());
        self.owned_tabs
            .lock()
            .unwrap()
            .entry(key)
            .or_default()
            .insert(tab.to_string());
    }

    fn session_tab_report(&self, session_id: &str) -> String {
        let key = session_tab_id(session_id);
        let current = self.current_tab(session_id);
        let tabs = self
            .owned_tabs
            .lock()
            .unwrap()
            .get(&key)
            .cloned()
            .unwrap_or_default();
        if tabs.is_empty() {
            return "No open tabs in this session".to_string();
        }
        let mut list: Vec<_> = tabs.into_iter().collect();
        list.sort();
        let lines: Vec<_> = list
            .into_iter()
            .map(|t| {
                if t == current {
                    format!("* {t} (active)")
                } else {
                    t
                }
            })
            .collect();
        format!("Open tabs:\n{}", lines.join("\n"))
    }

    /// Focus this session's tab and make it the browser's active page.
    pub async fn bind(&self, session_id: &str) -> Result<String> {
        let tab = self.current_tab(session_id);
        self.remember(session_id, &tab);
        let mut mgr = tokio::time::timeout(PAGE_LOCK_WAIT, self.inner.lock())
            .await
            .map_err(|_| anyhow::anyhow!("browser is busy with another operation; try again"))?;
        mgr.get_or_create_page(&tab)
            .await
            .context("failed to open the session browser tab")?;
        mgr.switch_tab(&tab)
            .context("failed to focus the session browser tab")?;
        Ok(tab)
    }

    pub async fn session_has_page(&self, session_id: &str) -> bool {
        let tab = self.current_tab(session_id);
        let mgr = self.inner.lock().await;
        mgr.is_running() && mgr.list_tabs().iter().any(|t| t == &tab)
    }

    /// Builds a generation-guarded [`GuardedBrowserTool`] bound to this manager.
    pub fn guarded_tool(&self) -> GuardedBrowserTool {
        GuardedBrowserTool::new(self.clone())
    }

    /// Enable per-turn auto screenshots after visual actions (vision-capable models only).
    pub fn set_auto_screenshot(&self, session_id: &str, on: bool) {
        self.auto_screenshot
            .lock()
            .unwrap()
            .insert(session_tab_id(session_id), on);
    }

    fn auto_screenshot_for(&self, session_id: &str) -> bool {
        self.auto_screenshot
            .lock()
            .unwrap()
            .get(&session_tab_id(session_id))
            .copied()
            .unwrap_or(false)
    }

    /// Close every tab and the Chromium process. Used when the project goes away.
    pub async fn close_all(&self) -> Result<()> {
        let _gate = tokio::time::timeout(PAGE_LOCK_WAIT, self.ops.lock())
            .await
            .map_err(|_| anyhow::anyhow!("browser is busy with another session; try again"))?;
        self.session_tabs.lock().unwrap().clear();
        self.owned_tabs.lock().unwrap().clear();
        self.trackers.lock().unwrap().clear();
        self.viewports.lock().unwrap().clear();
        let mut mgr = self.inner.lock().await;
        if mgr.is_running() {
            let _ = mgr.close().await;
        }
        Ok(())
    }

    /// Close only this session's tabs. The Chromium process stays up while any
    /// other session still has a page.
    pub async fn close_session(&self, session_id: &str) -> Result<()> {
        let _gate = tokio::time::timeout(PAGE_LOCK_WAIT, self.ops.lock())
            .await
            .map_err(|_| anyhow::anyhow!("browser is busy with another session; try again"))?;
        self.close_session_locked(session_id).await
    }

    async fn close_session_locked(&self, session_id: &str) -> Result<()> {
        let key = session_tab_id(session_id);
        let mut tabs = self
            .owned_tabs
            .lock()
            .unwrap()
            .remove(&key)
            .unwrap_or_default();
        if let Some(current) = self.session_tabs.lock().unwrap().remove(&key) {
            tabs.insert(current);
        }
        tabs.insert(key);
        let mut mgr = self.inner.lock().await;
        for tab in &tabs {
            if mgr.list_tabs().iter().any(|t| t == tab) {
                let _ = mgr.close_tab(tab).await;
            }
            self.trackers.lock().unwrap().remove(tab);
            self.viewports.lock().unwrap().remove(tab);
        }
        if mgr.list_tabs().is_empty() && mgr.is_running() {
            let _ = mgr.close().await;
        }
        Ok(())
    }

    /// Focus `session_id`'s tab, then run a closure with that page.
    ///
    /// Takes the session gate. Do not call this while already holding `ops`
    /// (it is not re-entrant).
    async fn with_page<T, F, Fut>(&self, session_id: &str, f: F) -> Result<T>
    where
        F: FnOnce(Page) -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let _gate = tokio::time::timeout(PAGE_LOCK_WAIT, self.ops.lock())
            .await
            .map_err(|_| anyhow::anyhow!("browser is busy with another session; try again"))?;
        self.bind(session_id).await?;
        self.with_active_page(f).await
    }

    /// Run a closure with the active page. Caller must already hold `ops` and
    /// have [`Self::bind`]ed the right tab.
    async fn with_active_page<T, F, Fut>(&self, f: F) -> Result<T>
    where
        F: FnOnce(Page) -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        // The manager lock is held for the whole call, so a hung page operation
        // would queue every later panel/agent call behind it (no clicks, no
        // typing, Enter in the address bar does nothing). Bound each phase.
        let mut mgr = tokio::time::timeout(PAGE_LOCK_WAIT, self.inner.lock())
            .await
            .map_err(|_| anyhow::anyhow!("browser is busy with another operation; try again"))?;
        tokio::time::timeout(PAGE_OP_TIMEOUT, mgr.launch())
            .await
            .map_err(|_| anyhow::anyhow!("browser launch timed out"))?
            .context("failed to launch browser")?;
        let page = tokio::time::timeout(PAGE_OP_TIMEOUT, mgr.active_page())
            .await
            .map_err(|_| anyhow::anyhow!("browser did not return an active page"))?
            .context("no active browser page")?;
        tokio::time::timeout(PAGE_OP_TIMEOUT, f(page.as_ref().clone()))
            .await
            .map_err(|_| anyhow::anyhow!("browser operation timed out"))?
    }

    /// Resize the Chromium viewport to match the IDE browser panel (CSS pixels).
    /// Keeps screenshot coordinates 1:1 with the panel for accurate clicks/picks.
    pub async fn set_viewport(&self, session_id: &str, width: u32, height: u32) -> Result<(u32, u32)> {
        use chromiumoxide::cdp::browser_protocol::emulation::{
            ScreenOrientation, ScreenOrientationType, SetDeviceMetricsOverrideParams,
        };

        let w = width.clamp(320, 3840);
        let h = height.clamp(200, 2160);
        let _gate = tokio::time::timeout(PAGE_LOCK_WAIT, self.ops.lock())
            .await
            .map_err(|_| anyhow::anyhow!("browser is busy with another session; try again"))?;
        let tab = self.bind(session_id).await?;
        self.viewports.lock().unwrap().insert(tab, (w, h));
        self.with_active_page(|page| async move {
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
    pub async fn viewport_size(&self, session_id: &str) -> (u32, u32) {
        let tab = self.current_tab(session_id);
        self.viewports
            .lock()
            .unwrap()
            .get(&tab)
            .copied()
            .unwrap_or((1280, 800))
    }

    /// Read the page's current scroll position and content/viewport extents.
    pub async fn scroll_info(&self, session_id: &str) -> Result<ScrollInfo> {
        let val = self.eval(session_id, SCROLL_INFO_JS).await?;
        Ok(serde_json::from_value(val).unwrap_or_default())
    }

    /// Scroll the page by a wheel delta (CSS pixels) and report new geometry.
    pub async fn scroll_by(&self, session_id: &str, dx: f64, dy: f64) -> Result<ScrollInfo> {
        let js = format!("(() => {{ window.scrollBy({dx}, {dy}); {SCROLL_INFO_JS_BODY} }})()");
        let val = self.eval(session_id, &js).await?;
        Ok(serde_json::from_value(val).unwrap_or_default())
    }

    /// Scroll the page to an absolute offset (CSS pixels) and report geometry.
    pub async fn scroll_to(&self, session_id: &str, x: f64, y: f64) -> Result<ScrollInfo> {
        let js = format!("(() => {{ window.scrollTo({x}, {y}); {SCROLL_INFO_JS_BODY} }})()");
        let val = self.eval(session_id, &js).await?;
        Ok(serde_json::from_value(val).unwrap_or_default())
    }

    /// Navigate to `url` and wait for the load to settle. Returns the final URL.
    pub async fn navigate(&self, session_id: &str, url: &str) -> Result<String> {
        let tab = self.current_tab(session_id);
        self.tracker(&tab).advance_navigation();
        let target = normalise_url(url);
        self.with_page(session_id, |page| async move {
            page.goto(target.as_str())
                .await
                .context("navigation failed")?;
            // Pages that never fire `load` must not pin the browser lock.
            let _ = tokio::time::timeout(NAV_SETTLE_TIMEOUT, page.wait_for_navigation()).await;
            current_url(&page).await
        })
        .await
    }

    /// Capture the current page as a base64-encoded PNG.
    pub async fn screenshot_png_base64(&self, session_id: &str) -> Result<String> {
        use base64::Engine;
        let bytes = self
            .with_page(session_id, |page| async move {
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
    pub async fn screenshot_png_bytes(&self, session_id: &str) -> Result<Vec<u8>> {
        self.with_page(session_id, |page| async move {
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
    pub async fn eval(&self, session_id: &str, script: &str) -> Result<serde_json::Value> {
        let script = script.to_string();
        self.with_page(session_id, |page| async move {
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
    pub async fn click_at(&self, session_id: &str, x: f64, y: f64) -> Result<bool> {
        // Real input events first: they move focus into inputs, follow links and
        // trigger form controls the way a user click does.
        if self.mouse_click(session_id, x, y).await.is_ok() {
            return Ok(true);
        }
        let js = format!(
            "(() => {{ const el = document.elementFromPoint({x}, {y}); if (!el) return false; el.scrollIntoView({{block:'center',inline:'center'}}); el.click(); return true; }})()"
        );
        Ok(self.eval(session_id, &js).await?.as_bool().unwrap_or(false))
    }

    /// Dispatch a genuine left click (move, press, release) at viewport coordinates.
    pub async fn mouse_click(&self, session_id: &str, x: f64, y: f64) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::input::{
            DispatchMouseEventParams, DispatchMouseEventType, MouseButton,
        };
        self.with_page(session_id, |page| async move {
            for (kind, button, count) in [
                (DispatchMouseEventType::MouseMoved, MouseButton::None, 0),
                (DispatchMouseEventType::MousePressed, MouseButton::Left, 1),
                (DispatchMouseEventType::MouseReleased, MouseButton::Left, 1),
            ] {
                let params = DispatchMouseEventParams::builder()
                    .r#type(kind)
                    .x(x)
                    .y(y)
                    .button(button)
                    .click_count(count)
                    .build()
                    .map_err(|e| anyhow::anyhow!(e))?;
                page.execute(params).await.context("mouse event failed")?;
            }
            Ok(())
        })
        .await
    }

    /// Insert text at the focused element (typing, IME and paste).
    pub async fn insert_text(&self, session_id: &str, text: &str) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::input::InsertTextParams;
        let text = text.to_string();
        self.with_page(session_id, |page| async move {
            page.execute(InsertTextParams::new(text))
                .await
                .context("insert text failed")?;
            Ok(())
        })
        .await
    }

    /// Press a non-printable key (Enter, Backspace, arrows, …) with modifiers
    /// (bit 1 = Alt, 2 = Ctrl, 4 = Meta, 8 = Shift).
    pub async fn press_key(&self, session_id: &str, key: &str, modifiers: i64) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::input::{
            DispatchKeyEventParams, DispatchKeyEventType,
        };
        let Some((code, vk)) = special_key(key) else {
            anyhow::bail!("unsupported key '{key}'");
        };
        let key = key.to_string();
        self.with_page(session_id, |page| async move {
            for kind in [DispatchKeyEventType::RawKeyDown, DispatchKeyEventType::KeyUp] {
                let mut b = DispatchKeyEventParams::builder()
                    .r#type(kind.clone())
                    .key(key.clone())
                    .code(code)
                    .windows_virtual_key_code(vk)
                    .native_virtual_key_code(vk)
                    .modifiers(modifiers);
                if key == "Enter" && matches!(kind, DispatchKeyEventType::RawKeyDown) {
                    b = b.text("\r");
                }
                let params = b.build().map_err(|e| anyhow::anyhow!(e))?;
                page.execute(params).await.context("key event failed")?;
            }
            Ok(())
        })
        .await
    }

    /// Go back / forward in the page history.
    pub async fn history_step(&self, session_id: &str, delta: i32) -> Result<()> {
        let js = format!("history.go({delta})");
        self.eval(session_id, &js).await.map(|_| ())
    }

    /// Identify the element at viewport coordinates (x, y) for pick / inspect.
    pub async fn pick_at(&self, session_id: &str, x: f64, y: f64) -> Result<Option<PickedElement>> {
        let js = element_at_point_js(x, y);
        let val = self.eval(session_id, &js).await?;
        if val.is_null() {
            return Ok(None);
        }
        Ok(serde_json::from_value(val).ok())
    }

    /// Alias for [`Self::pick_at`] — used for hover-highlight in the panel.
    pub async fn inspect_at(&self, session_id: &str, x: f64, y: f64) -> Result<Option<PickedElement>> {
        self.pick_at(session_id, x, y).await
    }

    /// Resolve a CSS selector against the live page (for chat `@browser-element` expansion).
    pub async fn query_selector(&self, session_id: &str, selector: &str) -> Result<Option<PickedElement>> {
        let sel = serde_json::to_string(selector).context("encode selector")?;
        let js = element_info_js(&format!("document.querySelector({sel})"));
        let val = self.eval(session_id, &js).await?;
        if val.is_null() {
            return Ok(None);
        }
        Ok(serde_json::from_value(val).ok())
    }

    /// Click the first element matching a CSS selector (agent automation).
    pub async fn click_selector(&self, session_id: &str, selector: &str) -> Result<()> {
        let selector = selector.to_string();
        self.with_page(session_id, |page| async move {
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
    pub async fn type_into(&self, session_id: &str, selector: &str, text: &str, submit: bool) -> Result<()> {
        let selector = selector.to_string();
        let text = text.to_string();
        self.with_page(session_id, |page| async move {
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
    pub async fn get_text(&self, session_id: &str, selector: Option<&str>) -> Result<String> {
        let js = match selector {
            Some(sel) => format!(
                "(() => {{ const e = document.querySelector({sel}); return e ? (e.innerText || '') : ''; }})()",
                sel = serde_json::to_string(sel).unwrap_or_else(|_| "\"\"".into())
            ),
            None => "document.body ? document.body.innerText : ''".to_string(),
        };
        Ok(self.eval(session_id, &js).await?.as_str().unwrap_or("").to_string())
    }

    /// The page's current URL.
    pub async fn current_url(&self, session_id: &str) -> Result<String> {
        self.with_page(session_id, |page| async move { current_url(&page).await })
            .await
    }

    /// Whether a Chromium session is currently live.
    pub async fn is_open(&self) -> bool {
        self.inner.lock().await.is_running()
    }

    /// Current browser session snapshot for the IDE panel.
    pub async fn state(&self, session_id: &str) -> BrowserState {
        let tab = self.current_tab(session_id);
        let open = self.session_has_page(session_id).await;
        let (width, height) = self
            .viewports
            .lock()
            .unwrap()
            .get(&tab)
            .copied()
            .unwrap_or((1280, 800));
        let url = if open {
            self.current_url(session_id).await.unwrap_or_default()
        } else {
            String::new()
        };
        BrowserState {
            open,
            url,
            viewport_width: width,
            viewport_height: height,
            snapshot_generation: self.tracker(&tab).current_generation(),
        }
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

/// DOM `code` and Windows virtual key code for keys that are not plain text.
fn special_key(key: &str) -> Option<(&'static str, i64)> {
    Some(match key {
        "Enter" => ("Enter", 13),
        "Backspace" => ("Backspace", 8),
        "Tab" => ("Tab", 9),
        "Escape" => ("Escape", 27),
        "Delete" => ("Delete", 46),
        "ArrowLeft" => ("ArrowLeft", 37),
        "ArrowUp" => ("ArrowUp", 38),
        "ArrowRight" => ("ArrowRight", 39),
        "ArrowDown" => ("ArrowDown", 40),
        "Home" => ("Home", 36),
        "End" => ("End", 35),
        "PageUp" => ("PageUp", 33),
        "PageDown" => ("PageDown", 34),
        _ => return None,
    })
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
        let tab = session_tab_id("test");
        mgr.tracker(&tab).record_snapshot(); // snapshot at gen 1
        mgr.tracker(&tab).advance_navigation(); // navigate to gen 2

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
        mgr.set_viewport("t", 800, 600).await.unwrap();
        mgr.navigate("t", "about:blank")
            .await
            .expect("browser should launch");
        assert!(mgr.is_open().await);
        mgr.close_session("t").await.unwrap();
        assert!(!mgr.is_open().await);
    }

    #[test]
    fn sessions_do_not_share_tabs_or_snapshot_generation() {
        let mgr = BrowserManager::new();
        let a = mgr.current_tab("sess-a");
        let b = mgr.current_tab("sess-b");
        assert_ne!(a, b);
        assert!(tab_belongs_to_session("sess-a", &a));
        assert!(!tab_belongs_to_session("sess-a", &b));
        mgr.tracker(&a).record_snapshot();
        mgr.tracker(&a).advance_navigation();
        assert_eq!(mgr.tracker(&b).current_generation(), 1);
        assert_eq!(mgr.tracker(&a).current_generation(), 2);
        assert_eq!(mgr.session_tab_report("sess-a"), "No open tabs in this session");
    }
}
