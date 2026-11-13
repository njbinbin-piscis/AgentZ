# AgentZ 右键菜单审计与改造（2026-11-13）

## 背景

桌面 IDE 的 webview 此前未屏蔽 Chromium 默认右键菜单（Back / Reload / Save as / Inspect…），
在绝大多数界面上右键会弹出浏览器菜单，泄漏内部实现且提供会破坏应用外壳的操作。
本次改造：**全局屏蔽默认菜单 + 统一可复用 ContextMenu + 按组件补齐语义化右键菜单**。

## 架构

| 层 | 文件 | 职责 |
|---|---|---|
| 全局守卫 | `src/services/contextMenuGuard.ts` | document 冒泡阶段拦截 `contextmenu`：组件已 `preventDefault` 则让行；`.monaco-editor` 与 `[data-native-context-menu]` 保留原生菜单；其余阻止默认并打开通用菜单（复制/剪切/粘贴/全选 + 链接/图片动作） |
| 共享组件 | `src/components/ContextMenu.tsx` + `.css` | Portal 到 body、视口内钳位（复用 `useContextMenuPosition`）、键盘导航（↑↓/Home/End/Enter/Esc）、外点/滚动/失焦关闭、danger 红色项、disabled 灰显、`useContextMenu()` hook（`openWith(items)` + `element`） |
| 宿主 | `src/App.tsx` | 安装守卫 + 渲染通用默认菜单 |
| i18n | `src/i18n/en.ts` / `zh.ts` | 新增 `contextMenu.*` 36 键，中英同步（parity 测试校验） |

**事件次序**：React 组件 handler（root 冒泡）先于 document 守卫执行 → 组件菜单优先；
菜单自身的关闭监听只 close、不 `preventDefault`，右键换目标时旧菜单关闭、新菜单正常打开。

**惰性菜单**：`useContextMenu().openWith` 接受 `items` 或 `(e) => items` 工厂。各组件用工厂在
**右键时刻**构建菜单（读取实时选区），而非渲染期——保持渲染纯净、无 DOM 依赖（node 测试环境可渲染）。

**附带修复**：
- `theme.ts` 存储访问防御化（`readStorage`）：localStorage 缺失**或不可用**（测试桩/隐私模式/嵌入式 webview/Node 实验性全局）时返回默认值而非崩溃。
- `useCodezWorkspace.ts` 补 `agentz-open-file` 监听，打通编辑卡片"打开文件"。

## 组件审计表

### 已有自定义菜单（保留，未改动）

| 组件 | 位置 | 菜单动作 |
|---|---|---|
| 文件树 | `codez/FileTree.tsx` + `codez/index.tsx` | 打开/新建/重命名/删除/复制路径等 |
| 编辑器标签页 | `codez/EditorTabs.tsx` | 关闭/关闭其他/关闭全部等 |
| Monaco 编辑器 | `.monaco-editor` | **保留原生编辑器菜单**（守卫白名单） |

### 本次新增语义化菜单

| 组件 | 位置 | 菜单动作 |
|---|---|---|
| 聊天消息（用户+助手） | `codez/AssistantMessageList.tsx` | 复制消息、复制选中内容；助手消息另有：从此处分叉会话、恢复到此检查点（danger） |
| Markdown 代码块 | `codez/Markdown.tsx` CodeBlock | 复制代码、复制选中内容、复制为 Markdown（带语言围栏） |
| 工具调用行 | `components/ToolTrace.tsx` ToolTraceRow | 复制工具名/输入/输出 |
| 文件编辑卡片 | `components/ToolTrace.tsx` FileEditCard | 打开文件、复制路径、复制 diff（hint 显示 +N -N）。**修复**：`agentz-open-file` 事件此前只有派发没有监听（点击卡片也是空操作），已在 `useCodezWorkspace.ts` 补上监听桥接到真实 `openFile` |
| 会话列表（CodeZ） | `codez/AssistantPanel.tsx` | 打开会话、复制标题、删除会话（danger） |
| 任务列表（WorkZ） | `workz/WorkzSidebar.tsx` | 打开会话、复制标题、删除会话（danger） |
| 项目分组行（WorkZ） | `workz/WorkzSidebar.tsx` | 在 CodeZ 打开、新建会话、复制路径、移除项目（danger） |
| 协作看板待办卡 | `workz/CollabBoard.tsx` | 复制标题、复制待办 ID、复制负责人 |
| 终端 | `codez/Terminal.tsx` | 复制（无选中禁用）、粘贴（写入 PTY）、全选、发送到聊天（有选中时，带快捷键 hint）、清空终端。**修复**：原实现无选中时直接 return → 弹浏览器菜单；现任何右键都有菜单 |
| 浏览器画布 | `codez/BrowserPanel.tsx` | 后退、前进、重新加载页面、复制页面地址、截图到聊天 |

### 由通用默认菜单覆盖（无需专属菜单）

| 表面 | 默认菜单行为 |
|---|---|
| 聊天输入框 / 浏览器地址栏 / 各文本输入 | 剪切/复制/粘贴/全选（粘贴用 `setRangeText` + input 事件，兼容 React 受控组件） |
| 聊天内链接 | 打开链接（经 linkRouter 路由：内置浏览器/文件预览/系统程序）、复制链接地址 |
| 聊天内图片 | data:/blob: 图片 → **复制图片**（转 PNG 写剪贴板，失败回退复制地址）；远程图片 → 复制图片地址 |
| 普通文本区域 | 复制（无选中禁用）、全选（就近选择 markdown/工具输出容器） |
| 设置/向导/看板空白区等 | 复制/全选兜底，不再出现浏览器菜单 |

### 明确排除

- **Monaco 编辑器**：原生菜单是编辑器体验的一部分（重构/跳转/格式化），白名单保留。
- **嵌入式浏览器页面内容**：画布是截图 img，右键给的是面板级导航菜单；页面内元素交互走既有 pick/inspect 模式。
- 需要原生菜单的未来表面可加 `data-native-context-menu` 属性 opting-out。

## 验证

- 新增测试 18/18 通过：`ContextMenu.test.tsx`（7：打开/点击执行/Escape/键盘导航/跳过 disabled/外点关闭/空态）、`contextMenuGuard.test.ts`（11：屏蔽/组件优先/Monaco 白名单/opt-out/卸载/链接/输入框/无选中禁用/data 图片/远程图片/isEditableTarget）
- 全量回归 **91 文件 / 473 测试全部通过**（含修复后的 Terminal.lifecycle 7/7、AssistantMessageList 3/3、fileEditDiff）
- `tsc --noEmit` 全量 0 错误；`read_lints` 全部改动文件 0 错误 0 告警
- i18n parity（en/zh 36 键同步）通过
