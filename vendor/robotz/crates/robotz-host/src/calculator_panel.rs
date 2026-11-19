//! Simple on-screen calculator for agent click / arithmetic tests.

use egui::{Color32, RichText, Ui, Vec2};

use crate::coords::rect_screen_center;
use crate::panel::{CalcOp, CalculatorState, PanelApp};

const KEY_W: f32 = 52.0;
const KEY_H: f32 = 44.0;
const KEY_GAP: f32 = 4.0;

const KEY_ROWS: [[&str; 4]; 4] = [
    ["7", "8", "9", "/"],
    ["4", "5", "6", "*"],
    ["1", "2", "3", "-"],
    ["C", "0", "=", "+"],
];

impl PanelApp {
    pub(super) fn draw_calculator_sidebar(&mut self, ui: &mut Ui) {
        ui.heading("计算器");
        ui.label(
            RichText::new("Agent 点击数字与运算符完成算术。")
                .small()
                .weak(),
        );
        ui.separator();

        let display = self.state.calculator.display.clone();
        egui::Frame::none()
            .fill(Color32::from_rgb(20, 24, 32))
            .stroke(egui::Stroke::new(1.0, Color32::from_gray(70)))
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.set_width(KEY_W * 4.0 + KEY_GAP * 3.0);
                ui.label(
                    RichText::new(display)
                        .size(22.0)
                        .monospace()
                        .color(Color32::from_rgb(120, 220, 255)),
                );
            });

        ui.add_space(6.0);
        self.state.calculator.button_screen.clear();

        for row in KEY_ROWS {
            ui.horizontal(|ui| {
                for key in row {
                    let (rect, resp) = ui.allocate_exact_size(
                        Vec2::new(KEY_W, KEY_H),
                        egui::Sense::click(),
                    );
                    let screen = rect_screen_center(ui.ctx(), rect);
                    self.state.calculator.button_screen.insert(key.to_string(), screen);

                    let fill = if resp.hovered() {
                        Color32::from_rgb(70, 80, 110)
                    } else {
                        Color32::from_rgb(45, 52, 68)
                    };
                    ui.painter().rect(
                        rect,
                        6.0,
                        fill,
                        egui::Stroke::new(1.0, Color32::from_gray(90)),
                    );
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        key,
                        egui::FontId::proportional(18.0),
                        Color32::WHITE,
                    );

                    if resp.clicked() {
                        self.on_calc_key(key);
                    }
                    ui.add_space(KEY_GAP);
                }
            });
            ui.add_space(KEY_GAP);
        }

        if let Some(result) = self.state.calculator.last_result {
            ui.separator();
            ui.label(format!("上次结果: {result}"));
        }
    }

    pub(super) fn draw_calculator_central(&mut self, ui: &mut Ui) {
        ui.heading("计算器测试");
        ui.label(
            RichText::new("左侧为计算器。Agent 通过 desktop_automation / uia 点击按键完成运算。")
                .small()
                .weak(),
        );
        ui.separator();
        ui.label(RichText::new("示例任务").strong());
        ui.label("计算 23 × 4 + 7，期望结果 99。");
        ui.label("计算 100 − 37，期望结果 63。");
        ui.add_space(8.0);

        if !self.state.calculator.button_screen.is_empty() {
            ui.collapsing("按键屏幕坐标（物理像素）", |ui| {
                egui::Grid::new("calc_coords")
                    .num_columns(3)
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label("键");
                        ui.label("X");
                        ui.label("Y");
                        ui.end_row();
                        let mut keys: Vec<_> =
                            self.state.calculator.button_screen.keys().cloned().collect();
                        keys.sort();
                        for k in keys {
                            if let Some(&(x, y)) = self.state.calculator.button_screen.get(&k) {
                                ui.label(&k);
                                ui.monospace(format!("{x}"));
                                ui.monospace(format!("{y}"));
                                ui.end_row();
                            }
                        }
                    });
            });
        }

        ui.add_space(8.0);
        ui.collapsing("使用说明", |ui| {
            ui.label("1. 读取按键坐标表，或截图 + 网格定位。");
            ui.label("2. 依次 click 数字与运算符，最后按 =。");
            ui.label("3. 显示屏数值应与手算结果一致。");
            ui.label("4. C 键清除；支持连续运算。");
        });
    }

    fn on_calc_key(&mut self, key: &str) {
        match key {
            "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" => {
                self.push_digit(key);
            }
            "+" => self.push_op(CalcOp::Add),
            "-" => self.push_op(CalcOp::Sub),
            "*" => self.push_op(CalcOp::Mul),
            "/" => self.push_op(CalcOp::Div),
            "=" => self.evaluate(),
            "C" => self.state.calculator.reset(),
            _ => {}
        }
        self.state
            .push_log(format!("计算器按键: {key} → {}", self.state.calculator.display), false);
    }

    fn push_digit(&mut self, d: &str) {
        let calc = &mut self.state.calculator;
        if calc.fresh_entry || calc.display == "0" || calc.display == "Error" {
            calc.display = d.to_string();
            calc.fresh_entry = false;
        } else {
            calc.display.push_str(d);
        }
    }

    fn push_op(&mut self, op: CalcOp) {
        let should_eval = self.state.calculator.pending_op.is_some()
            && !self.state.calculator.fresh_entry;
        if should_eval {
            self.evaluate();
        }
        if let Ok(v) = self.state.calculator.display.parse::<f64>() {
            self.state.calculator.accumulator = Some(v);
        }
        self.state.calculator.pending_op = Some(op);
        self.state.calculator.fresh_entry = true;
    }

    fn evaluate(&mut self) {
        let calc = &mut self.state.calculator;
        let current = match calc.display.parse::<f64>() {
            Ok(v) => v,
            Err(_) => {
                calc.display = "Error".into();
                return;
            }
        };
        let result = if let (Some(acc), Some(op)) = (calc.accumulator, calc.pending_op) {
            match op.apply(acc, current) {
                Some(r) => r,
                None => {
                    calc.display = "Error".into();
                    calc.pending_op = None;
                    calc.accumulator = None;
                    return;
                }
            }
        } else {
            current
        };
        let text = format_calc_result(result);
        calc.display = text.clone();
        calc.last_result = Some(result);
        calc.accumulator = Some(result);
        calc.pending_op = None;
        calc.fresh_entry = true;
        self.state
            .push_log(format!("计算器结果: {text}"), false);
    }
}

impl CalculatorState {
    pub fn reset(&mut self) {
        self.display = "0".into();
        self.accumulator = None;
        self.pending_op = None;
        self.fresh_entry = true;
        self.last_result = None;
    }
}

impl CalcOp {
    fn apply(self, a: f64, b: f64) -> Option<f64> {
        match self {
            CalcOp::Add => Some(a + b),
            CalcOp::Sub => Some(a - b),
            CalcOp::Mul => Some(a * b),
            CalcOp::Div if b.abs() > f64::EPSILON => Some(a / b),
            CalcOp::Div => None,
        }
    }
}

fn format_calc_result(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.6}").trim_end_matches('0').trim_end_matches('.').to_string()
    }
}
