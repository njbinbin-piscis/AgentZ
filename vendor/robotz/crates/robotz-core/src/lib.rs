//! `robotz-core` — the minimal, dependency-free foundation every RobotZ
//! tool builds on.
//!
//! It intentionally mirrors the shape of `piscis_kernel::agent::tool` so that
//! the automation tools extracted from openpiscis compile against this crate
//! with a one-line import change (`piscis_kernel::agent::tool` →
//! `robotz_core`). An optional `piscis-kernel` feature in the downstream
//! crates then re-implements `piscis_kernel::Tool` on top of these types so
//! openpiscis can keep consuming the very same structs.

pub mod proc;
pub mod tool;

pub use tool::{ImageData, Tool, ToolContext, ToolResult};
