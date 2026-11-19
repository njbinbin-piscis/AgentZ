//! Convert egui widget rects to physical screen pixel coordinates.

use egui::{Context, Pos2, Rect};

/// Viewport origin in screen space (UI points), when the backend provides it.
pub fn viewport_offset(ctx: &Context) -> Pos2 {
    ctx.input(|i| i.viewport().inner_rect)
        .map(|r| r.min)
        .unwrap_or(Pos2::ZERO)
}

/// Center of `rect` (viewport-local UI points) → physical screen pixels.
pub fn rect_screen_center(ctx: &Context, rect: Rect) -> (i32, i32) {
    let ppp = ctx.pixels_per_point();
    let off = viewport_offset(ctx);
    let c = rect.center();
    (
        ((off.x + c.x) * ppp).round() as i32,
        ((off.y + c.y) * ppp).round() as i32,
    )
}

/// Best-effort physical pixels for a pointer position inside `rect`.
pub fn pointer_screen_physical(ctx: &Context, rect: Rect, pointer: Option<Pos2>) -> (i32, i32) {
    let ppp = ctx.pixels_per_point();
    let off = viewport_offset(ctx);
    let pos = pointer.unwrap_or(rect.center());
    (
        ((off.x + pos.x) * ppp).round() as i32,
        ((off.y + pos.y) * ppp).round() as i32,
    )
}
