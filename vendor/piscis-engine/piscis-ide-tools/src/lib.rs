//! Shared IDE agent tools — LSP stack and diagnostics helpers used by desktop hosts.

pub mod lsp;
pub mod tools;

pub use tools::lsp::LspTool;
pub use tools::read_lints::ReadLintsTool;
