//! `robotz-automation` — cross-platform desktop control tools extracted from
//! openpiscis.
//!
//! - [`ScreenTool`] (`screen_capture`): full-screen / window / region capture,
//!   multi-monitor enumeration, coordinate-grid + cursor-crosshair overlays.
//! - [`DesktopAutomationTool`] (`desktop_automation`): mouse, keyboard, window
//!   management, app launching.
//! - `UiaTool` (`uia`, Windows only): UI Automation tree navigation, precise
//!   element click/type/drag, backed by [`calibration`] for residual-drift
//!   compensation.
//!
//! All tools implement [`robotz_core::Tool`]. Enable the `piscis-kernel`
//! feature to additionally implement `piscis_kernel::agent::tool::Tool` (see
//! [`piscis_bridge`]) so openpiscis can consume the very same structs.

pub mod calibration;
pub mod desktop_automation;
pub mod screen;

#[cfg(target_os = "windows")]
pub mod uia;

pub use desktop_automation::DesktopAutomationTool;
pub use screen::ScreenTool;

#[cfg(target_os = "windows")]
pub use uia::UiaTool;

#[cfg(feature = "piscis-kernel")]
pub mod piscis_bridge;
