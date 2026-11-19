//! Shared assembly of the standard RobotZ [`Tool`] set.
//!
//! Used by `robotz-mcp`, `robotz-host`, and integration tests so the exposed
//! tool list never drifts between transports.

use std::sync::Arc;

use robotz_browser::{create_browser_manager, BrowserOptions, BrowserTool, SharedBrowserManager};
use robotz_core::Tool;

/// Build the default RobotZ tool instances.
///
/// When `readonly` is true, only observation-safe tools are kept (screenshots,
/// window listing, UIA reads on Windows).
pub fn build_tools(readonly: bool, browser: SharedBrowserManager) -> Vec<Arc<dyn Tool>> {
    let mut tools: Vec<Arc<dyn Tool>> = vec![
        Arc::new(robotz_automation::ScreenTool),
        Arc::new(robotz_automation::DesktopAutomationTool),
        Arc::new(BrowserTool::new(browser)),
    ];

    #[cfg(target_os = "windows")]
    tools.push(Arc::new(robotz_automation::UiaTool));

    if readonly {
        tools.retain(|t| t.is_read_only());
    }

    tools
}

/// Default managed Chrome instance (headless by default).
pub fn default_browser_manager() -> SharedBrowserManager {
    create_browser_manager(BrowserOptions::default())
}
