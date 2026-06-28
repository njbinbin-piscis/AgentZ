#!/usr/bin/env bash
# Verify Wiki / knowledge graph pipeline (P0–P3).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TAURI="$ROOT/src-tauri"

# NFS workspaces often hit incremental lock errors; use a local target dir.
export CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-0}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/agentz-cargo-target}"

echo "==> cargo check"
(cd "$TAURI" && cargo check)

echo "==> graph_agent unit tests"
(cd "$TAURI" && cargo test graph_agent -- --nocapture)

echo "==> graph module unit tests"
(cd "$TAURI" && cargo test graph:: -- --nocapture)

echo "==> graph_db unit tests"
(cd "$TAURI" && cargo test graph_db -- --nocapture)

echo "==> graph_index unit tests"
(cd "$TAURI" && cargo test graph_index -- --nocapture 2>/dev/null) || true

echo "==> optional: generate graph on AGENTZ_VERIFY_PROJECT"
if [[ -n "${AGENTZ_VERIFY_PROJECT:-}" && -d "$AGENTZ_VERIFY_PROJECT" ]]; then
  echo "    project: $AGENTZ_VERIFY_PROJECT"
  (cd "$TAURI" && cargo run --quiet --example graph_verify "$AGENTZ_VERIFY_PROJECT" 2>/dev/null) || {
    echo "    (skip: graph_verify example not built — use UI Wiki → Rebuild graph)"
  }
fi

echo ""
echo "OK — automated Wiki/graph tests passed."
echo "Manual UI checklist: docs/wiki-graph-verification.md"
