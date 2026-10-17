//! Live smoke test against a real rust-analyzer. Run with:
//! `cargo test -p piscis-ide-tools --test lsp_live -- --ignored --nocapture`

use piscis_ide_tools::lsp::manager::LspManager;
use serde_json::json;
use std::time::Duration;

#[tokio::test]
#[ignore]
async fn rust_analyzer_symbols_and_reuse() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../piscis-ide-tools");
    let root = std::fs::canonicalize(root).unwrap().to_string_lossy().to_string();
    let root = root.trim_start_matches(r"\\?\").to_string();
    let file = format!("{root}/src/lsp/client.rs");
    let mgr = LspManager::new();

    let c1 = mgr.client(&root, "rust").await.expect("spawn rust-analyzer");
    let (uri, _) = c1.sync_file(&file, "rust").await.unwrap();
    let mut syms = serde_json::Value::Null;
    for _ in 0..20 {
        syms = c1
            .request(
                "textDocument/documentSymbol",
                json!({ "textDocument": { "uri": uri } }),
                Duration::from_secs(30),
            )
            .await
            .unwrap();
        if syms.as_array().is_some_and(|a| !a.is_empty()) {
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    println!("{syms}");
    assert!(syms.to_string().contains("LspClient"));

    // Second call must reuse the same live server instead of "connection refused".
    let c2 = mgr.client(&root, "rust").await.unwrap();
    assert!(std::sync::Arc::ptr_eq(&c1, &c2));
    assert!(c2.is_alive());
}
