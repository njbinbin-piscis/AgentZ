//! `robotz-browser` — Chrome-DevTools-Protocol web automation, extracted from
//! openpiscis.
//!
//! [`BrowserTool`] (`browser`) drives a managed Chrome instance: navigate,
//! click, type, screenshot, Set-of-Mark element annotation, DOM interaction.
//! [`BrowserManager`] owns the Chrome-for-Testing lifecycle and page pool.
//!
//! Implements [`robotz_core::Tool`]; enable the `piscis-kernel` feature for the
//! `piscis_kernel::Tool` bridge so openpiscis can consume it directly.

pub mod download;
pub mod manager;
pub mod snapshot;
pub mod tool;

pub use manager::{create_browser_manager, BrowserManager, BrowserOptions, SharedBrowserManager};
pub use tool::BrowserTool;

#[cfg(feature = "piscis-kernel")]
pub mod piscis_bridge;
