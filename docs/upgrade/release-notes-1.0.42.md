# AgentZ 1.0.42 发布说明

发布日期：2026-11-02。完整变更见 `CHANGELOG.md` 的 1.0.42 节。

## 修复

- **HTML 文件预览中的脚本可以运行。** 编辑器里的 HTML 预览和 Agent 文件预览之前的沙箱没有 `allow-scripts`，页面里的 JavaScript 完全不执行。现在预览在隔离源中运行脚本：不能访问应用本身，不能发起网络请求，不能提交表单。该环境下不能使用 `localStorage`，也不能加载外部 CDN 脚本。

## 说明

- 引擎引用仍为 piscis-engine `v0.8.72`。
