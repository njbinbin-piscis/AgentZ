#!/usr/bin/env bash
# Install RobotZ release binaries into ~/.local/share/robotz and ~/.local/bin
set -euo pipefail

VERSION="${1:-}"
INSTALL_DIR="${ROBOTZ_INSTALL_DIR:-$HOME/.local/share/robotz}"
BIN_DIR="${ROBOTZ_BIN_DIR:-$HOME/.local/bin}"
REPO="njbinbin-piscis/RobotZ"

if [[ -n "$VERSION" && "$VERSION" != v* ]]; then
  VERSION="v${VERSION}"
fi

detect_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os" in
    Linux)
      case "$arch" in
        x86_64) echo "x86_64-unknown-linux-gnu" ;;
        aarch64|arm64) echo "aarch64-unknown-linux-gnu" ;;
        *) echo "unsupported Linux arch: $arch" >&2; exit 1 ;;
      esac
      ;;
    Darwin)
      case "$arch" in
        arm64) echo "aarch64-apple-darwin" ;;
        x86_64) echo "x86_64-apple-darwin" ;;
        *) echo "unsupported macOS arch: $arch" >&2; exit 1 ;;
      esac
      ;;
    *)
      echo "unsupported OS: $os (use release archive + manual install)" >&2
      exit 1
      ;;
  esac
}

install_from_archive() {
  local archive="$1"
  local tmp
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT
  tar xzf "$archive" -C "$tmp"
  local root
  root="$(find "$tmp" -mindepth 1 -maxdepth 1 -type d | head -1)"
  mkdir -p "$INSTALL_DIR/bin" "$BIN_DIR"
  install -m 755 "$root/bin/robotz-host" "$INSTALL_DIR/bin/"
  install -m 755 "$root/bin/robotz-mcp" "$INSTALL_DIR/bin/"
  ln -sf "$INSTALL_DIR/bin/robotz-host" "$BIN_DIR/robotz-host"
  ln -sf "$INSTALL_DIR/bin/robotz-mcp" "$BIN_DIR/robotz-mcp"
  echo "Installed to $INSTALL_DIR/bin (linked from $BIN_DIR)"
}

install_from_github() {
  local ver target url archive
  ver="${VERSION:?Pass version e.g. v0.1.1 or run from extracted archive dir}"
  target="$(detect_target)"
  archive="robotz-${ver#v}-${target}.tar.gz"
  url="https://github.com/${REPO}/releases/download/${ver}/${archive}"
  echo "Downloading $url"
  tmp="$(mktemp)"
  curl -fsSL "$url" -o "$tmp"
  install_from_archive "$tmp"
}

# Running inside an extracted release directory
if [[ -f ./bin/robotz-host && -f ./bin/robotz-mcp ]]; then
  mkdir -p "$INSTALL_DIR/bin" "$BIN_DIR"
  install -m 755 ./bin/robotz-host "$INSTALL_DIR/bin/"
  install -m 755 ./bin/robotz-mcp "$INSTALL_DIR/bin/"
  ln -sf "$INSTALL_DIR/bin/robotz-host" "$BIN_DIR/robotz-host"
  ln -sf "$INSTALL_DIR/bin/robotz-mcp" "$BIN_DIR/robotz-mcp"
  echo "Installed to $INSTALL_DIR/bin (linked from $BIN_DIR)"
  echo "Run: robotz-host   # test panel"
  echo "     robotz-mcp    # MCP server"
  exit 0
fi

if [[ -n "$VERSION" ]]; then
  install_from_github
else
  echo "Usage:"
  echo "  $0 v0.1.1          # download release from GitHub"
  echo "  cd robotz-* && ./install.sh   # install from extracted folder"
  exit 1
fi

echo "Run: robotz-host   # visual test panel"
echo "     robotz-mcp    # MCP server (register in Cursor MCP config)"
