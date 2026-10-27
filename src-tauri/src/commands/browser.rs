//! Tauri commands backing the IDE Browser panel. Thin wrappers over the shared
//! [`BrowserManager`] in `AppState`, so the panel and the agent's `browser`
//! tool drive the exact same Chromium page.

use tauri::State;

use crate::browser::activity::BrowserCloseGuard;
use crate::browser::{BrowserState, PickedElement, ScrollInfo};
use crate::state::AppState;

#[tauri::command]
pub async fn browser_navigate(state: State<'_, AppState>, url: String) -> Result<String, String> {
    state
        .browser
        .navigate(&url)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn browser_screenshot(state: State<'_, AppState>) -> Result<String, String> {
    state
        .browser
        .screenshot_png_base64()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn browser_click_at(state: State<'_, AppState>, x: f64, y: f64) -> Result<bool, String> {
    state
        .browser
        .click_at(x, y)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn browser_pick_at(
    state: State<'_, AppState>,
    x: f64,
    y: f64,
) -> Result<Option<PickedElement>, String> {
    state.browser.pick_at(x, y).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn browser_current_url(state: State<'_, AppState>) -> Result<String, String> {
    state.browser.current_url().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn browser_is_open(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.browser.is_open().await)
}

#[tauri::command]
pub async fn browser_close(state: State<'_, AppState>) -> Result<(), String> {
    state.browser.close().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn browser_set_viewport(
    state: State<'_, AppState>,
    width: u32,
    height: u32,
) -> Result<(u32, u32), String> {
    state
        .browser
        .set_viewport(width, height)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn browser_scroll_by(
    state: State<'_, AppState>,
    x: f64,
    y: f64,
) -> Result<ScrollInfo, String> {
    state
        .browser
        .scroll_by(x, y)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn browser_scroll_to(
    state: State<'_, AppState>,
    x: f64,
    y: f64,
) -> Result<ScrollInfo, String> {
    state
        .browser
        .scroll_to(x, y)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn browser_scroll_info(state: State<'_, AppState>) -> Result<ScrollInfo, String> {
    state.browser.scroll_info().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn browser_inspect_at(
    state: State<'_, AppState>,
    x: f64,
    y: f64,
) -> Result<Option<PickedElement>, String> {
    state
        .browser
        .inspect_at(x, y)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn browser_state(state: State<'_, AppState>) -> Result<BrowserState, String> {
    Ok(state.browser.state().await)
}

#[tauri::command]
pub async fn browser_close_guard(state: State<'_, AppState>) -> Result<BrowserCloseGuard, String> {
    Ok(state.browser_activity.close_guard().await)
}

#[tauri::command]
pub async fn browser_insert_text(state: State<'_, AppState>, text: String) -> Result<(), String> {
    state.browser.insert_text(&text).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn browser_press_key(
    state: State<'_, AppState>,
    key: String,
    modifiers: Option<i64>,
) -> Result<(), String> {
    state
        .browser
        .press_key(&key, modifiers.unwrap_or(0))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn browser_history_step(state: State<'_, AppState>, delta: i32) -> Result<(), String> {
    state.browser.history_step(delta).await.map_err(|e| e.to_string())
}
