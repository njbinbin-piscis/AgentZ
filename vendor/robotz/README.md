# RobotZ

A standalone **computer-use agent toolkit**, extracted from
[openpiscis](../openpiscis)'s desktop-automation layer.

RobotZ packages the ability to *drive a computer* — take screenshots, move the
mouse, type, manage windows, read the Windows UI Automation tree, and automate
a browser over the Chrome DevTools Protocol — as a set of reusable tools, and
exposes them as an **MCP server** so any MCP client (Cursor, Claude Desktop, …)
can control the machine.

## Workspace layout

| Crate | Responsibility |
|-------|----------------|
| `robotz-core` | Dependency-free `Tool` / `ToolResult` / `ImageData` abstraction + cross-platform `proc` spawn helper. |
| `robotz-automation` | Desktop control: screen capture, mouse/keyboard, window management, Windows UIA, mouse calibration. |
| `robotz-browser` | Chrome-DevTools-Protocol web automation. |
| `robotz-mcp` | MCP server binary (`rmcp`, stdio) exposing the tools to any MCP client. |
| `robotz-toolset` | Shared tool registry used by the MCP server and host. |
| `robotz-host` | GUI 测试与屏幕校准宿主（egui 面板：五点校准、靶场、MCP、基准）。 |

## Run the MCP server

```bash
cargo run -p robotz-mcp            # full tool set
cargo run -p robotz-mcp -- --readonly   # observation-only (screenshots, reads)
```

## Run the example host（GUI 屏幕校准与测试）

```bash
cargo run -p robotz-host              # 打开 GUI（默认「屏幕校准」页）
cargo run -p robotz-host -- tools     # 打印工具名与说明
cargo run -p robotz-host -- inspect   # 无头冒烟（显示器 / 光标）
cargo run -p robotz-host -- mcp-demo  # 子进程连接 robotz-mcp 并调用工具
cargo run -p robotz-host -- bench     # 基准测试 → ~/.local/share/robotz-host/bench-latest.json
```

### GUI 标签页

| 标签 | 用途 |
|------|------|
| **屏幕校准** | 左侧向导 + 中央五点靶心；测量点击偏差、导出 JSON；Windows 可保存 UIA 校准 |
| **操作测试** | 完整 5×4 靶场、拖拽区、键盘输入，配合右侧快捷操作 |
| **UIA 拖拽** | 橙色小球拖入绿色目标区（移植自 openpiscis Debug）；标注物理坐标，支持一次 `drag` / `uia.drag_drop` |
| **计算器** | 左侧简易计算器（0–9、+−×÷、=、C）；中央显示按键坐标表，供 Agent 点击验算 |
| **高级** | MCP 子进程连接、基准测试 JSON |

### 五点屏幕校准流程

1. 打开 `robotz-host`，确认在 **屏幕校准** 页。
2. 左侧点 **▶ 开始五点校准**。
3. 在中央高亮靶心（#0、#4、#9、#14、#19）上 **用鼠标点击一次**，记录物理坐标。
4. 左侧点 **◎ 采样此点（自动点击）** — 工具自动点击并读取实际光标位置，表格显示偏差。
5. 重复 3–4 共五点；可 **导出测量报告 JSON**（`pointer-calibration-report.json`）。
6. **Windows**：可选 **保存为 UIA 校准**（`uia_calibration.json`），供 `uia.click` 自动纠偏。  
   **Linux / macOS**：可测量与导出报告，UIA 持久化仅 Windows。

数据目录：`~/.local/share/robotz-host/`（Windows 为 `%LOCALAPPDATA%\robotz-host\`）。

面板显示每个靶心的 **物理像素坐标**（供 `desktop_automation.click` 使用）。将 MCP 客户端指向
`robotz-mcp` 可自动化本窗口，做端到端 computer-use 验证。

**Linux**: requires an X11 session plus `xdotool`, `wmctrl`, and `xclip` (see
Runtime dependencies below).

Register it with an MCP client (e.g. Cursor / Claude Desktop) by pointing the
client at the built `robotz-mcp` binary over stdio. It exposes `screen_capture`,
`desktop_automation`, `browser`, and (on Windows) `uia`, with screenshots
returned as MCP image content.

The `robotz-*` crates expose an optional **`piscis-kernel`** feature that
re-implements `piscis_kernel::Tool` on top of the same structs, so openpiscis can
consume RobotZ as a drop-in replacement for its in-tree tools.

## Runtime dependencies

These are *external programs* the tools shell out to (not bundled):

- **Linux**: an X11 session, `xdotool`, `wmctrl`, `xclip`; the bundled
  `xi_helpers` C helper for smooth pointer movement.
- **macOS**: `cliclick`, `osascript`; Accessibility permission granted.
- **Windows**: built-in UI Automation runtime + PowerShell.
- **Browser tool**: Chrome / Chrome for Testing (auto-downloaded on first use).

## Install from GitHub Releases

Pre-built **robotz-host** (test panel) and **robotz-mcp** (MCP server) binaries
are attached to [GitHub Releases](https://github.com/njbinbin-piscis/RobotZ/releases).

**Linux / macOS** (download by version):

```bash
curl -fsSL https://raw.githubusercontent.com/njbinbin-piscis/RobotZ/v0.1.1/scripts/install.sh | bash -s -- v0.1.1
robotz-host    # open test panel
robotz-mcp     # MCP server on stdio
```

Or extract the `.tar.gz` for your platform and run `./install.sh` inside the folder.

**Windows** — download `robotz-*-pc-windows-msvc.zip`, extract, then in PowerShell:

```powershell
.\install.ps1
robotz-host.exe
```

Ensure runtime dependencies (see below) are installed before driving the desktop.

## Status

Functional. All crates build and the MCP server responds to
`initialize` / `tools/list`. The optional `piscis-kernel` feature (see each
crate's `piscis_bridge`) lets openpiscis consume these crates as a drop-in
replacement for its in-tree tools — that migration (removing the originals from
openpiscis) is the remaining step.
