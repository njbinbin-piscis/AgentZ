//! Smoke-test the migrated automation tools: print their schemas and exercise
//! a couple of read-only actions. Run with `cargo run -p robotz-automation
//! --example inspect`.

use robotz_automation::{DesktopAutomationTool, ScreenTool};
use robotz_core::{Tool, ToolContext};
use serde_json::json;

#[tokio::main]
async fn main() {
    let ctx = ToolContext::default();

    for tool in [
        Box::new(ScreenTool) as Box<dyn Tool>,
        Box::new(DesktopAutomationTool) as Box<dyn Tool>,
    ] {
        println!("== {} (read_only={}) ==", tool.name(), tool.is_read_only());
        println!("{}\n", tool.description());
    }

    let screen = ScreenTool;
    match screen
        .call(json!({ "action": "list_monitors" }), &ctx)
        .await
    {
        Ok(r) => println!("[screen.list_monitors] is_error={}\n{}\n", r.is_error, r.content),
        Err(e) => println!("[screen.list_monitors] errored (expected in headless): {e}\n"),
    }

    let da = DesktopAutomationTool;
    match da
        .call(json!({ "action": "get_cursor_position" }), &ctx)
        .await
    {
        Ok(r) => println!("[desktop_automation.get_cursor_position] {}", r.content),
        Err(e) => println!("[desktop_automation.get_cursor_position] errored: {e}"),
    }
}
