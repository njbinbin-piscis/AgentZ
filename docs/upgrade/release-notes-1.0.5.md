# AgentZ 1.0.5 发布说明

发布日期：2026-10-21。版本号与 `CHANGELOG.md` 的 1.0.5 条目一致。

AgentZ 1.0.5 修复编辑器打开代码后右下角持续显示「语言服务不可用」的问题。语言服务桥现在把握手转发给真实语言服务器，并只对已安装服务的语言显示状态。

## 修复

- **真实 LSP 握手**：本地 WebSocket 桥不再拦截 `initialize`，按 `Content-Length` 帧转发请求与响应，`rust-analyzer` 与 `typescript-language-server` 可以完成初始化。
- **监听竞态**：桥在返回端口前完成绑定，编辑器不会连到尚未监听的端口。
- **状态提示**：仅 `rust`、`typescript`、`python`、`cpp` 启动语言服务。JSON、Markdown 等只有语法高亮的文件不再显示「语言服务不可用」。
- **Python 回退**：未安装全局 `pyright-langserver` 时，使用已安装的 `ms-python.python` 扩展自带的 Jedi 语言服务器。
