//! Screen / pointer calibration wizard for the RobotZ GUI host.
//!
//! - **All platforms**: five-point sampling via `desktop_automation` to measure
//!   click drift; results shown in the panel and exported as JSON.
//! - **Windows**: optional persist to `uia_calibration.json` so `uia.click`
//!   applies the fitted transform automatically.

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[cfg(target_os = "windows")]
use robotz_automation::calibration::{
    self, build_fingerprint, calibration_file_path, current_snapshot, fit_monitor_calibration,
    load_file, refresh_cache, save_file, set_cached, CalibrationFile, MonitorCalibration,
    MonitorSnapshot,
};
use crate::runner::ToolRunner;

/// Indices on the 5×4 panel grid used as calibration anchors (corners + center).
pub const CALIBRATION_TARGET_INDICES: [usize; 5] = [0, 4, 9, 14, 19];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClickBackend {
    #[default]
    /// Cross-platform `desktop_automation.click`.
    Desktop = 0,
    /// Windows `uia.click` (feeds the UIA calibration store).
    Uia,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CalibrationSample {
    pub index: usize,
    pub grid_index: usize,
    pub target: [i32; 2],
    pub actual: [i32; 2],
    pub error_px: [i32; 2],
}

#[derive(Debug, Clone, Default)]
pub struct CalibrationWizard {
    pub step: usize,
    pub backend: ClickBackend,
    pub targets: Vec<(i32, i32)>,
    pub actuals: Vec<(i32, i32)>,
    pub samples: Vec<CalibrationSample>,
    pub monitor_index: usize,
    pub message: String,
    pub finished_rms: Option<f64>,
}

impl CalibrationWizard {
    pub fn reset(&mut self) {
        *self = Self::default();
        self.backend = ClickBackend::Desktop;
        self.message = "点击「开始五点校准」，然后在每个高亮靶心上执行采样。".into();
    }

    pub fn is_active(&self) -> bool {
        self.step > 0 && self.step <= CALIBRATION_TARGET_INDICES.len()
    }

    pub fn is_done(&self) -> bool {
        self.step > CALIBRATION_TARGET_INDICES.len()
    }

    pub fn current_grid_index(&self) -> Option<usize> {
        if !self.is_active() {
            return None;
        }
        CALIBRATION_TARGET_INDICES.get(self.step - 1).copied()
    }
}

/// Shown in the calibration sidebar.
#[derive(Debug, Clone, Default)]
pub struct CalibrationUiStatus {
    pub summary: String,
    pub monitors_lines: Vec<String>,
    pub uia_valid: bool,
    pub uia_rms_px: Option<f64>,
    pub uia_file: String,
}

pub fn default_click_backend() -> ClickBackend {
    ClickBackend::Desktop
}

pub fn uia_persist_supported() -> bool {
    cfg!(target_os = "windows")
}

pub fn sample_error(target: (i32, i32), actual: (i32, i32)) -> (i32, i32) {
    (actual.0 - target.0, actual.1 - target.1)
}

pub fn wizard_rms(wizard: &CalibrationWizard) -> Option<f64> {
    if wizard.targets.is_empty() {
        return None;
    }
    let n = wizard.targets.len() as f64;
    let sq: f64 = wizard
        .targets
        .iter()
        .zip(wizard.actuals.iter())
        .map(|(t, a)| {
            let dx = (a.0 - t.0) as f64;
            let dy = (a.1 - t.1) as f64;
            dx * dx + dy * dy
        })
        .sum();
    Some((sq / n).sqrt())
}

/// Refresh monitor / UIA calibration status for the GUI (Windows fills real data).
pub fn query_ui_status(app_data: &Path) -> CalibrationUiStatus {
    #[cfg(target_os = "windows")]
    {
        calibration::refresh_cache(app_data);
        let snap = calibration::current_snapshot();
        let path = calibration_file_path(app_data);
        let mut status = CalibrationUiStatus {
            uia_file: path.display().to_string(),
            ..Default::default()
        };
        for m in &snap.monitors {
            status.monitors_lines.push(format!(
                "显示器 {}: {}x{} @ ({},{}) DPI {}% {}",
                m.index,
                m.rect[2] - m.rect[0],
                m.rect[3] - m.rect[1],
                m.rect[0],
                m.rect[1],
                m.scale_percent,
                if m.primary { "[主屏]" } else { "" }
            ));
        }
        if let Some(file) = load_file(&path) {
            if file.fingerprint == snap.fingerprint {
                status.uia_valid = true;
                status.uia_rms_px = file.monitors.first().map(|m| m.residual_rms_px);
                status.summary = format!(
                    "UIA 校准有效，RMS {:.2}px",
                    status.uia_rms_px.unwrap_or(0.0)
                );
                return status;
            }
            status.summary = "UIA 校准文件已过期（分辨率/DPI/显示器变化）".into();
            return status;
        }
        status.summary = "尚未保存 UIA 校准（Windows 可在采样后写入）".into();
        status
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app_data;
        CalibrationUiStatus {
            summary: "使用五点采样测量点击偏差；结果可导出 JSON 报告。".into(),
            ..Default::default()
        }
    }
}

/// Sample one calibration point using the chosen click backend.
pub fn run_calibration_sample(
    runner: &ToolRunner,
    wizard: &mut CalibrationWizard,
    grid_index: usize,
    target: (i32, i32),
) -> Result<()> {
    let (x, y) = target;
    match wizard.backend {
        ClickBackend::Desktop => {
            let r = runner.call_sync(
                "desktop_automation",
                json!({ "action": "click", "x": x, "y": y }),
            )?;
            if r.is_error {
                anyhow::bail!("desktop_automation.click 失败: {}", r.content);
            }
        }
        ClickBackend::Uia => {
            if !uia_persist_supported() {
                anyhow::bail!("UIA 点击仅支持 Windows");
            }
            let r = runner.call_sync(
                "uia",
                json!({
                    "action": "click",
                    "x": x,
                    "y": y,
                    "_skip_calibration": true
                }),
            )?;
            if r.is_error {
                anyhow::bail!("uia.click 失败: {}", r.content);
            }
        }
    }

    std::thread::sleep(std::time::Duration::from_millis(150));
    let actual = read_cursor(runner).context("读取光标位置")?;
    let err = sample_error(target, actual);

    wizard.targets.push(target);
    wizard.actuals.push(actual);
    wizard.samples.push(CalibrationSample {
        index: wizard.samples.len(),
        grid_index,
        target: [target.0, target.1],
        actual: [actual.0, actual.1],
        error_px: [err.0, err.1],
    });

    let rms = wizard_rms(wizard).unwrap_or(0.0);
    wizard.message = format!(
        "采样 {}/{}：靶心 #{grid_index} 目标({x},{y}) → 实际({},{}) 偏差({},{}) RMS≈{rms:.1}px",
        wizard.samples.len(),
        CALIBRATION_TARGET_INDICES.len(),
        actual.0,
        actual.1,
        err.0,
        err.1
    );
    Ok(())
}

pub fn advance_after_sample(wizard: &mut CalibrationWizard) {
    wizard.step += 1;
    if wizard.step > CALIBRATION_TARGET_INDICES.len() {
        wizard.finished_rms = wizard_rms(wizard);
        wizard.message = format!(
            "五点采样完成，RMS {:.2}px。可导出报告{}",
            wizard.finished_rms.unwrap_or(0.0),
            if uia_persist_supported() {
                "或保存为 UIA 校准"
            } else {
                ""
            }
        );
    } else if let Some(next) = wizard.current_grid_index() {
        wizard.message = format!(
            "步骤 {}/{}：请在面板高亮靶心 #{next} 上点击「采样此点」",
            wizard.step,
            CALIBRATION_TARGET_INDICES.len()
        );
    }
}

fn read_cursor(runner: &ToolRunner) -> Result<(i32, i32)> {
    #[cfg(target_os = "windows")]
    {
        if let Some(p) = calibration::windows_helpers::cursor_position() {
            return Ok(p);
        }
    }
    let r = runner.call_sync(
        "desktop_automation",
        json!({ "action": "get_cursor_position" }),
    )?;
    parse_cursor(&r.content).ok_or_else(|| anyhow::anyhow!("无法解析光标: {}", r.content))
}

fn parse_cursor(content: &str) -> Option<(i32, i32)> {
    let start = content.find('(')?;
    let rest = &content[start + 1..];
    let end = rest.find(')')?;
    let parts: Vec<_> = rest[..end].split(',').collect();
    if parts.len() != 2 {
        return None;
    }
    Some((parts[0].trim().parse().ok()?, parts[1].trim().parse().ok()?))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointerMeasurementReport {
    pub generated_at: String,
    pub platform: String,
    pub backend: String,
    pub residual_rms_px: f64,
    pub samples: Vec<CalibrationSample>,
}

pub fn export_measurement_report(wizard: &CalibrationWizard, path: &Path) -> Result<()> {
    if wizard.samples.is_empty() {
        anyhow::bail!("尚无采样数据");
    }
    let report = PointerMeasurementReport {
        generated_at: chrono::Utc::now().to_rfc3339(),
        platform: std::env::consts::OS.into(),
        backend: match wizard.backend {
            ClickBackend::Desktop => "desktop_automation",
            ClickBackend::Uia => "uia",
        }
        .into(),
        residual_rms_px: wizard_rms(wizard).unwrap_or(0.0),
        samples: wizard.samples.clone(),
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(&report)?)?;
    Ok(())
}

/// Persist UIA calibration on Windows (uses the same five samples).
#[cfg(target_os = "windows")]
pub fn save_uia_calibration(wizard: &mut CalibrationWizard, app_data: &Path) -> Result<()> {
    use calibration::windows_helpers;

    if wizard.targets.len() < 3 {
        anyhow::bail!("至少需要 3 个采样点");
    }

    let virtual_screen = windows_helpers::virtual_screen_rect();
    let monitors = windows_helpers::enumerate_monitors_with_dpi();
    let monitor = monitors
        .iter()
        .find(|m| m.index == wizard.monitor_index)
        .or(monitors.first())
        .context("未找到显示器")?;

    let entry: MonitorCalibration = fit_monitor_calibration(
        monitor.index,
        monitor.rect,
        &wizard.targets,
        &wizard.actuals,
    );
    wizard.finished_rms = Some(entry.residual_rms_px);

    let file = CalibrationFile {
        fingerprint: build_fingerprint(&virtual_screen, &monitors),
        monitors: vec![entry],
        version: 1,
    };

    let path = calibration_file_path(app_data);
    save_file(&path, &file)?;
    calibration::refresh_cache(app_data);
    set_cached(file);

    wizard.message = format!(
        "UIA 校准已保存到 {}（RMS {:.2}px）",
        path.display(),
        wizard.finished_rms.unwrap_or(0.0)
    );
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn save_uia_calibration(_wizard: &mut CalibrationWizard, _app_data: &Path) -> Result<()> {
    anyhow::bail!("UIA 校准持久化仅支持 Windows")
}
