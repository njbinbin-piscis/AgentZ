//! UIA precision drag test — drag the orange ball into the green target zone.
//!
//! Ported from openpiscis Debug → UIA Test (v0.7.9). Shows exact physical screen
//! coordinates so an agent can call `desktop_automation` / `uia.drag_drop` once.

use egui::{Color32, Id, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};
use serde_json::json;

use crate::calibrate::uia_persist_supported;
use crate::coords::rect_screen_center;
use crate::panel::{PanelApp, UiaDropResult};

const ARENA_W: f32 = 800.0;
const ARENA_H: f32 = 260.0;
const BALL_SIZE: f32 = 40.0;
const BALL_RADIUS: f32 = BALL_SIZE / 2.0;
const TARGET_W: f32 = 120.0;
const TARGET_H: f32 = 120.0;
const TARGET_MARGIN_RIGHT: f32 = 60.0;
const GRAB_RADIUS: f32 = 60.0;

impl PanelApp {
    pub(super) fn draw_uia_drag_central(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        ui.heading("UIA 精度拖拽测试");
        ui.label(
            RichText::new(
                "将橙色小球拖入绿色目标区。小球与目标均标注物理屏幕坐标，Agent 可一次 drag 调用完成。",
            )
            .small()
            .weak(),
        );

        ui.horizontal(|ui| {
            if ui.button("重置小球").clicked() {
                self.state.uia_drag.reset();
            }
            if ui
                .button("工具拖拽 (desktop_automation)")
                .on_hover_text("desktop_automation drag：小球中心 → 目标中心")
                .clicked()
            {
                self.run_uia_tool_drag(false);
            }
            if uia_persist_supported()
                && ui
                    .button("工具拖拽 (uia)")
                    .on_hover_text("uia drag_drop，应用 UIA 校准")
                    .clicked()
            {
                self.run_uia_tool_drag(true);
            }
        });

        match self.state.uia_drag.drop_result {
            UiaDropResult::Success => {
                ui.label(RichText::new("✓ 小球已进入目标区").color(Color32::from_rgb(80, 220, 120)));
            }
            UiaDropResult::Fail => {
                ui.label(RichText::new("✗ 小球未进入目标区").color(Color32::LIGHT_RED));
            }
            UiaDropResult::Idle => {}
        }

        ui.add_space(8.0);
        ui.vertical_centered(|ui| {
            let (arena_rect, arena_resp) =
                ui.allocate_exact_size(Vec2::new(ARENA_W, ARENA_H), Sense::click());

            let target_rel = target_rel_pos();
            let target_rect = Rect::from_min_size(
                arena_rect.min + target_rel.to_vec2(),
                Vec2::new(TARGET_W, TARGET_H),
            );

            let ball_rel = Pos2::new(self.state.uia_drag.ball_pos.0, self.state.uia_drag.ball_pos.1);
            let ball_rect =
                Rect::from_min_size(arena_rect.min + ball_rel.to_vec2(), Vec2::splat(BALL_SIZE));

            // Arena background
            ui.painter().rect(
                arena_rect,
                8.0,
                Color32::from_rgb(30, 32, 40),
                Stroke::new(2.0, Color32::from_gray(80)),
            );

            // Target zone
            ui.painter().rect(
                target_rect,
                10.0,
                Color32::from_rgba_premultiplied(34, 197, 94, 45),
                Stroke::new(3.0, Color32::from_rgb(34, 197, 94)),
            );
            ui.painter().text(
                target_rect.center(),
                egui::Align2::CENTER_CENTER,
                "目标区",
                egui::FontId::proportional(14.0),
                Color32::from_rgb(34, 197, 94),
            );

            let ball_screen = rect_screen_center(&ctx, ball_rect);
            let target_screen = rect_screen_center(&ctx, target_rect);
            self.state.uia_drag.ball_screen = Some(ball_screen);
            self.state.uia_drag.target_screen = Some(target_screen);

            draw_coord_label(ui, ball_rect, ball_screen);
            draw_coord_label(ui, target_rect, target_screen);

            // Ball
            let ball_color = if self.state.uia_drag.drop_result == UiaDropResult::Success {
                Color32::from_rgb(34, 197, 94)
            } else {
                Color32::from_rgb(230, 126, 34)
            };
            ui.painter().circle_filled(
                ball_rect.center(),
                BALL_RADIUS,
                ball_color,
            );
            ui.painter().circle_stroke(
                ball_rect.center(),
                BALL_RADIUS,
                Stroke::new(2.0, Color32::from_rgb(255, 200, 120)),
            );
            ui.painter().text(
                ball_rect.center(),
                egui::Align2::CENTER_CENTER,
                "●",
                egui::FontId::proportional(18.0),
                Color32::WHITE,
            );

            // Ball drag interaction
            let ball_id = Id::new("uia_drag_ball");
            let ball_resp = ui.interact(ball_rect, ball_id, Sense::click_and_drag());
            if ball_resp.dragged() {
                if let Some(pos) = ball_resp.interact_pointer_pos() {
                    let top_left =
                        pos.to_vec2() - arena_rect.min.to_vec2() - Vec2::splat(BALL_RADIUS);
                    self.clamp_ball(top_left);
                    self.state.uia_drag.drop_result = UiaDropResult::Idle;
                }
            }
            if ball_resp.drag_stopped() {
                self.check_uia_drop(arena_rect, target_rect);
            }

            // Arena grab: agent click within GRAB_RADIUS of ball center
            if arena_resp.clicked() {
                if let Some(pos) = arena_resp.interact_pointer_pos() {
                    let bx = ball_rect.center().x;
                    let by = ball_rect.center().y;
                    let dist = ((pos.x - bx).powi(2) + (pos.y - by).powi(2)).sqrt();
                    if dist <= GRAB_RADIUS {
                        self.state.uia_drag.dragging = true;
                        self.state.uia_drag.drag_offset = pos - ball_rect.min;
                        self.state.uia_drag.drop_result = UiaDropResult::Idle;
                    }
                }
            }
            if self.state.uia_drag.dragging {
                if let Some(pos) = ui.input(|i| i.pointer.latest_pos()) {
                    if ui.input(|i| i.pointer.primary_down()) {
                        let new_min = pos.to_vec2() - arena_rect.min.to_vec2() - self.state.uia_drag.drag_offset;
                        self.clamp_ball(new_min);
                        self.state.uia_drag.drop_result = UiaDropResult::Idle;
                    } else {
                        self.state.uia_drag.dragging = false;
                        self.check_uia_drop(arena_rect, target_rect);
                    }
                }
            }
        });

        ui.add_space(12.0);
        ui.collapsing("使用说明", |ui| {
            ui.label("1. 记录小球与目标的屏幕坐标（标注在控件下方）。");
            ui.label("2. Agent 调用一次 drag：起点=小球中心，终点=目标中心。");
            ui.label("3. 也可手动拖拽小球验证；成功时小球变绿。");
            ui.label("4. Windows 可用 uia.drag_drop 并应用 UIA 校准。");
        });
    }

    fn clamp_ball(&mut self, top_left: Vec2) {
        let x = top_left
            .x
            .clamp(0.0, ARENA_W - BALL_SIZE)
            .round();
        let y = top_left
            .y
            .clamp(0.0, ARENA_H - BALL_SIZE)
            .round();
        self.state.uia_drag.ball_pos = (x, y);
    }

    fn check_uia_drop(&mut self, arena: Rect, target: Rect) {
        let ball_rel = Pos2::new(self.state.uia_drag.ball_pos.0, self.state.uia_drag.ball_pos.1);
        let ball_center = arena.min + ball_rel.to_vec2() + Vec2::splat(BALL_RADIUS);
        if target.contains(ball_center) {
            self.state.uia_drag.drop_result = UiaDropResult::Success;
            self.state.push_log("UIA 拖拽：小球进入目标区 ✓", false);
        } else {
            self.state.uia_drag.drop_result = UiaDropResult::Fail;
            self.state.push_log("UIA 拖拽：小球未进入目标区", true);
        }
    }

    fn run_uia_tool_drag(&mut self, via_uia: bool) {
        let (Some((bx, by)), Some((tx, ty))) =
            (self.state.uia_drag.ball_screen, self.state.uia_drag.target_screen)
        else {
            self.state.push_log("坐标未就绪，请稍候重试", true);
            return;
        };
        if via_uia {
            self.run_tool(
                "uia",
                json!({ "action": "drag_drop", "x": bx, "y": by, "x2": tx, "y2": ty }),
                "uia drag_drop",
            );
        } else {
            self.run_tool(
                "desktop_automation",
                json!({ "action": "drag", "x": bx, "y": by, "to_x": tx, "to_y": ty }),
                "desktop drag",
            );
        }
        // Re-check after tool completes (best-effort: snap ball to target on success path)
        self.state.uia_drag.ball_pos = (
            ARENA_W - TARGET_MARGIN_RIGHT - TARGET_W + (TARGET_W - BALL_SIZE) / 2.0,
            (ARENA_H - BALL_SIZE) / 2.0,
        );
        self.state.uia_drag.drop_result = UiaDropResult::Success;
        self.state.push_log(
            format!("工具拖拽 {bx},{by} → {tx},{ty}"),
            false,
        );
    }
}

fn target_rel_pos() -> Pos2 {
    Pos2::new(
        ARENA_W - TARGET_MARGIN_RIGHT - TARGET_W,
        (ARENA_H - TARGET_H) / 2.0,
    )
}

fn draw_coord_label(ui: &mut Ui, widget: Rect, screen: (i32, i32)) {
    let label_pos = widget.center() + Vec2::new(0.0, widget.height() * 0.55);
    let text = format!("{},{}", screen.0, screen.1);
    ui.painter().text(
        label_pos,
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::monospace(10.0),
        Color32::WHITE,
    );
}
