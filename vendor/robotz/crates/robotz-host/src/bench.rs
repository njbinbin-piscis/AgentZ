//! Computer-use benchmark suite — produces `bench-report.json`.

use std::path::Path;
use std::time::Instant;

use anyhow::Result;
use serde::Serialize;
use serde_json::json;

use crate::runner::ToolRunner;

#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkReport {
    pub generated_at: String,
    pub platform: String,
    pub arch: String,
    pub hostname: String,
    pub transport: String,
    pub cases: Vec<BenchCase>,
    pub passed: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct BenchCase {
    pub name: String,
    pub ok: bool,
    pub duration_ms: u64,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub struct BenchOptions {
    /// When set, run click-accuracy against these physical pixel targets.
    pub click_targets: Vec<(i32, i32)>,
    pub include_heavy_capture: bool,
}

impl Default for BenchOptions {
    fn default() -> Self {
        Self {
            click_targets: Vec::new(),
            include_heavy_capture: true,
        }
    }
}

pub fn run_benchmark(runner: &ToolRunner, opts: BenchOptions) -> BenchmarkReport {
    let mut cases = Vec::new();

    bench_case(&mut cases, "list_monitors", || {
        let r = runner.call_sync(
            "screen_capture",
            json!({ "action": "list_monitors" }),
        )?;
        Ok((!r.is_error, r.content.lines().next().unwrap_or("").into()))
    });

    bench_case(&mut cases, "get_cursor_position", || {
        let r = runner.call_sync(
            "desktop_automation",
            json!({ "action": "get_cursor_position" }),
        )?;
        Ok((!r.is_error, r.content.clone()))
    });

    if opts.include_heavy_capture {
        bench_case(&mut cases, "screen_capture_plain", || {
            let r = runner.call_sync(
                "screen_capture",
                json!({ "action": "capture", "format": "jpeg", "quality": 60 }),
            )?;
            let detail = if r.image.is_some() {
                format!("image attached, {} chars text", r.content.len())
            } else {
                r.content.lines().next().unwrap_or("no image").into()
            };
            Ok((!r.is_error && r.image.is_some(), detail))
        });

        bench_case(&mut cases, "screen_capture_grid", || {
            let r = runner.call_sync(
                "screen_capture",
                json!({ "action": "capture", "grid": true, "format": "png" }),
            )?;
            Ok((
                !r.is_error && r.image.is_some(),
                format!("grid capture, {} bytes text", r.content.len()),
            ))
        });
    }

    bench_case(&mut cases, "list_windows", || {
        let r = runner.call_sync(
            "desktop_automation",
            json!({ "action": "list_windows" }),
        )?;
        let has_panel = r.content.contains("RobotZ Test Panel");
        Ok((
            !r.is_error,
            if has_panel {
                "found RobotZ Test Panel window".into()
            } else {
                format!("{} windows listed", r.content.lines().count())
            },
        ))
    });

    for (i, &(x, y)) in opts.click_targets.iter().take(3).enumerate() {
        let name = format!("click_accuracy_target_{i}");
        bench_case(&mut cases, &name, || {
            runner.call_sync(
                "desktop_automation",
                json!({ "action": "click", "x": x, "y": y }),
            )?;
            std::thread::sleep(std::time::Duration::from_millis(80));
            let r = runner.call_sync(
                "desktop_automation",
                json!({ "action": "get_cursor_position" }),
            )?;
            let (cx, cy) = parse_cursor_xy(&r.content).unwrap_or((0, 0));
            let dx = (cx - x).abs();
            let dy = (cy - y).abs();
            let ok = dx <= 12 && dy <= 12;
            Ok((
                ok,
                format!("target ({x},{y}) cursor ({cx},{cy}) Δ=({dx},{dy})"),
            ))
        });
    }

    let passed = cases.iter().filter(|c| c.ok).count();
    let failed = cases.len() - passed;

    BenchmarkReport {
        generated_at: chrono::Utc::now().to_rfc3339(),
        platform: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        hostname: hostname(),
        transport: "direct".into(),
        cases,
        passed,
        failed,
    }
}

fn bench_case<F>(cases: &mut Vec<BenchCase>, name: &str, f: F)
where
    F: FnOnce() -> Result<(bool, String)>,
{
    let started = Instant::now();
    let (ok, detail, error) = match f() {
        Ok((ok, detail)) => (ok, detail, None),
        Err(e) => (false, String::new(), Some(e.to_string())),
    };
    cases.push(BenchCase {
        name: name.into(),
        ok,
        duration_ms: started.elapsed().as_millis() as u64,
        detail,
        error,
    });
}

pub fn write_report(path: &Path, report: &BenchmarkReport) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(report)?;
    std::fs::write(path, raw)?;
    Ok(())
}

pub fn host_data_dir() -> std::path::PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        return std::path::PathBuf::from(xdg).join("robotz-host");
    }
    if let Ok(home) = std::env::var("HOME") {
        return std::path::PathBuf::from(home)
            .join(".local/share/robotz-host");
    }
    std::path::PathBuf::from(".robotz-host-data")
}

fn hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

fn parse_cursor_xy(content: &str) -> Option<(i32, i32)> {
    let start = content.find('(')?;
    let rest = &content[start + 1..];
    let end = rest.find(')')?;
    let parts: Vec<_> = rest[..end].split(',').collect();
    if parts.len() != 2 {
        return None;
    }
    Some((parts[0].trim().parse().ok()?, parts[1].trim().parse().ok()?))
}
