# AgentZ 1.0.6 发布说明

发布日期：2026-10-21。版本号与 `CHANGELOG.md` 的 1.0.6 条目一致。

AgentZ 1.0.6 修复了 `WorkZ` 与 `CodeZ` 输入框（Composer 及底栏）下拉菜单被窗口边缘裁剪或被其他 UI 遮挡的问题，并修复了字体大小设置（`Aa`）不影响代码编辑器与终端文字大小的问题。

## 修复

- **Composer 下拉菜单（模型、技能、模式、智能体、连接器等）防遮挡与防越界**：
  - `DropdownSelect` 与 `DropdownMultiSelect` 弹层通过 `createPortal(..., document.body)` 挂载至顶层（`z-index: 100100`），脱离所有父容器的 `overflow: hidden/clip` 与层叠上下文限制。
  - 修复向上弹出菜单（`placement="up"`）在窗口较矮或内容较多时错误向下翻转至窗口底部外侧的问题；支持根据上方可用空间动态收敛 `maxHeight`，并在右侧靠近窗口边缘时自动左移对齐，同时兼容 `.agentz-app` 的 `zoom` 坐标归一化。
  - 技能多选菜单改为内联展示双行截断的技能说明，不再在悬停时弹出遮挡下拉菜单的浮动 Toast。
- **编辑器与终端字号随全局字号缩放并支持独立调节**：
  - `CodeEditor`、`DiffEditor`、`NotebookEditor`、`Terminal` 及 `WorkZ` 文件预览/编辑画布的字体大小现在随顶栏 `Aa` 缩放比例（扩展至 `88%`–`150%`）实时同步缩放。
  - 在顶栏 `Aa` 菜单中新增「编辑器字号」独立步进调节（`−` / 点击重置默认 `14px` / `+`），并在编辑器内启用 `Ctrl + 鼠标滚轮` 缩放以及 `Ctrl/Cmd + =`、`Ctrl/Cmd + -`、`Ctrl/Cmd + 0` 快捷键。
