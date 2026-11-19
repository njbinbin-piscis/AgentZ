//! Visual test surface for mouse, keyboard, and screen-capture workflows.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;

use base64::Engine;
use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use serde_json::json;

use std::collections::HashMap;

use crate::bench::{host_data_dir, run_benchmark, write_report, BenchOptions};
use crate::calibrate::{CalibrationUiStatus, CalibrationWizard, CALIBRATION_TARGET_INDICES};
use crate::coords::{pointer_screen_physical, rect_screen_center};
use crate::mcp_client::{find_robotz_mcp_binary, mcp_result_summary, McpSession};
use crate::runner::ToolRunner;

pub const WINDOW_TITLE: &str = "RobotZ 屏幕校准与测试";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppTab {
    #[default]
    Calibration,
    TestRange,
    UiaDrag,
    Calculator,
    Advanced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiaDropResult {
    #[default]
    Idle,
    Success,
    Fail,
}

#[derive(Debug, Clone)]
pub struct UiaDragState {
    pub ball_pos: (f32, f32),
    pub ball_screen: Option<(i32, i32)>,
    pub target_screen: Option<(i32, i32)>,
    pub drop_result: UiaDropResult,
    pub dragging: bool,
    pub drag_offset: Vec2,
}

impl Default for UiaDragState {
    fn default() -> Self {
        Self {
            ball_pos: (60.0, 120.0),
            ball_screen: None,
            target_screen: None,
            drop_result: UiaDropResult::Idle,
            dragging: false,
            drag_offset: Vec2::ZERO,
        }
    }
}

impl UiaDragState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalcOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, Default)]
pub struct CalculatorState {
    pub display: String,
    pub accumulator: Option<f64>,
    pub pending_op: Option<CalcOp>,
    pub fresh_entry: bool,
    pub last_result: Option<f64>,
    pub button_screen: HashMap<String, (i32, i32)>,
}

impl CalculatorState {
    pub fn new() -> Self {
        Self {
            display: "0".into(),
            ..Default::default()
        }
    }
}

const GRID_COLS: usize = 5;
const GRID_ROWS: usize = 4;
const TARGET_COUNT: usize = GRID_COLS * GRID_ROWS;

#[derive(Clone, Debug)]
pub struct LogLine {
    pub at: Instant,
    pub text: String,
    pub is_error: bool,
}

#[derive(Default)]
pub struct PanelState {
    /// Index of the last target the pointer hovered (from tool poll).
    pub hover_target: Option<usize>,
    /// Index of targets clicked (panel-local egui clicks).
    pub clicked_targets: Vec<usize>,
    /// Last desktop_automation cursor read (physical pixels).
    pub cursor_physical: Option<(i32, i32)>,
    /// Screen coords shown on each target button (estimated or calibrated).
    pub target_screen_coords: Vec<(i32, i32)>,
    /// Per-target coords learned from `get_cursor_position` after a local click.
    pub calibrated_targets: Vec<Option<(i32, i32)>>,
    /// Keyboard test field content (also target for type_text).
    pub keyboard_text: String,
    pub hotkey_log: String,
    pub logs: VecDeque<LogLine>,
    pub last_capture_png: Option<Vec<u8>>,
    pub last_tool_message: String,
    pub drag_start: Option<Pos2>,
    pub drag_end: Option<Pos2>,
    pub running_action: bool,
    /// When true, tool calls go through the MCP subprocess instead of direct `Tool`.
    pub use_mcp: bool,
    pub mcp_status: String,
    pub cal_wizard: CalibrationWizard,
    pub cal_ui_status: CalibrationUiStatus,
    pub active_tab: AppTab,
    pub last_bench_report: Option<String>,
    pub uia_drag: UiaDragState,
    pub calculator: CalculatorState,
}

impl PanelState {
    pub fn push_log(&mut self, text: impl Into<String>, is_error: bool) {
        self.logs.push_front(LogLine {
            at: Instant::now(),
            text: text.into(),
            is_error,
        });
        if self.logs.len() > 80 {
            self.logs.pop_back();
        }
    }
}

pub struct PanelApp {
    pub runner: Arc<ToolRunner>,
    pub mcp: Arc<McpSession>,
    pub state: PanelState,
}

impl PanelApp {
    pub fn new(runner: Arc<ToolRunner>) -> Self {
        let mut state = PanelState::default();
        state.mcp_status = format!(
            "MCP binary: {}",
            find_robotz_mcp_binary().display()
        );
        state.active_tab = AppTab::Calibration;
        state.cal_wizard.reset();
        state.uia_drag.reset();
        state.calculator = CalculatorState::new();
        let mut app = Self {
            runner,
            mcp: Arc::new(McpSession::new(find_robotz_mcp_binary())),
            state,
        };
        app.refresh_calibration_status();
        app
    }

    pub(super) fn run_tool(&mut self, name: &str, input: serde_json::Value, label: &str) {
        if self.state.running_action {
            return;
        }
        self.state.running_action = true;
        let via = if self.state.use_mcp { "MCP" } else { "direct" };
        let outcome = if self.state.use_mcp {
            self.mcp
                .call_tool_sync(name, input)
                .map(|r| {
                    let (text, png, is_error) = mcp_result_summary(&r);
                    (text, png, is_error)
                })
                .map_err(|e| e.to_string())
        } else {
            self.runner
                .call_sync(name, input)
                .map(|r| {
                    let png = r.image.as_ref().and_then(|img| {
                        base64::engine::general_purpose::STANDARD
                            .decode(&img.base64)
                            .ok()
                    });
                    (r.content, png, r.is_error)
                })
                .map_err(|e| e.to_string())
        };

        match outcome {
            Ok((text, png, is_error)) => {
                self.state.last_tool_message = text.clone();
                if let Some(bytes) = png {
                    self.state.last_capture_png = Some(bytes);
                }
                self.state.push_log(
                    format!("[{via}] {label}: {}", text.lines().next().unwrap_or("ok")),
                    is_error,
                );
            }
            Err(e) => {
                self.state.last_tool_message = e.clone();
                self.state.push_log(format!("[{via}] {label} failed: {e}"), true);
            }
        }
        self.state.running_action = false;
    }

    pub fn poll_cursor(&mut self) {
        if self.state.running_action {
            return;
        }
        if let Ok(r) = self
            .runner
            .call_sync("desktop_automation", json!({ "action": "get_cursor_position" }))
        {
            if !r.is_error {
                if let Some((x, y)) = parse_cursor(&r.content) {
                    self.state.cursor_physical = Some((x, y));
                    self.state.hover_target = nearest_target(&self.state.target_screen_coords, x, y);
                }
            }
        }
    }
}

impl eframe::App for PanelApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(egui::Visuals::dark());
        self.poll_cursor();

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("RobotZ");
                ui.separator();
                ui.selectable_value(
                    &mut self.state.active_tab,
                    AppTab::Calibration,
                    "屏幕校准",
                );
                ui.selectable_value(
                    &mut self.state.active_tab,
                    AppTab::TestRange,
                    "操作测试",
                );
                ui.selectable_value(
                    &mut self.state.active_tab,
                    AppTab::UiaDrag,
                    "UIA 拖拽",
                );
                ui.selectable_value(
                    &mut self.state.active_tab,
                    AppTab::Calculator,
                    "计算器",
                );
                ui.selectable_value(
                    &mut self.state.active_tab,
                    AppTab::Advanced,
                    "高级",
                );
                ui.separator();
                if ui
                    .button("📷 Capture + grid")
                    .on_hover_text("screen_capture with coordinate grid overlay")
                    .clicked()
                {
                    self.run_tool(
                        "screen_capture",
                        json!({ "action": "capture", "grid": true, "format": "png" }),
                        "screen_capture",
                    );
                }
                if ui.button("🖥 List monitors").clicked() {
                    self.run_tool(
                        "screen_capture",
                        json!({ "action": "list_monitors" }),
                        "list_monitors",
                    );
                }
                if ui.button("📍 Poll cursor (tool)").clicked() {
                    self.poll_cursor();
                    if let Some((x, y)) = self.state.cursor_physical {
                        self.state
                            .push_log(format!("cursor at physical ({x}, {y})"), false);
                    }
                }
                if ui.button("🪟 List windows").clicked() {
                    self.run_tool(
                        "desktop_automation",
                        json!({ "action": "list_windows" }),
                        "list_windows",
                    );
                }
                ui.separator();
                let hint = match self.state.active_tab {
                    AppTab::Calibration => "五点屏幕校准：侧栏引导采样，面板高亮靶心。",
                    AppTab::UiaDrag => "拖动小球到目标区，或读取坐标后一次 drag 调用。",
                    AppTab::Calculator => "左侧计算器：Agent 点击数字与运算符完成算术。",
                    _ => "MCP / desktop_automation 可自动化本窗口。",
                };
                ui.label(RichText::new(hint).small().weak());
            });
        });

        if self.state.active_tab == AppTab::Calibration {
            egui::SidePanel::left("calibration")
                .resizable(true)
                .default_width(300.0)
                .show(ctx, |ui| self.draw_calibration_sidebar(ui));
        }

        if self.state.active_tab == AppTab::Calculator {
            egui::SidePanel::left("calculator")
                .resizable(true)
                .default_width(260.0)
                .show(ctx, |ui| self.draw_calculator_sidebar(ui));
        }

        egui::SidePanel::right("sidebar")
            .resizable(true)
            .default_width(280.0)
            .show(ctx, |ui| {
                ui.heading("工具输出");
                ui.separator();

                ui.label("最近结果");
                ui.add(
                    egui::TextEdit::multiline(&mut self.state.last_tool_message)
                        .desired_width(f32::INFINITY)
                        .desired_rows(4),
                );

                if let Some((x, y)) = self.state.cursor_physical {
                    ui.label(format!("光标物理坐标: ({x}, {y})"));
                    if let Some(idx) = self.state.hover_target {
                        ui.label(format!("最近靶心: #{idx}"));
                    }
                }

                if self.state.active_tab == AppTab::UiaDrag {
                    ui.separator();
                    ui.heading("拖拽坐标");
                    if let Some((x, y)) = self.state.uia_drag.ball_screen {
                        ui.label(format!("小球中心: ({x}, {y})"));
                    }
                    if let Some((x, y)) = self.state.uia_drag.target_screen {
                        ui.label(format!("目标中心: ({x}, {y})"));
                    }
                }

                if self.state.active_tab == AppTab::Calculator {
                    ui.separator();
                    ui.heading("计算器");
                    ui.label(format!("显示: {}", self.state.calculator.display));
                    if let Some(r) = self.state.calculator.last_result {
                        ui.label(format!("结果: {r}"));
                    }
                }

                if self.state.active_tab == AppTab::TestRange {
                ui.separator();
                ui.heading("快捷操作");
                ui.horizontal_wrapped(|ui| {
                    for idx in 0..TARGET_COUNT.min(8) {
                        if ui.button(format!("#{idx} click")).clicked() {
                            if let Some(&(x, y)) = self.state.target_screen_coords.get(idx) {
                                self.run_tool(
                                    "desktop_automation",
                                    json!({ "action": "click", "x": x, "y": y }),
                                    &format!("click #{idx}"),
                                );
                            }
                        }
                    }
                });
                if ui.button("Type sample text into field").clicked() {
                    self.run_tool(
                        "desktop_automation",
                        json!({ "action": "type_text", "text": "RobotZ keyboard test 123!" }),
                        "type_text",
                    );
                }
                if ui.button("Hotkey Ctrl+A").clicked() {
                    self.run_tool(
                        "desktop_automation",
                        json!({ "action": "hotkey", "keys": ["ctrl", "a"] }),
                        "hotkey",
                    );
                    self.state.hotkey_log = "Sent ctrl+a via tool".into();
                }
                }

                if self.state.active_tab == AppTab::Advanced {
                ui.separator();
                ui.heading("MCP transport");
                ui.label(RichText::new(&self.state.mcp_status).small());
                ui.checkbox(&mut self.state.use_mcp, "Route tools via MCP subprocess");
                ui.horizontal(|ui| {
                    if ui.button("Connect MCP").clicked() {
                        match self.mcp.connect_sync(false) {
                            Ok(()) => {
                                self.state.use_mcp = true;
                                self.state.mcp_status =
                                    format!("Connected: {}", self.mcp.mcp_path().display());
                                self.state.push_log("MCP connected", false);
                            }
                            Err(e) => {
                                self.state.mcp_status = format!("Connect failed: {e}");
                                self.state.push_log(format!("MCP connect: {e}"), true);
                            }
                        }
                    }
                    if ui.button("Disconnect").clicked() {
                        self.mcp.disconnect_sync();
                        self.state.use_mcp = false;
                        self.state.mcp_status = "Disconnected".into();
                    }
                });
                if ui.button("MCP list_tools").clicked() {
                    if let Ok(t) = self.mcp.list_tools_sync() {
                        let names: Vec<_> = t.tools.iter().map(|x| x.name.as_ref()).collect();
                        self.state.last_tool_message = names.join(", ");
                        self.state.push_log(format!("MCP tools: {}", names.join(", ")), false);
                    }
                }

                ui.separator();
                ui.heading("Benchmark");
                if ui.button("Run benchmark → JSON").clicked() {
                    let targets: Vec<_> = CALIBRATION_TARGET_INDICES
                        .iter()
                        .filter_map(|&i| self.state.calibrated_targets.get(i).copied().flatten())
                        .collect();
                    let report = run_benchmark(
                        &self.runner,
                        BenchOptions {
                            click_targets: targets,
                            ..Default::default()
                        },
                    );
                    let path = host_data_dir().join("bench-latest.json");
                    match write_report(&path, &report) {
                        Ok(()) => {
                            let msg = format!(
                                "Benchmark {}/{} passed → {}",
                                report.passed,
                                report.cases.len(),
                                path.display()
                            );
                            self.state.last_bench_report = Some(msg.clone());
                            self.state.push_log(msg, false);
                        }
                        Err(e) => self.state.push_log(format!("bench write: {e}"), true),
                    }
                }
                if let Some(ref msg) = self.state.last_bench_report {
                    ui.label(RichText::new(msg).small().weak());
                }
                }

                ui.separator();
                if let Some(ref png) = self.state.last_capture_png {
                    ui.label("Last capture preview");
                    if let Ok(img) = image::load_from_memory(png) {
                        let tex = ctx.load_texture(
                            "last_capture",
                            egui::ColorImage::from_rgba_unmultiplied(
                                [img.width() as usize, img.height() as usize],
                                &img.to_rgba8(),
                            ),
                            egui::TextureOptions::LINEAR,
                        );
                        let max_w = ui.available_width();
                        let scale = (max_w / img.width() as f32).min(1.0);
                        let size = Vec2::new(img.width() as f32 * scale, img.height() as f32 * scale);
                        ui.image((tex.id(), size));
                    }
                }

                ui.separator();
                ui.heading("Log");
                egui::ScrollArea::vertical()
                    .max_height(200.0)
                    .show(ui, |ui| {
                        for line in &self.state.logs {
                            let color = if line.is_error {
                                Color32::LIGHT_RED
                            } else {
                                Color32::LIGHT_GRAY
                            };
                            ui.label(RichText::new(&line.text).color(color).small());
                        }
                    });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.state.active_tab == AppTab::UiaDrag {
                self.draw_uia_drag_central(ui);
                return;
            }
            if self.state.active_tab == AppTab::Calculator {
                self.draw_calculator_central(ui);
                return;
            }

            match self.state.active_tab {
                AppTab::Calibration => {
                    ui.heading("五点校准靶场");
                    ui.label(
                        RichText::new(
                            "① 侧栏点「开始五点校准」 ② 在本面板点击高亮靶心记录坐标 ③ 侧栏点「采样此点」",
                        )
                        .small()
                        .weak(),
                    );
                }
                AppTab::TestRange => {
                    ui.heading("鼠标 / 键盘测试靶场");
                    ui.label(
                        RichText::new(
                            "本地点击或经 desktop_automation / MCP 自动化。绿色=已点击；黄圈=光标靠近。",
                        )
                        .small()
                        .weak(),
                    );
                }
                AppTab::Advanced => {
                    ui.heading("靶场预览");
                    ui.label(
                        RichText::new("配合右侧 MCP 与基准测试使用。")
                            .small()
                            .weak(),
                    );
                }
                AppTab::UiaDrag | AppTab::Calculator => {}
            }

            let spacing = 8.0;
            let cal_mode = self.state.active_tab == AppTab::Calibration;
            self.state.target_screen_coords.clear();

            for row in 0..GRID_ROWS {
                ui.horizontal(|ui| {
                    for col in 0..GRID_COLS {
                        let idx = row * GRID_COLS + col;
                        let is_cal_anchor = CALIBRATION_TARGET_INDICES.contains(&idx);
                        if cal_mode && !is_cal_anchor {
                            self.state.target_screen_coords.push((0, 0));
                            ui.add_space(72.0 + spacing);
                            continue;
                        }

                        let button_size = if cal_mode {
                            self.calibration_button_size(is_cal_anchor)
                        } else {
                            Vec2::new(88.0, 52.0)
                        };
                        let label = format!("#{idx}");
                        let (rect, response) = ui.allocate_exact_size(button_size, Sense::click());
                        let estimated = pointer_screen_physical(
                            ctx,
                            rect,
                            response.hover_pos(),
                        );
                        let screen = self
                            .state
                            .calibrated_targets
                            .get(idx)
                            .and_then(|o| *o)
                            .unwrap_or(estimated);
                        self.state.target_screen_coords.push(screen);

                        let clicked = self.state.clicked_targets.contains(&idx);
                        let near = self.state.hover_target == Some(idx);

                        let (fill, stroke_color, stroke_w, _anchor) = if cal_mode {
                            self.calibration_target_style(idx)
                        } else {
                            let fill = if clicked {
                                Color32::from_rgb(40, 120, 60)
                            } else if is_cal_anchor {
                                Color32::from_rgb(80, 60, 30)
                            } else {
                                Color32::from_rgb(45, 55, 75)
                            };
                            let stroke_color = if near {
                                Color32::YELLOW
                            } else {
                                Color32::from_gray(100)
                            };
                            let stroke_w = if near { 3.0 } else { 1.0 };
                            (fill, stroke_color, stroke_w, is_cal_anchor)
                        };
                        let stroke = Stroke::new(stroke_w, stroke_color);

                        ui.painter().rect(rect, 6.0, fill, stroke);
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            &label,
                            egui::FontId::proportional(if cal_mode && is_cal_anchor { 18.0 } else { 16.0 }),
                            Color32::WHITE,
                        );
                        ui.painter().text(
                            rect.center() + Vec2::new(0.0, 14.0),
                            egui::Align2::CENTER_CENTER,
                            format!("{screen:?}"),
                            egui::FontId::monospace(9.0),
                            Color32::from_gray(180),
                        );

                        if response.clicked() {
                            if !self.state.clicked_targets.contains(&idx) {
                                self.state.clicked_targets.push(idx);
                            }
                            self.poll_cursor();
                            if let Some(c) = self.state.cursor_physical {
                                if self.state.calibrated_targets.len() <= idx {
                                    self.state.calibrated_targets.resize(idx + 1, None);
                                }
                                self.state.calibrated_targets[idx] = Some(c);
                                self.state.push_log(
                                    format!("面板点击 #{idx} — 光标 {c:?}"),
                                    false,
                                );
                            } else {
                                self.state.push_log(
                                    format!("面板点击 #{idx} 坐标 {screen:?}"),
                                    false,
                                );
                            }
                        }
                        ui.add_space(spacing);
                    }
                });
                ui.add_space(spacing);
            }

            if self.state.active_tab != AppTab::TestRange {
                return;
            }

            ui.separator();
            ui.heading("拖拽测试区");
            let (drag_rect, drag_resp) = ui.allocate_exact_size(Vec2::new(400.0, 80.0), Sense::drag());
            let drag_color = Color32::from_rgb(70, 50, 90);
            ui.painter().rect(
                drag_rect,
                8.0,
                drag_color,
                Stroke::new(1.5, Color32::from_rgb(140, 100, 180)),
            );
            ui.painter().text(
                drag_rect.center(),
                egui::Align2::CENTER_CENTER,
                "Drag here (panel or desktop_automation drag)",
                egui::FontId::proportional(14.0),
                Color32::WHITE,
            );
            if drag_resp.drag_started() {
                self.state.drag_start = drag_resp.interact_pointer_pos();
            }
            if drag_resp.drag_stopped() {
                self.state.drag_end = drag_resp.interact_pointer_pos();
                if let (Some(a), Some(b)) = (self.state.drag_start, self.state.drag_end) {
                    let s1 = rect_screen_center(ctx, Rect::from_min_size(a, Vec2::ZERO));
                    let s2 = rect_screen_center(ctx, Rect::from_min_size(b, Vec2::ZERO));
                    self.state.push_log(
                        format!("Panel drag {s1:?} → {s2:?} (use tool drag with these coords)"),
                        false,
                    );
                    if ui.input(|i| i.pointer.secondary_clicked()) {
                        let _ = (s1, s2);
                    }
                }
            }
            if ui.button("Run tool drag across zone").clicked() {
                let s1 = rect_screen_center(ctx, drag_rect);
                let s2 = (
                    s1.0 + drag_rect.width() as i32 - 20,
                    s1.1 + drag_rect.height() as i32 / 2,
                );
                self.run_tool(
                    "desktop_automation",
                    json!({
                        "action": "drag",
                        "x": s1.0,
                        "y": s1.1,
                        "to_x": s2.0,
                        "to_y": s2.1
                    }),
                    "drag",
                );
            }

            ui.separator();
            ui.heading("键盘测试");
            ui.label("聚焦下方输入框，使用侧栏或 MCP 的 type_text / hotkey。");
            ui.add(
                egui::TextEdit::singleline(&mut self.state.keyboard_text)
                    .desired_width(f32::INFINITY)
                    .hint_text("Type here manually or via desktop_automation.type_text…"),
            );
            ui.label(format!("Hotkey log: {}", self.state.hotkey_log));

            let keys: Vec<String> = ctx.input(|i| {
                i.events
                    .iter()
                    .filter_map(|e| {
                        if let egui::Event::Key { key, pressed: true, .. } = e {
                            Some(format!("{key:?}"))
                        } else {
                            None
                        }
                    })
                    .collect()
            });
            if !keys.is_empty() {
                self.state.hotkey_log = keys.join(", ");
            }
        });

        if self.state.running_action {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
    }
}

fn parse_cursor(content: &str) -> Option<(i32, i32)> {
    // "Cursor position: (123, 456)" or similar
    let start = content.find('(')?;
    let rest = &content[start + 1..];
    let end = rest.find(')')?;
    let parts: Vec<_> = rest[..end].split(',').collect();
    if parts.len() != 2 {
        return None;
    }
    let x = parts[0].trim().parse().ok()?;
    let y = parts[1].trim().parse().ok()?;
    Some((x, y))
}

fn nearest_target(coords: &[(i32, i32)], x: i32, y: i32) -> Option<usize> {
    const THRESHOLD: i32 = 48;
    coords
        .iter()
        .enumerate()
        .min_by_key(|(_, (tx, ty))| (tx - x).abs() + (ty - y).abs())
        .filter(|(_, (tx, ty))| (tx - x).abs() < THRESHOLD && (ty - y).abs() < THRESHOLD)
        .map(|(i, _)| i)
}
