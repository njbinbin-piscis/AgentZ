//! Accessibility-style page snapshots with stable opaque refs (e1, e2, …).
//!
//! Builds a compact YAML-like tree from the DOM (roles, names, states) and
//! caches ref → selector/bounding-box mappings for subsequent tool actions.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use chromiumoxide::Page;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::RwLock;

const DEFAULT_MAX_DEPTH: u32 = 20;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefEntry {
    /// Navigation generation of the tab when the snapshot that produced this ref was captured.
    #[serde(default)]
    pub generation: u64,
    pub selector: String,
    pub role: String,
    pub name: String,
    pub rect_x: f64,
    pub rect_y: f64,
    pub rect_width: f64,
    pub rect_height: f64,
}

#[derive(Default)]
struct TabSnapshotState {
    refs: HashMap<String, RefEntry>,
    last_tree: String,
    locked: bool,
    /// Bumped by every navigation or reload of the tab. Refs from an older generation are stale.
    generation: u64,
}

#[derive(Default)]
struct SnapshotStore {
    tabs: HashMap<String, TabSnapshotState>,
    locked_tab: Option<String>,
    /// Tab whose lock was dropped by a navigation. Reported once on that tab's next action.
    lost_lock: Option<String>,
}

static SNAPSHOT_STORE: Lazy<Arc<RwLock<SnapshotStore>>> =
    Lazy::new(|| Arc::new(RwLock::new(SnapshotStore::default())));

/// Start a new page generation for `tab_id` before a navigation or reload. Awaited by the
/// caller, so no snapshot or ref lookup can interleave with the change. Refs from earlier
/// generations are kept (marked stale) so their use fails with an explicit error. A lock on the
/// tab is released and remembered so the next action reports it.
pub async fn invalidate_tab(tab_id: &str) {
    let mut guard = SNAPSHOT_STORE.write().await;
    let s = &mut *guard;
    let state = s.tabs.entry(tab_id.to_string()).or_default();
    state.generation += 1;
    state.locked = false;
    state.last_tree.clear();
    if s.locked_tab.as_deref() == Some(tab_id) {
        s.locked_tab = None;
        s.lost_lock = Some(tab_id.to_string());
    }
}

/// Fail once if a navigation released the lock on `tab_id`. Returns `Ok` afterwards.
pub async fn take_lock_loss(tab_id: &str) -> Result<()> {
    let mut s = SNAPSHOT_STORE.write().await;
    if s.lost_lock.as_deref() == Some(tab_id) {
        s.lost_lock = None;
        return Err(anyhow!(
            "Browser lock on tab '{}' was released because the page navigated. Call lock again before locked actions.",
            tab_id
        ));
    }
    Ok(())
}

/// Current page generation of `tab_id` (0 when the tab has never been seen).
pub async fn tab_generation(tab_id: &str) -> u64 {
    SNAPSHOT_STORE
        .read()
        .await
        .tabs
        .get(tab_id)
        .map(|t| t.generation)
        .unwrap_or(0)
}

/// Store a finished snapshot only if the tab is still on the generation it started in.
/// A navigation during capture makes the refs describe a page that is gone, so they are dropped.
pub async fn commit_snapshot(
    tab_id: &str,
    started_generation: u64,
    refs: HashMap<String, RefEntry>,
    tree: String,
) -> Result<()> {
    let mut s = SNAPSHOT_STORE.write().await;
    let state = s.tabs.entry(tab_id.to_string()).or_default();
    if state.generation != started_generation {
        return Err(anyhow!(
            "The page navigated while the snapshot was being captured. Call snapshot again."
        ));
    }
    let generation = state.generation;
    state.refs = refs
        .into_iter()
        .map(|(k, mut v)| {
            v.generation = generation;
            (k, v)
        })
        .collect();
    state.last_tree = tree;
    Ok(())
}

pub async fn lock_tab(tab_id: &str) -> Result<()> {
    let mut s = SNAPSHOT_STORE.write().await;
    s.locked_tab = Some(tab_id.to_string());
    s.lost_lock = None;
    s.tabs.entry(tab_id.to_string()).or_default().locked = true;
    Ok(())
}

pub async fn unlock_tab() -> Result<()> {
    let mut s = SNAPSHOT_STORE.write().await;
    if let Some(tab) = s.locked_tab.take() {
        if let Some(state) = s.tabs.get_mut(&tab) {
            state.locked = false;
        }
    }
    Ok(())
}

pub async fn assert_not_locked_for_switch(current_tab: &str) -> Result<()> {
    let s = SNAPSHOT_STORE.read().await;
    if let Some(locked) = s.locked_tab.as_deref() {
        if locked != current_tab {
            return Err(anyhow!(
                "Browser is locked to tab '{}'. Call unlock before switching tabs.",
                locked
            ));
        }
    }
    Ok(())
}

pub async fn resolve_ref(tab_id: &str, ref_id: &str) -> Result<RefEntry> {
    let s = SNAPSHOT_STORE.read().await;
    let unknown = || {
        anyhow!(
            "Unknown ref '{}'. Call snapshot first to refresh the ref map.",
            ref_id
        )
    };
    let tab = s.tabs.get(tab_id).ok_or_else(unknown)?;
    let entry = tab.refs.get(ref_id).ok_or_else(unknown)?;
    if entry.generation != tab.generation {
        return Err(anyhow!(
            "Ref '{}' belongs to an earlier page state; the tab navigated since that snapshot. Call snapshot again.",
            ref_id
        ));
    }
    Ok(entry.clone())
}

pub async fn ref_bounding_box(tab_id: &str, ref_id: &str) -> Result<(f64, f64, f64, f64)> {
    let entry = resolve_ref(tab_id, ref_id).await?;
    Ok((
        entry.rect_x,
        entry.rect_y,
        entry.rect_width,
        entry.rect_height,
    ))
}

pub struct SnapshotOptions {
    pub interactive: bool,
    pub max_depth: u32,
    pub compact: bool,
    pub selector: Option<String>,
    pub include_diff: bool,
}

impl Default for SnapshotOptions {
    fn default() -> Self {
        Self {
            interactive: false,
            max_depth: DEFAULT_MAX_DEPTH,
            compact: false,
            selector: None,
            include_diff: false,
        }
    }
}

pub async fn capture_snapshot(page: &Page, tab_id: &str, opts: SnapshotOptions) -> Result<String> {
    let interactive = opts.interactive;
    let max_depth = opts.max_depth;
    let compact = opts.compact;
    let root_selector = opts
        .selector
        .as_deref()
        .map(|s| serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into()))
        .unwrap_or_else(|| "null".into());

    let js = format!(
        r#"(() => {{
  const interactive = {interactive};
  const maxDepth = {max_depth};
  const compact = {compact};
  const rootSel = {root_selector};

  function roleOf(el) {{
    const explicit = el.getAttribute('role');
    if (explicit) return explicit;
    const tag = el.tagName.toLowerCase();
    if (tag === 'a' && el.href) return 'link';
    if (tag === 'button') return 'button';
    if (tag === 'input') {{
      const t = (el.type || 'text').toLowerCase();
      if (t === 'checkbox') return 'checkbox';
      if (t === 'radio') return 'radio';
      if (t === 'submit' || t === 'button') return 'button';
      return 'textbox';
    }}
    if (tag === 'select') return 'combobox';
    if (tag === 'textarea') return 'textbox';
    if (tag === 'img' && el.alt) return 'image';
    if (tag === 'h1' || tag === 'h2' || tag === 'h3') return 'heading';
    return tag;
  }}

  function nameOf(el) {{
    return (
      el.getAttribute('aria-label') ||
      el.getAttribute('title') ||
      el.getAttribute('placeholder') ||
      el.getAttribute('alt') ||
      (el.labels && el.labels[0] && el.labels[0].textContent) ||
      el.textContent ||
      ''
    ).trim().slice(0, 120);
  }}

  function isInteractive(el) {{
    const tag = el.tagName.toLowerCase();
    if (['a','button','input','select','textarea'].includes(tag)) return true;
    const role = el.getAttribute('role');
    if (role && /button|link|checkbox|radio|menuitem|tab|option|combobox|textbox|searchbox|switch/.test(role)) return true;
    if (el.hasAttribute('onclick') || el.hasAttribute('tabindex')) return true;
    return false;
  }}

  function bestSelector(el) {{
    if (el.id) return '#' + CSS.escape(el.id);
    const testId = el.getAttribute('data-testid');
    if (testId) return `[data-testid="${{CSS.escape(testId)}}"]`;
    const name = el.getAttribute('name');
    if (name) return `${{el.tagName.toLowerCase()}}[name="${{CSS.escape(name)}}"]`;
    const aria = el.getAttribute('aria-label');
    if (aria) return `${{el.tagName.toLowerCase()}}[aria-label="${{CSS.escape(aria)}}"]`;
    let s = el.tagName.toLowerCase();
    if (el.classList && el.classList.length) {{
      s += '.' + Array.from(el.classList).slice(0, 2).map(c => CSS.escape(c)).join('.');
    }}
    const p = el.parentElement;
    if (p) {{
      const same = Array.from(p.children).filter(c => c.tagName === el.tagName);
      if (same.length > 1) s += `:nth-of-type(${{same.indexOf(el) + 1}})`;
    }}
    return s;
  }}

  function visible(el) {{
    const r = el.getBoundingClientRect();
    if (!r.width || !r.height) return false;
    const st = window.getComputedStyle(el);
    return st.visibility !== 'hidden' && st.display !== 'none' && st.opacity !== '0';
  }}

  const root = rootSel ? document.querySelector(rootSel) : document.body;
  if (!root) return {{ error: 'Root selector not found', tree: '', refs: [] }};

  const refs = [];
  let counter = 0;
  const lines = [];

  function walk(el, depth) {{
    if (!el || depth > maxDepth) return;
    if (el.nodeType !== 1) return;
    const tag = el.tagName.toLowerCase();
    if (tag === 'script' || tag === 'style' || tag === 'noscript') return;
    const inter = isInteractive(el);
    if (interactive && !inter) {{
      for (const ch of el.children || []) walk(ch, depth + 1);
      return;
    }}
    if (!visible(el) && depth > 0) {{
      for (const ch of el.children || []) walk(ch, depth + 1);
      return;
    }}
    counter += 1;
    const ref = 'e' + counter;
    const role = roleOf(el);
    const name = nameOf(el);
    const r = el.getBoundingClientRect();
    const sel = bestSelector(el);
    refs.push({{
      ref,
      selector: sel,
      role,
      name,
      rect_x: r.x,
      rect_y: r.y,
      rect_width: r.width,
      rect_height: r.height,
    }});
    const indent = compact ? '' : '  '.repeat(depth);
    const stateParts = [];
    if (el.disabled) stateParts.push('disabled');
    if (el.checked) stateParts.push('checked');
    if (el.getAttribute('aria-expanded') === 'true') stateParts.push('expanded');
    const stateStr = stateParts.length ? ` [${{stateParts.join(',')}}]` : '';
    const nameStr = name ? ` "${{name.replace(/"/g, '\\"')}}"` : '';
    lines.push(`${{indent}}- ${{ref}} ${{role}}${{nameStr}}${{stateStr}}`);
    for (const ch of el.children || []) walk(ch, depth + 1);
  }}

  walk(root, 0);
  return {{ tree: lines.join('\n'), refs }};
}})()"#
    );

    // Generation before the page is read; a navigation during the read makes the result stale.
    let started_generation = tab_generation(tab_id).await;
    let val = page
        .evaluate(js.as_str())
        .await
        .map_err(|e| anyhow!("snapshot evaluate failed: {e}"))?
        .into_value()
        .unwrap_or(Value::Null);

    if let Some(err) = val.get("error").and_then(|v| v.as_str()) {
        return Err(anyhow!("{err}"));
    }

    let tree = val
        .get("tree")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let mut ref_map = HashMap::new();
    if let Some(arr) = val.get("refs").and_then(|v| v.as_array()) {
        for item in arr {
            let ref_id = item
                .get("ref")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if ref_id.is_empty() {
                continue;
            }
            ref_map.insert(
                ref_id.clone(),
                RefEntry {
                    generation: 0,
                    selector: item
                        .get("selector")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    role: item
                        .get("role")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    name: item
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    rect_x: item.get("rect_x").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    rect_y: item.get("rect_y").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    rect_width: item
                        .get("rect_width")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0),
                    rect_height: item
                        .get("rect_height")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0),
                },
            );
        }
    }

    let mut output = format!(
        "Page snapshot ({} refs). Use ref with click/type/fill/assert/hover.\n\n{}",
        ref_map.len(),
        tree
    );

    if opts.include_diff {
        let prev = {
            let s = SNAPSHOT_STORE.read().await;
            s.tabs
                .get(tab_id)
                .map(|t| t.last_tree.clone())
                .unwrap_or_default()
        };
        if !prev.is_empty() && prev != tree {
            output.push_str("\n\n--- diff: snapshot changed since last capture ---");
        } else if !prev.is_empty() {
            output.push_str("\n\n--- diff: no structural change ---");
        }
    }

    commit_snapshot(tab_id, started_generation, ref_map, tree).await?;

    Ok(output)
}

#[cfg(test)]
mod generation_tests {
    use super::*;

    fn entry(selector: &str) -> RefEntry {
        RefEntry {
            generation: 0,
            selector: selector.into(),
            role: "button".into(),
            name: "Go".into(),
            rect_x: 0.0,
            rect_y: 0.0,
            rect_width: 1.0,
            rect_height: 1.0,
        }
    }

    fn refs(selector: &str) -> HashMap<String, RefEntry> {
        HashMap::from([("e1".to_string(), entry(selector))])
    }

    #[tokio::test]
    async fn ref_from_before_navigation_is_rejected_explicitly() {
        let tab = "gen-test-stale-ref";
        let started = tab_generation(tab).await;
        commit_snapshot(tab, started, refs("#go"), "tree".into()).await.unwrap();
        assert_eq!(resolve_ref(tab, "e1").await.unwrap().selector, "#go");

        invalidate_tab(tab).await;
        let err = resolve_ref(tab, "e1").await.unwrap_err().to_string();
        assert!(err.contains("earlier page state"), "{err}");
    }

    #[tokio::test]
    async fn navigation_during_capture_discards_the_snapshot() {
        let tab = "gen-test-capture-race";
        let started = tab_generation(tab).await;
        invalidate_tab(tab).await; // navigation happens while the page is being read
        let err = commit_snapshot(tab, started, refs("#old"), "old".into())
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("navigated while"), "{err}");
        assert!(resolve_ref(tab, "e1").await.is_err());
    }

    #[tokio::test]
    async fn lock_released_by_navigation_is_reported_once() {
        let tab = "gen-test-lock-loss";
        lock_tab(tab).await.unwrap();
        invalidate_tab(tab).await;
        let first = take_lock_loss(tab).await.unwrap_err().to_string();
        assert!(first.contains("released because the page navigated"), "{first}");
        assert!(take_lock_loss(tab).await.is_ok());
    }

    #[tokio::test]
    async fn fresh_snapshot_after_navigation_resolves() {
        let tab = "gen-test-recover";
        invalidate_tab(tab).await;
        let started = tab_generation(tab).await;
        commit_snapshot(tab, started, refs("#new"), "new".into()).await.unwrap();
        assert_eq!(resolve_ref(tab, "e1").await.unwrap().selector, "#new");
    }
}

pub async fn get_ref_map_json(tab_id: &str) -> Value {
    let s = SNAPSHOT_STORE.read().await;
    let refs: Vec<Value> = s
        .tabs
        .get(tab_id)
        .map(|t| {
            t.refs
                .iter()
                .map(|(k, v)| {
                    json!({
                        "ref": k,
                        "selector": v.selector,
                        "role": v.role,
                        "name": v.name,
                        "rect": [v.rect_x, v.rect_y, v.rect_width, v.rect_height],
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    json!({ "refs": refs })
}
