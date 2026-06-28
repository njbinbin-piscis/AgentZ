//! CLI helper for Wiki/graph verification (see `scripts/verify-wiki-graph.sh`).
//!
//! Usage: cargo run --example graph_verify -- <project_root> [query]

use std::env;
use std::path::Path;

use agentz_desktop_lib::commands::graph_db::explore_graph;
use agentz_desktop_lib::commands::graph_index::{request_rebuild, status, IndexPhase};

fn main() {
    let root = env::args()
        .nth(1)
        .expect("usage: graph_verify <project_root> [query]");
    let query = env::args().nth(2).unwrap_or_else(|| "graph_context_block".into());
    let root = Path::new(&root);

    eprintln!("==> queue background index …");
    let ack = request_rebuild(root.to_path_buf());
    eprintln!("    {}", ack.message);

    for _ in 0..120 {
        let st = status(root);
        if st.phase == IndexPhase::Idle {
            eprintln!(
                "    graph.db: {} nodes, {} edges",
                st.nodes, st.edges
            );
            if let Some(err) = st.last_error {
                eprintln!("    error: {err}");
            }
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    eprintln!("==> explore_graph({query:?}) …\n");
    let out = explore_graph(root, &query, 12).expect("explore_graph");
    print!("{out}");
}
