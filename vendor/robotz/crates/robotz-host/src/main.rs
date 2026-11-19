//! `robotz-host` — example host with a visual test panel.
//!
//! ```bash
//! cargo run -p robotz-host                    # test panel (default)
//! cargo run -p robotz-host -- tools           # list tools
//! cargo run -p robotz-host -- inspect         # read-only smoke
//! cargo run -p robotz-host -- mcp-demo        # MCP subprocess demo
//! cargo run -p robotz-host -- bench           # benchmark → JSON report
//! ```

use std::sync::Arc;

use robotz_host::{
    find_robotz_mcp_binary, host_data_dir, mcp_demo_sync, run_benchmark, write_report,
    BenchOptions, PanelApp, ToolRunner, WINDOW_TITLE,
};
use robotz_toolset::{build_tools, default_browser_manager};
use tracing::info;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("panel");

    let browser = default_browser_manager();
    let tools = build_tools(false, browser);
    let runner = Arc::new(ToolRunner::new(tools));

    match mode {
        "panel" | "" => run_panel(runner),
        "tools" => {
            list_tools(&runner);
            Ok(())
        }
        "inspect" => {
            run_inspect(&runner);
            Ok(())
        }
        "mcp-demo" => {
            let readonly = args.iter().any(|a| a == "--readonly");
            info!("mcp-demo via {}", find_robotz_mcp_binary().display());
            mcp_demo_sync(readonly)
        }
        "bench" => {
            run_bench_cli(&runner, &args[2..]);
            Ok(())
        }
        other => {
            eprintln!(
                "Unknown mode '{other}'. Usage:\n  \
                 robotz-host [panel|tools|inspect|mcp-demo|bench] [--readonly] [--out PATH]"
            );
            std::process::exit(1);
        }
    }
}

fn run_panel(runner: Arc<ToolRunner>) -> anyhow::Result<()> {
    info!("starting RobotZ test panel");
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(WINDOW_TITLE)
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };
    eframe::run_native(
        WINDOW_TITLE,
        native_options,
        Box::new(|_cc| Ok(Box::new(PanelApp::new(runner)))),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}

fn list_tools(runner: &ToolRunner) {
    for t in runner.tools() {
        println!(
            "- {} (read_only={})",
            t.name(),
            t.is_read_only()
        );
        println!("  {}\n", t.description().lines().next().unwrap_or(""));
    }
}

fn run_inspect(runner: &ToolRunner) {
    use serde_json::json;
    for (tool, input) in [
        (
            "screen_capture",
            json!({ "action": "list_monitors" }),
        ),
        (
            "desktop_automation",
            json!({ "action": "get_cursor_position" }),
        ),
    ] {
        match runner.call_sync(tool, input) {
            Ok(r) => println!("[{tool}] is_error={}\n{}\n", r.is_error, r.content),
            Err(e) => println!("[{tool}] error: {e}\n"),
        }
    }
}

fn run_bench_cli(runner: &ToolRunner, extra_args: &[String]) {
    let mut out = host_data_dir().join("bench-latest.json");
    for (i, a) in extra_args.iter().enumerate() {
        if a == "--out" {
            if let Some(p) = extra_args.get(i + 1) {
                out = p.into();
            }
        }
    }
    let report = run_benchmark(
        runner,
        BenchOptions {
            include_heavy_capture: true,
            ..Default::default()
        },
    );
    match write_report(&out, &report) {
        Ok(()) => println!(
            "Benchmark: {}/{} passed → {}",
            report.passed,
            report.cases.len(),
            out.display()
        ),
        Err(e) => eprintln!("Failed to write report: {e}"),
    }
    for c in &report.cases {
        let mark = if c.ok { "✓" } else { "✗" };
        println!(
            "  {mark} {} ({}ms) {}",
            c.name, c.duration_ms, c.detail
        );
        if let Some(ref err) = c.error {
            println!("      error: {err}");
        }
    }
}
