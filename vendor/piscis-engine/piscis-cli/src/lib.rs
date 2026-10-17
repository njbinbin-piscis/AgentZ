//! piscis-cli — headless host adapter for the OpenPiscis kernel.
//!
//! This crate implements the [`piscis_core::host`] traits for a pure CLI
//! environment: events are serialised as JSON lines to stdout, confirmation /
//! interactive prompts fall back to deterministic defaults, and no
//! platform-specific tools are registered. It exists so that the
//! `openpiscis-headless` binary no longer needs Tauri at runtime.
//!
//! The concrete `CliHost` and its `EventSink` / `Notifier` implementations
//! live in submodules; the binary under `src/bin/openpiscis.rs` wires them up
//! with the kernel's headless entry point.

pub mod args;
pub mod host;
pub mod interactive;
pub mod rpc_server;
pub mod runner;
