//! Dedicated GUI for screen / pointer calibration.

use egui::{Color32, RichText, Ui, Vec2};

use crate::bench::host_data_dir;
use crate::calibrate::{
    self, advance_after_sample, export_measurement_report, query_ui_status,
    run_calibration_sample, save_uia_calibration, uia_persist_supported, CalibrationUiStatus,
    ClickBackend, CALIBRATION_TARGET_INDICES,
};
use crate::panel::PanelApp;

impl PanelApp {
    pub(super) fn refresh_calibration_status(&mut self) {
        self.state.cal_ui_status = query_ui_status(&host_data_dir());
    }

    pub(super) fn resolve_grid_target(&self, grid_idx: usize) -> Option<(i32, i32)> {
        self.state
            .calibrated_targets
            .get(grid_idx)
            .and_then(|o| *o)
            .or_else(|| self.state.target_screen_coords.get(grid_idx).copied())
    }

    pub(super) fn draw_calibration_sidebar(&mut self, ui: &mut Ui) {
        ui.heading("屏幕校准");
        ui.label(
            RichText::new("五点采样：在面板高亮靶心上自动点击并测量光标偏差。")
                .small()
                .weak(),
        );
        ui.separator();

        if ui.button("刷新显示器 / 校准状态").clicked() {
            self.refresh_calibration_status();
            self.run_tool(
                "screen_capture",
                serde_json::json!({ "action": "list_monitors" }),
                "list_monitors",
            );
        }

        Self::draw_status_block(ui, &self.state.cal_ui_status);

        ui.separator();
        ui.label(RichText::new(&self.state.cal_wizard.message).small());

        if uia_persist_supported() {
            ui.horizontal(|ui| {
                ui.label("点击后端:");
                ui.selectable_value(
                    &mut self.state.cal_wizard.backend,
                    ClickBackend::Desktop,
                    "desktop",
                );
                ui.selectable_value(
                    &mut self.state.cal_wizard.backend,
                    ClickBackend::Uia,
                    "uia",
                );
            });
        }

        if ui
            .button(RichText::new("▶ 开始五点校准").strong())
            .clicked()
        {
            self.state.cal_wizard.reset();
            self.state.cal_wizard.step = 1;
            self.state.cal_wizard.backend = calibrate::default_click_backend();
            if let Some(g) = self.state.cal_wizard.current_grid_index() {
                self.state.cal_wizard.message =
                    format!("步骤 1/5：请先本地点击靶心 #{g} 记录坐标，再点「采样此点」");
            }
        }

        if self.state.cal_wizard.is_active() {
            if let Some(grid_idx) = self.state.cal_wizard.current_grid_index() {
                ui.label(format!("当前靶心: #{grid_idx}"));
                if ui.button("◎ 采样此点（自动点击）").clicked() {
                    if let Some(target) = self.resolve_grid_target(grid_idx) {
                        match run_calibration_sample(
                            &self.runner,
                            &mut self.state.cal_wizard,
                            grid_idx,
                            target,
                        ) {
                            Ok(()) => {
                                advance_after_sample(&mut self.state.cal_wizard);
                                self.state.push_log(
                                    format!("校准采样 #{grid_idx} OK"),
                                    false,
                                );
                            }
                            Err(e) => {
                                self.state.cal_wizard.message = e.to_string();
                                self.state.push_log(format!("采样失败: {e}"), true);
                            }
                        }
                    } else {
                        self.state.cal_wizard.message =
                            "请先在面板上用鼠标点击该靶心，以记录物理坐标。".into();
                    }
                }
            }
        }

        if !self.state.cal_wizard.samples.is_empty() {
            ui.separator();
            ui.heading("采样记录");
            egui::Grid::new("cal_samples")
                .num_columns(4)
                .striped(true)
                .show(ui, |ui| {
                    ui.label("靶心");
                    ui.label("目标");
                    ui.label("实际");
                    ui.label("偏差");
                    ui.end_row();
                    for s in &self.state.cal_wizard.samples {
                        ui.label(format!("#{}", s.grid_index));
                        ui.monospace(format!("{},{}", s.target[0], s.target[1]));
                        ui.monospace(format!("{},{}", s.actual[0], s.actual[1]));
                        ui.monospace(format!("{},{}", s.error_px[0], s.error_px[1]));
                        ui.end_row();
                    }
                });
            if let Some(rms) = calibrate::wizard_rms(&self.state.cal_wizard) {
                ui.label(format!("RMS 偏差: {rms:.2} px"));
            }
        }

        ui.separator();
        ui.horizontal_wrapped(|ui| {
            if ui.button("导出测量报告 JSON").clicked() {
                let path = host_data_dir().join("pointer-calibration-report.json");
                match export_measurement_report(&self.state.cal_wizard, &path) {
                    Ok(()) => {
                        self.state.cal_wizard.message =
                            format!("报告已写入 {}", path.display());
                        self.state.push_log(self.state.cal_wizard.message.clone(), false);
                    }
                    Err(e) => self.state.push_log(format!("导出失败: {e}"), true),
                }
            }
            if uia_persist_supported()
                && ui
                    .button("保存为 UIA 校准")
                    .on_hover_text("写入 uia_calibration.json，供 uia 工具使用")
                    .clicked()
            {
                match save_uia_calibration(&mut self.state.cal_wizard, &host_data_dir()) {
                    Ok(()) => {
                        self.refresh_calibration_status();
                        self.state.push_log("UIA 校准已保存", false);
                    }
                    Err(e) => self.state.push_log(format!("保存失败: {e}"), true),
                }
            }
            if ui.button("清除 UIA 校准").clicked() {
                let path =
                    robotz_automation::calibration::calibration_file_path(&host_data_dir());
                let _ = robotz_automation::calibration::delete_file(&path);
                robotz_automation::calibration::clear_cache();
                self.state.cal_wizard.reset();
                self.refresh_calibration_status();
                self.state.push_log("已清除 UIA 校准", false);
            }
        });

        if ui.button("带网格截图（辅助目视）").clicked() {
            self.run_tool(
                "screen_capture",
                serde_json::json!({ "action": "capture", "grid": true, "format": "png" }),
                "grid_capture",
            );
        }
    }

    fn draw_status_block(ui: &mut Ui, status: &CalibrationUiStatus) {
        ui.label(RichText::new(&status.summary).color(Color32::LIGHT_BLUE));
        for line in &status.monitors_lines {
            ui.label(RichText::new(line).small());
        }
        if !status.uia_file.is_empty() {
            ui.label(RichText::new(format!("校准文件: {}", status.uia_file)).small().weak());
        }
        if status.uia_valid {
            if let Some(rms) = status.uia_rms_px {
                ui.label(RichText::new(format!("已生效 UIA RMS: {rms:.2}px")).small());
            }
        }
    }

    /// Larger crosshair targets for the calibration tab.
    pub(super) fn calibration_target_style(
        &self,
        idx: usize,
    ) -> (Color32, Color32, f32, bool) {
        let is_anchor = CALIBRATION_TARGET_INDICES.contains(&idx);
        let is_current = self
            .state
            .cal_wizard
            .current_grid_index()
            .map(|g| g == idx)
            .unwrap_or(false);
        let sampled = self
            .state
            .cal_wizard
            .samples
            .iter()
            .any(|s| s.grid_index == idx);

        if is_current {
            (
                Color32::from_rgb(180, 90, 20),
                Color32::YELLOW,
                3.0,
                true,
            )
        } else if sampled {
            (
                Color32::from_rgb(30, 110, 60),
                Color32::from_rgb(80, 200, 120),
                2.0,
                is_anchor,
            )
        } else if is_anchor {
            (
                Color32::from_rgb(90, 70, 25),
                Color32::from_rgb(220, 170, 60),
                2.0,
                true,
            )
        } else {
            (
                Color32::from_rgb(40, 45, 55),
                Color32::from_gray(70),
                1.0,
                false,
            )
        }
    }

    pub(super) fn calibration_button_size(&self, is_anchor: bool) -> Vec2 {
        if is_anchor {
            Vec2::new(100.0, 64.0)
        } else {
            Vec2::new(72.0, 44.0)
        }
    }
}
