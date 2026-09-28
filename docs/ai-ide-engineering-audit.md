# AgentZ AI IDE 工程审计与修复指南

> 审计日期：2026-09-28  
> 审计范围：编辑器、Git 版本控制、Hint/AI Completion、LSP、性能、Extension Host、Agent Harness、安全边界、测试与工程维护性  
> 项目版本：`0.6.3`  
> 审计性质：只读代码审计与本地构建验证；审计过程中未修改产品代码

## 1. 执行摘要

AgentZ 已经形成一个较完整的桌面 AI IDE 技术体系：Tauri/Rust 后端、React/Monaco 编辑器、独立 Extension Host、LSP、Git、多 Agent、工作流、上下文压缩、文件 journal、代码索引和知识图谱等模块均已落地。

从长期维护和面向真实用户交付的标准看，当前更接近功能覆盖较广的 Beta：

- Rust 基础模块、工作流、索引和 gateway 测试质量较好。
- 编辑器、LSP 和 Git 的核心路径存在会直接影响功能正确性的缺陷。
- Agent Harness 已有 PolicyGate 和能力分层，但实际执行上下文默认绕过权限确认，安全模型存在断层。
- 前端测试、LSP 集成测试和 Extension Host 契约测试明显落后于功能增长。
- Monaco、Mermaid 和主应用的加载成本偏高，冷启动和首个编辑器可用时间有较大优化空间。
- 多个核心文件已超过 1,000 行，功能继续增长时回归风险会快速上升。

综合评价：

| 维度 | 当前评价 | 说明 |
| --- | ---: | --- |
| 架构潜力 | 8/10 | 分层方向合理，Rust kernel、Tauri host、React UI、Extension Host 边界基本明确 |
| 正确性/可靠性 | 6/10 | LSP 生命周期、模型 URI、协议竞态和 Git 路径解析存在关键缺陷 |
| 安全边界 | 5/10 | ToolContext 绕过权限、Mermaid loose mode、CSP 关闭构成组合风险 |
| 性能 | 5/10 | Monaco、TS worker、主包和 Mermaid 图形依赖体积较大 |
| 测试成熟度 | 6/10 | Rust 单测较好，但 IDE 主路径和端到端测试不足 |

建议先处理本文 P0/P1 问题，再继续扩展新功能。

## 2. 审计方法与验证结果

### 2.1 已执行的验证

```bash
npm run typecheck
npm run lint
npm test -- --reporter=verbose
cargo check --workspace
cargo test --workspace
npm --prefix extension-host run build
npm run build
```

结果：

- TypeScript typecheck：通过。
- ESLint：0 errors，61 warnings，其中包含多个 React Hook 依赖问题。
- Vitest：5 个测试文件、21 个测试全部通过。
- Cargo check：通过。
- Rust tests：87 通过、1 个需要本地 Chromium 的测试被忽略。
- Extension Host：构建通过，`dist/host.js` 约 80 KB。
- Vite production build：通过，但存在 CSS 语法警告、无效动态拆包警告和多个大 chunk 警告。
- 审计开始和结束时未对现有业务文件进行修改；本文档是唯一新增文件。

### 2.2 关键源码规模

下列文件已经成为维护热点：

| 文件 | 约 LOC | 风险 |
| --- | ---: | --- |
| `src-tauri/src/commands/chat_turn.rs` | 2,288 | Agent turn 组装、工具注册、上下文、执行和持久化混合 |
| `src-tauri/src/gateway/wechat.rs` | 2,064 | 协议、网络、状态和解析职责集中 |
| `src-tauri/src/commands/ide.rs` | 1,560 | 文件、Git、终端、watcher、LSP command 聚合 |
| `src/workspaces/workz/index.tsx` | 1,521 | 工作区 UI 和大量状态集中 |
| `src/workspaces/codez/AssistantPanel.tsx` | 1,368 | 会话、事件、输入、计划、工具状态混合 |
| `src/workspaces/codez/index.tsx` | 1,323 | 文件树、标签页、watcher、Git 刷新和布局集中 |
| `src/workspaces/codez/CodeEditor.tsx` | 690 | Monaco、LSP、AI completion、inline edit、DAP 混合 |
| `src/services/tauri/lsp.ts` | 737 | 协议、client、转换、Monaco provider 混合 |

## 3. 缺陷优先级总览

| ID | 优先级 | 模块 | 问题 | 主要影响 |
| --- | --- | --- | --- | --- |
| LSP-01 | P0 | 编辑器/LSP | Monaco model URI 与 provider 目标 URI 不一致 | Hover、补全、跳转、引用可能全部失效，marker 可能落到错误文件 |
| LSP-02 | P0 | 编辑器/LSP | 切换标签后断开 LSP，但未可靠重连 | 首个文件外的标签页失去 LSP |
| LSP-03 | P0 | LSP 协议 | initialize 响应存在丢失竞态 | 偶发连接等待 10 秒后失败 |
| LSP-04 | P0 | LSP 协议 | Content-Length 使用 UTF-16 字符长度 | 含中文/emoji 的消息长度不正确 |
| AGENT-01 | P0/P1 | Agent Harness | ToolContext 默认 `bypass_permissions: true` | 权限模型难以保证，写文件/执行命令等工具可能绕过确认 |
| SEC-01 | P1 | Markdown/Tauri | Mermaid `securityLevel: loose` 且 CSP 关闭 | 不可信内容渲染攻击面扩大 |
| GIT-01 | P1 | Git | porcelain v1 按行/字符串切片解析 | 空格、引号、中文、换行、rename 路径错误 |
| GIT-02 | P1 | Git | nested repo diff 混用 workspace path 与 repo-relative path | diff 为空或找不到文件 |
| GIT-03 | P1 | Git API | 多个 API 接收 `git_root` 后忽略 | UI 选择的 repo 与实际操作可能不一致 |
| EDIT-01 | P1 | Inline Edit | Reject 依赖全局 undo 栈 | 并发编辑后可能撤销用户修改 |
| HINT-01 | P1/P2 | AI Completion | 只有前端逻辑取消，没有后端请求取消 | 无效推理、成本增加、资源占用 |
| PERF-01 | P1/P2 | Build/启动 | Monaco、主包和 TS worker 体积过大 | 冷启动、编辑器 ready 时间和内存占用偏高 |
| CSS-01 | P1 | CSS | 注释内包含 `*/` 导致语法警告 | 后续 CSS 可能被错误解析 |
| TEST-01 | P1/P2 | 测试 | 编辑器/Git/LSP/Agent 权限缺少回归测试 | 核心缺陷难以及时发现 |
| MAINT-01 | P2 | 架构 | 核心模块职责和文件规模过大 | 修改成本和回归风险持续上升 |

## 4. 编辑器与 LSP

### 4.1 LSP-01：Monaco model URI 与 provider URI 不一致

#### 定位

- `src/workspaces/codez/CodeEditor.tsx:593-640`
- `src/services/tauri/lsp.ts:626-660`

`CodeEditor` 创建 Monaco `<Editor>` 时未传入 `path`：

```tsx
<Editor
  height="100%"
  theme={editorTheme}
  language={editorLanguage}
  value={tab.content}
  ...
/>
```

`@monaco-editor/react` 在没有 `path` 时通常使用 `inmemory://model/...` URI。但 LSP provider 使用文件 URI：

```ts
const modelUri = monaco.Uri.parse(`file://${filePath}`);
const matchesFile = (model) => model.uri.toString() === modelUri.toString();
```

#### 影响

- Completion provider 因 `matchesFile` 为 false 返回 `null`。
- Hover、Definition、References 同样失效。
- Diagnostics 找不到目标模型时使用 `monaco.editor.getModels()[0]`，可能把 marker 写到错误标签页。
- 多标签页打开顺序不同会使问题呈现随机性。

#### 修复方案

1. 建立唯一的 URI 工具函数：

```ts
export function editorUri(monaco: typeof Monaco, fullPath: string) {
  return monaco.Uri.file(fullPath);
}
```

2. 给 Editor 传入稳定 path：

```tsx
<Editor
  path={fullPath}
  keepCurrentModel
  ...
/>
```

3. `registerLspProviders`、markers、didOpen/didChange/didClose、definition/reference location 全部使用同一 URI 生成逻辑。
4. 删除以下 fallback：

```ts
monaco.editor.getModel(modelUri) ?? monaco.editor.getModels()[0]
```

目标 model 不存在时应跳过 marker，并记录可观测日志。

#### 验收测试

- 打开 TS、Rust、Python 文件后，model URI 必须为对应文件 URI。
- 同时打开两个同语言文件，诊断只出现在对应文件。
- Hover、Completion、Definition 和 References 的 provider 能匹配当前 model。
- 关闭文件后不会给已销毁 model 更新 marker。

### 4.2 LSP-02：标签切换后 LSP 生命周期断裂

#### 定位

- 初始化：`src/workspaces/codez/CodeEditor.tsx:483-535`
- cleanup：`src/workspaces/codez/CodeEditor.tsx:554-566`
- Editor mount：`src/workspaces/codez/CodeEditor.tsx:595-624`

LSP 初始化完全位于 `onMount` callback 中。标签切换时 React 可能复用同一个 Editor 组件，因此 `onMount` 不会再次执行。与此同时，以 `tab.path` 为依赖的 effect cleanup 会断开当前 client。

#### 可复现路径

1. 打开第一个支持 LSP 的文件。
2. 等待 LSP 连接。
3. 打开并切换至第二个文件。
4. 第一个 path 的 effect cleanup 调用 `disconnect()`。
5. Monaco 没有重新 mount，新文件没有执行 LSP start/connect/provider registration。

#### 修复方案

将职责拆开：

- `onMount`：只保存 `editor` 和 `monaco` 实例、注册全局命令。
- `useEffect([projectDir, language])`：获取或复用 project/language 级 LSP client。
- `useEffect([client, tab.path])`：执行 didOpen/didClose、注册文件级 provider/marker binding。
- project + language 对应一个长期存活的 LSP server/session，不应每次切换文件就停止 server。
- 每个 document 单独维护递增版本号。
- project 关闭或最后一个对应语言文档关闭时，再按策略回收 server。

建议的数据结构：

```ts
type DocumentState = {
  uri: string;
  version: number;
  opened: boolean;
};

type LanguageSession = {
  projectDir: string;
  languageId: string;
  client: LspClient;
  documents: Map<string, DocumentState>;
  refCount: number;
};
```

#### 验收测试

- A.ts → B.ts → A.ts 切换后两者 LSP 均可用。
- TS → Rust → TS 切换不会错误复用不同 language server。
- 快速连续切换 10 次不会留下 orphan WebSocket/provider。
- 关闭一个标签不会断开其他同语言标签使用的 server。

### 4.3 LSP-03：initialize 响应竞态

#### 定位

- `src/services/tauri/lsp.ts:155-245`
- `src/services/tauri/lsp.ts:435-462`

当前流程：

```ts
ws.send(initReq);
const initResp = await this.waitForResponse();
```

`waitForResponse()` 会临时替换 `ws.onmessage`。如果 initialize response 在替换 handler 前到达，原始 `handleMessage()` 会收到该响应；因为 initialize ID 没有加入 `pending`，响应被丢弃，最终等待 10 秒超时。

#### 修复方案

- WebSocket 生命周期内只允许一个 `onmessage` dispatcher。
- initialize 必须走统一的 `sendRequest("initialize", params)`。
- `sendRequest` 必须先注册 pending，再发送消息。
- pending 应包含 `resolve`、`reject` 和 timeout handle。
- server 返回 JSON-RPC error 时 reject，而不是将 `null` 当正常结果。
- disconnect/close/error 时 reject 全部 pending 请求。
- 删除 `waitForResponse()` 临时替换 handler 的实现。

参考结构：

```ts
type PendingRequest = {
  resolve: (value: unknown) => void;
  reject: (error: Error) => void;
  timeoutId: ReturnType<typeof setTimeout>;
};
```

#### 验收测试

- fake LSP server 同步/立即回复 initialize 时连接稳定成功。
- initialize 返回 error 时立即失败，不等待超时。
- WebSocket close 时所有 pending promise 被 reject。
- notification 和 response 交错到达时不会丢消息。

### 4.4 LSP-04：Content-Length 计算错误

#### 定位

- `src/services/tauri/lsp.ts:401-404`

当前使用：

```ts
return `Content-Length: ${body.length}\r\n\r\n${body}`;
```

JavaScript `string.length` 是 UTF-16 code unit 数，不是 LSP Content-Length 要求的 UTF-8 byte length。中文、emoji、部分组合字符都会产生错误长度。

#### 修复方案

```ts
const byteLength = new TextEncoder().encode(body).byteLength;
return `Content-Length: ${byteLength}\r\n\r\n${body}`;
```

同时确认 WebSocket bridge 是否真的需要 LSP stdio framing。如果 bridge 的 WebSocket 每个 frame 已经是一条 JSON-RPC 消息，优先直接发送 JSON，避免重复 framing。

#### 测试

- body 含中文、emoji、CRLF、代理对。
- 一个 WebSocket frame 内包含一个或多个 LSP frame。
- 分片 frame 能够被 bridge 正确重组；如果协议不支持分片，应在双方明确约束。

### 4.5 LSP 其余正确性问题

#### Diagnostics 没有按 URI 隔离

`LspClient` 只有一个 `diagnostics: LspDiagnostic[]` 和一个 callback。多文档共享 client 后必须改为：

```ts
Map<DocumentUri, LspDiagnostic[]>
Map<DocumentUri, Set<DiagnosticsListener>>
```

`publishDiagnostics.params.uri` 必须参与路由。

#### 文档版本使用 `Date.now()`

`src/services/tauri/lsp.ts:248-264` 使用时间戳作为 version。虽然通常递增，但语义上应为文档级整数计数器。快速调用、系统时间调整或恢复快照都可能破坏顺序假设。

#### Completion fallback range 错误

`src/services/tauri/lsp.ts:543-550` 在 LSP item 没有 `textEdit` 时给出固定的 `(1,1)-(1,1)` range。这会让补全文本插入文件首部，或被 Monaco 判定为无效。

正确做法：

- provider 层使用当前 `position` 和当前 word range 补齐 range；或
- converter 接收 `model`、`position`，通过 `model.getWordUntilPosition(position)` 计算替换范围。

#### Location URI 手工伪造

`src/services/tauri/lsp.ts:556-575` 手工构造一个假 `Monaco.Uri` 对象。应在 provider 具备 Monaco namespace 的位置使用 `monaco.Uri.parse(loc.uri)`，并正确处理 percent encoding、Windows drive 和 UNC path。

## 5. Hint、AI Completion 与 Inline Edit

### 5.1 HINT-01：AI completion 不能真正取消后端请求

#### 定位

- 前端 provider：`src/workspaces/codez/CodeEditor.tsx:432-480`
- 后端请求：`src-tauri/src/commands/edit.rs:117-206`

前端先等待 350ms，并检查 Monaco token；但 `aiInlineCompletion()` 一旦通过 Tauri invoke 发出，token 无法取消 Rust 内部的 LLM HTTP 请求。连续输入会产生已过时但仍在执行的请求。

#### 影响

- 模型调用成本和网络请求增加。
- 旧请求占用连接、内存和 provider rate limit。
- 关闭标签或切换项目后请求仍可能继续。
- 高延迟模型会导致 completion queue 堵塞。

#### 修复方案

- 前端为每个 editor/model 维护 request generation。
- 新请求发起时显式取消上一个 request ID。
- Rust 维护 `request_id -> CancellationToken`。
- LLM client 接受 cancel future/AbortHandle。
- 全局或 project 级最多允许 1 个 completion in-flight；最新请求优先。
- 对短时间重复上下文增加小型 LRU cache。
- 记录 requested、displayed、accepted、cancelled、latency、tokens 指标。

#### 验收测试

- 连续输入 20 个字符，后端最多保留一个有效请求。
- 关闭标签后请求取消。
- 旧请求晚于新请求返回时，旧结果不会显示。
- provider 超时、取消和 API error 不污染 UI。

### 5.2 Completion 上下文过于机械

#### 定位

- `src-tauri/src/commands/edit.rs:180-187`

当前仅截取 prefix 最后 2,000 字符和 suffix 前 600 字符。建议逐步升级：

1. 当前函数/类 AST 范围。
2. 文件 imports 和导出 symbol。
3. LSP expected type/signature。
4. 最近编辑区域和光标邻近上下文。
5. 相关文件的轻量符号摘要，而不是整文件拼接。
6. 按 token 预算截取，而不是按 Unicode 字符数截取。

必须在设置或首次启用时明确提示：代码上下文可能被发送至用户配置的远程模型/provider。

### 5.3 EDIT-01：Inline edit reject 依赖全局 undo

#### 定位

- `src/workspaces/codez/CodeEditor.tsx:169-220`
- `src/workspaces/codez/CodeEditor.tsx:284-299`

AI proposal 通过 `executeEdits()` 直接写入 model；Reject 通过：

```ts
editor.trigger("agentz-inline-edit", "undo", null);
```

如果 preview 后发生其他编辑、扩展 format、LSP code action 或外部同步，undo 栈顶部不一定仍是 AI edit。

#### 修复方案

proposal state 应保存：

- model URI；
- apply 前的 `alternativeVersionId`；
- 原始 range；
- inverse edit；
- proposal generation ID；
- apply 后的 version。

Reject 时：

- 如果 model/version 仍匹配，则应用 inverse edit；
- 如果已经发生并发变化，则显示冲突 diff，让用户选择保留/恢复；
- 不使用全局 undo 作为业务回滚机制。

### 5.4 Inline edit 的异步一致性

生成过程中切换 tab 时，目前只检查 `inlineStateRef.current` 是否存在，没有验证 proposal 是否仍属于原始 model/path。建议响应返回时验证：

```ts
request.path === activePath
request.modelUri === editor.getModel()?.uri.toString()
request.generation === currentGeneration
```

不匹配时直接丢弃响应。

## 6. Git 版本控制

### 6.1 GIT-01：porcelain 输出解析不安全

#### 定位

- 命令：`src-tauri/src/commands/ide.rs:739-745`
- 解析：`src-tauri/src/commands/git_workspace.rs:173-208`

当前执行：

```bash
git status --porcelain=v1 -uall
```

并将 `line[3..]` 当作路径。这无法正确处理：

- 空格、引号、反斜杠及 Git quotePath 转义；
- 中文/非 ASCII 文件名；
- rename/copy 的 `old -> new`；
- 文件名中的换行符；
- 非 UTF-8 路径；
- Rust 字符边界与字节切片差异。

#### 修复方案

使用 NUL 输出并按 bytes 解析：

```bash
git -c core.quotepath=false status --porcelain=v1 -z -uall
```

建议让 `run_git_cmd` 为 Git status 提供 byte-oriented 版本：

```rust
async fn run_git_cmd_bytes(...) -> Result<Vec<u8>, String>
```

rename/copy 状态在 `-z` 模式下需要读取额外 path 字段。内部路径尽量使用 `OsString`/`PathBuf`，只在 UI 序列化边界进行可损转换并标注风险。

#### 必须增加的 fixture

- `normal.ts`
- `a b.ts`
- `中文.ts`
- `quote"name.ts`
- rename old → new
- copied 文件
- staged + worktree 同时修改
- untracked directory
- nested repository

### 6.2 GIT-02：nested repo diff 路径混用

#### 定位

- `src-tauri/src/commands/ide.rs:796-835`
- `src-tauri/src/commands/git_workspace.rs:102-147`

`resolve_git_context()` 已经返回 `root` 和 `path_in_repo`，但 diff 命令部分仍使用 workspace-relative `path`。当当前目录已经是嵌套 repo root 时，再传带 repo 前缀的 workspace path 会导致路径重复。

示例：

```text
workspace: /repo
nested repo: /repo/packages/app
workspace path: packages/app/src/main.ts
repo-relative path: src/main.ts
```

在 `/repo/packages/app` 执行 Git 命令时只能使用 `src/main.ts`。

#### 修复方案

统一定义：

```rust
struct GitContext {
    repo_root: PathBuf,
    repo_root_rel: PathBuf,
    workspace_path: PathBuf,
    repo_relative_path: PathBuf,
}
```

所有 Git 子命令只能接收 `GitContext`，禁止单独传递含义不明确的 `path: String`。

### 6.3 GIT-03：`git_root` 参数被忽略

#### 定位

典型位置：

- `src-tauri/src/commands/ide.rs:906-917`
- `src-tauri/src/commands/ide.rs:935-945`
- `src-tauri/src/commands/ide.rs:953-980`
- `src-tauri/src/commands/ide.rs:986-996`

多个 API 接收 `git_root` 后执行：

```rust
let _ = git_root;
```

这会使 UI 显式选择的仓库与后端实际解析不一致，也让 API 契约具有误导性。

#### 修复方案

- 若提供 `git_root`，先通过 `resolve_git_dir()` 验证其位于 workspace 内且为已发现 repo。
- 再验证 path 确实属于该 repo。
- 未提供时才根据 path 自动推断。
- 如果 path 与 git_root 冲突，返回明确错误，不要静默选择另一个 repo。

### 6.4 Git discard 和文件系统安全

`GitPanel` 已在 `src/workspaces/codez/GitPanel.tsx:79-109` 对 discard 提示确认，这是正确方向。但仍建议：

- 使用现有 `confirmDialog()` 原生 Tauri dialog，避免 WebView `window.confirm` 不可靠问题。
- untracked 文件删除优先进入系统回收站或项目 recovery area。
- discard 前创建可恢复 journal/checkpoint。
- discard all 不要吞掉每个文件的错误；应汇总成功、失败和未处理路径。
- 所有 destructive operation 返回结构化结果，不返回拼接字符串。

### 6.5 Git 产品能力缺口

为达到成熟 IDE 体验，后续建议补充：

- staged diff 与 worktree diff 明确分离；
- hunk/line stage 和 unstage；
- merge conflict 三方视图；
- detached HEAD、remote branch、ahead/behind；
- fetch/pull/push；
- stash；
- amend、sign-off、签名；
- askpass/credential helper 集成；
- Git operation queue，避免并发修改 index；
- operation progress、cancel 和审计日志。

## 7. Agent Harness

### 7.1 AGENT-01：执行上下文绕过权限

#### 定位

- delegated/辅助路径：`src-tauri/src/commands/chat_turn.rs:1243`
- 主 Agent turn：`src-tauri/src/commands/chat_turn.rs:2084`

两处均设置：

```rust
bypass_permissions: true
```

外层虽然创建了 `PolicyGate`，但 ToolContext 又显式绕过权限，使最终权限语义不透明。主 registry 包含文件写入、shell、process、浏览器、MCP、App self-management、用户自定义 executable tools 等能力，不能只依赖 system prompt 中“先确认”的软约束。

#### 修复方案

1. 主 Agent 默认设为 `false`。
2. 仅允许内部、只读、已证明安全的任务使用 bypass。
3. 权限判断必须由 harness 强制执行，不依赖模型自觉。
4. 对以下能力分别建策略：
   - workspace read；
   - workspace write；
   - outside-workspace read/write；
   - process execution；
   - network；
   - credentials/secrets；
   - destructive operation；
   - external side effect，例如发送消息、提交表单。
5. 每次策略判定记录：tool、参数摘要、能力、匹配规则、decision、用户确认结果。
6. 子 Agent 只读 registry 是正确方向，但应再加 capability token，形成双重约束。

### 7.2 Tool allowlist 依赖字符串，容易漂移

`chat_turn.rs:930-944` 和 tool registry 通过工具名字符串启用/禁用能力。新工具添加、重命名或 alias 时可能漏进安全分类。

建议每个工具暴露结构化 metadata：

```rust
struct ToolCapabilities {
    reads_workspace: bool,
    writes_workspace: bool,
    executes_process: bool,
    uses_network: bool,
    accesses_credentials: bool,
    external_side_effect: bool,
    destructive: bool,
    reversible: bool,
    workspace_scope: Scope,
}
```

PolicyGate 基于 metadata 和参数进行判定。CI 增加测试：任何注册工具如果没有 capability metadata，则构建失败。

### 7.3 Tool transaction 和恢复能力不足

当前有 FileJournal，这是很好的基础。但一个 Agent turn 中多个修改并不天然构成事务：第三个文件写失败时，前两个可能已经落盘。

建议：

- turn 开始创建 journal transaction ID；
- file tool 记录 before image、after image 和 hash；
- turn 失败时 UI 提供“一键回滚本轮”；
- 自动回滚只能在文件未被外部再次修改时执行；
- 使用 content hash 检测冲突；
- process/network side effect 标记为不可逆，不能伪装成可回滚。

### 7.4 Cancellation 和 timeout

`chat_turn.rs:2171-2191` 在 timeout 后设置 cancel flag，再等待 collector。需要验证每个工具和 LLM client 都会及时观察 cancel；否则 UI 可能显示超时但后台任务仍占用资源。

建议：

- 所有 Tool trait 实现接受统一 `CancellationToken`；
- 外部进程进入独立 process group，取消时终止整个 group；
- HTTP 请求绑定 cancellation；
- collector 在 producer 异常退出时有独立 deadline；
- timeout 后区分 `cancel_requested`、`cancelled`、`force_terminated`；
- 增加故意不响应 cancel 的 fake tool 测试。

### 7.5 Agent Harness 推荐拆分

`chat_turn.rs` 同时负责请求规范化、附件、slash command、skills、MCP、工具注册、上下文、模型配置、运行、事件收集和持久化。建议拆为：

```text
TurnRequestNormalizer
AttachmentResolver
ContextAssembler
CapabilityResolver
ToolRegistryFactory
HarnessFactory
TurnRunner
TurnEventCollector
TurnPersistence
TurnRecovery
```

拆分原则：

- Tool registry 构建不依赖 UI 事件细节。
- Context assembly 是纯函数，可 fixture 测试。
- Persistence 与 streaming collector 分离。
- Policy decision 可独立单测。
- 主 Agent、delegate、Fish、Koi 使用相同 capability 模型，仅配置不同。

## 8. 安全

### 8.1 SEC-01：Mermaid loose mode 与 CSP 关闭

#### 定位

- `src/workspaces/codez/Markdown.tsx:49-60`
- `src/workspaces/codez/Markdown.tsx:92-124`
- `src-tauri/tauri.conf.json:25-27`

当前 Mermaid：

```ts
mermaid.initialize({
  startOnLoad: false,
  theme: "dark",
  securityLevel: "loose",
});
```

生成 SVG 通过 `innerHTML` 插入 DOM；Tauri CSP 同时为 `null`。Markdown 普通 HTML 路径已经使用 rehype-sanitize，这是优点，但 Mermaid 生成 SVG 走独立路径。

内容来源包括模型输出、工具结果、仓库文档或扩展，不能全部视为可信。

#### 修复方案

- Mermaid 改为 `securityLevel: "strict"`。
- SVG 插入 DOM 前使用 SVG-aware sanitizer。
- 禁止 event handler attribute、`javascript:`、`foreignObject`、外部 resource 和不必要的 style/url。
- 为 Tauri 配置明确 CSP，至少限制 script、connect、img、style 和 frame source。
- 逐项列出 LSP WebSocket、模型 API、自带资源需要的 connect-src，不使用全开放。
- Tauri commands 仍必须验证路径和权限，不能将 WebView 当成可信边界。

#### 测试

- 恶意 Mermaid label/link。
- SVG event attributes。
- `javascript:`/data URL。
- raw HTML、iframe、foreignObject。
- 模型输出恶意 Markdown 链接。

### 8.2 Markdown sanitizer 注意事项

`sanitizeSchema` 允许 span style 和全局 className/id。应确认：

- style 仅用于 KaTeX 的必要属性；
- 不允许 `url()`、position overlay 等可疑 CSS；
- id 不会与应用 DOM 产生 clobbering；
- link click 经过协议 allowlist，并统一加 `rel="noopener noreferrer"`；
- 本地绝对路径链接需要经过 workspace scope 检查。

## 9. 性能与资源管理

### 9.1 PERF-01：构建产物体积

本次 production build 关键结果：

| 产物 | 大小 | gzip |
| --- | ---: | ---: |
| 主应用 `index-*.js` | 约 2,011.65 KB | 约 561.29 KB |
| Monaco `monaco-*.js` | 约 3,830.98 KB | 约 992.59 KB |
| TypeScript worker | 约 7,020.26 KB | worker 未显示 gzip |
| CSS worker | 约 1,031.26 KB | worker 未显示 gzip |
| HTML worker | 约 690.01 KB | worker 未显示 gzip |
| Markdown chunk | 约 335.50 KB | 约 101.86 KB |
| Cytoscape | 约 442.57 KB | 约 141.98 KB |
| Wardley | 约 615.35 KB | 约 148.57 KB |

构建转换 4,106 个模块，本机耗时约 2 分 29 秒。

### 9.2 Monaco 加载策略

#### 定位

- `src/monaco-setup.ts:5-46`
- `vite.config.ts:20-29`

`monaco-setup.ts` 顶层导入完整 Monaco 和 JSON/CSS/HTML/TS workers。建议：

- IDE workspace 首次打开前不加载 Monaco。
- 按语言动态加载 contribution 和 worker。
- 未打开 JS/TS 文件时不启动 TS worker。
- 只读 diff/preview 场景评估是否需要完整 language service。
- Extension UI、WorkZ、设置页不应因为共享入口而加载 Monaco。
- 使用 route/workspace-level lazy boundary。

### 9.3 Mermaid 动态拆包失效

Vite 报告：Mermaid 在 `Markdown.tsx` 动态导入，但在 `MarkdownPreview.tsx` 静态导入，因此不会真正进入独立动态 chunk。

修复：两个入口都通过同一个 `loadMermaid()` 模块动态加载，删除任何顶层 Mermaid import。

### 9.4 主包拆分

当前 `vite.config.ts` 只有 app-vendor、monaco、markdown 三个粗粒度 manual chunks。建议 lazy boundary：

- CodeZ workspace；
- WorkZ workspace；
- Settings；
- Extension Manager；
- Browser Panel；
- Workflow Designer；
- Graph/Cytoscape；
- Markdown/Mermaid/KaTeX；
- Image Preview；
- Git panel 高级功能。

不要仅靠 manualChunks 强拆第三方包，应优先从 React 页面/功能入口动态 import，确保未访问功能不下载、不解析、不执行。

### 9.5 文件 watcher 与索引

现有优点：

- watcher 对 ignored path 做过滤；
- index worker 有事件合并和有界队列测试；
- agent busy 时减少 UI refresh；
- graph rebuild/patch 有异步调度。

建议补充指标：

- watcher raw events、ignored、coalesced；
- 单批次 path 数和 rebuild 次数；
- index latency 和失败率；
- Git refresh 次数与耗时；
- 大仓库打开时初次扫描耗时；
- watcher overflow 和 fallback rebuild。

### 9.6 性能预算

CI 建议加入：

- 首屏主包 gzip budget；
- CodeZ 首次加载资源 budget；
- Monaco ready 时间；
- LSP ready 时间；
- 首次按键响应；
- AI completion TTFT/P95；
- 大文件打开和编辑延迟；
- 10k/100k 文件 workspace watcher/index benchmark；
- 稳态内存和标签切换后的泄漏检查。

## 10. Extension Host

现状优点：

- Extension Host 独立构建，避免扩展直接混入 React UI 主线程。
- 有 RPC、workspace、SCM、language features、tasks、testing、debug、tree view、webview 等抽象。
- 输出 retention 已有边界测试。

风险和建议：

- 当前主要验证是 build/smoke，缺少 VS Code API compatibility contract tests。
- 为每个已实现 API 建 capability/compatibility matrix。
- 扩展崩溃、无限输出、死循环、超时 RPC、重复 activate/deactivate 需要隔离测试。
- Extension Host 应有内存/CPU/消息大小/调用频率限制。
- Webview、shell、filesystem、network 权限应按扩展声明授予。
- RPC request 必须支持 timeout、cancel、structured error 和 host restart。
- Extension Host 重启后要恢复已安装扩展状态，但不能重复注册 command/provider。
- 快速切换 workspace 时验证旧 host 不再操作新 workspace。

## 11. React 和 UI 状态问题

ESLint 报告 61 个 warnings，其中以下 Hook warning 具有行为风险，不应仅当作格式问题：

- `src/extensions/ui/QuickInput.tsx:17-20`：effect 缺少 `inputBox?.value`，同一 input ID 更新默认值时 UI 不同步。
- `src/workspaces/codez/AssistantPanel.tsx:530`：callback 缺少 `flushStreamDelta`，可能捕获过期流式刷新逻辑。
- `src/workspaces/codez/AssistantPanel.tsx:784-802`：callbacks 缺少 `onModeChange`，可能使用过期翻译/状态。
- `src/workspaces/codez/CodeEditor.tsx:538`：handleMount 缺少 `tab.content`、`tab.isReadOnly`，进一步放大 LSP 和 breakpoint 旧闭包问题。
- `src/workspaces/codez/FileTree.tsx:255`：缺少 `node.is_dir`。
- `src/workspaces/codez/Terminal.tsx:159`：effect 缺少 `active`。
- `src/workspaces/codez/index.tsx:595`：cleanup 直接读取可能已变化的 ref；应在 effect 内捕获对应集合/状态。
- `src/workspaces/codez/settings/WfBranchExprPaths.tsx:29`：memo 缺少 `nodeLabel`。
- `src/workspaces/workz/index.tsx:355,821`：缺少 `setPreviewPath`。

处理方式：

- 不建议批量 `eslint --fix` 后结束。
- 对每个 warning 判断 callback 是否需要 ref、useEvent 风格封装或真实依赖。
- 对涉及订阅/异步请求的 effect 增加 race/cancellation 测试。
- CI 可逐步设置 warning budget，最终将 `react-hooks/exhaustive-deps` 提升为 error。

## 12. CSS-01：生产构建 CSS 语法警告

#### 定位

- `src/extensions/ui/extensions.css:1-2`

注释文本中包含：

```css
(--bg-*/--text-*/--border/--accent)
```

其中 `*/` 会提前关闭 CSS 注释，Vite/esbuild 报：

```text
Unexpected "*" [css-syntax-error]
```

#### 修复

改写为不包含注释终止符的文字，例如：

```css
/* Uses the --bg-, --text-, --border-, and --accent token families. */
```

将 production build warning 设为 CI failure，避免真正的语法错误长期被忽略。

## 13. 测试策略

### 13.1 当前覆盖优势

Rust 已覆盖：

- workflow validation/runtime；
- gateway 消息解析；
- graph/index；
- path filtering；
- codebase index worker；
- terminal log bounds；
- skills provenance；
- 部分 interactive UI schema；
- nested repo discovery 的基础情况。

### 13.2 当前关键缺口

前端仅 5 个测试文件、21 个测试，未充分覆盖：

- Monaco 多标签页生命周期；
- LSP client 和 WebSocket framing；
- provider URI 匹配；
- diagnostics 路由；
- Git porcelain 特殊路径；
- nested repo Git 操作；
- GitPanel destructive flows；
- AI completion cancellation；
- inline edit 冲突和回滚；
- AssistantPanel streaming event ordering；
- Agent permission matrix；
- prompt injection；
- Markdown/Mermaid hostile input；
- Extension Host API contracts。

### 13.3 必须新增的测试套件

#### LSP fake server integration tests

- initialize 立即响应；
- delayed response；
- response/error/notification 交错；
- non-ASCII payload；
- socket close；
- multiple documents；
- diagnostics per URI；
- reconnect 和 server crash。

#### Monaco integration tests

- 两个标签 URI 唯一；
- A → B → A provider 仍有效；
- marker 不串文件；
- close/reopen 不泄漏 provider；
- external disk update 与 dirty buffer 冲突。

#### Git fixture tests

- 空格、中文、引号、rename、copy、删除、untracked；
- staged + unstaged 双状态；
- nested repo status/diff/add/reset/discard；
- 显式 git_root 和 path 冲突；
- discard failure 汇总；
- unborn branch/首次 commit；
- detached HEAD。

#### Agent policy tests

- 每个注册 tool 必须有 capability metadata；
- balanced/strict/auto 等 profile 的 decision matrix；
- outside-workspace；
- shell/network/credentials/destructive；
- 用户拒绝确认；
- sub-agent 无法取得 write/exec capability；
- MCP tool capability 映射缺失时默认 deny。

#### Cancellation tests

- LLM request cancel；
- shell process tree cancel；
- hung tool timeout；
- collector close；
- UI cancel 后无后续 token/tool event；
- timeout 后 session 状态正确恢复。

#### Security tests

- 恶意 Markdown/HTML/SVG/Mermaid；
- link protocol；
- WebView → Tauri invoke scope；
- extension webview 消息；
- user tool manifest path traversal；
- prompt injection 试图启用禁用工具。

## 14. 推荐重构边界

### 14.1 `CodeEditor.tsx`

拆分为：

```text
CodeEditorView
useMonacoModel
useLspDocument
useAiInlineCompletion
useInlineEditPreview
useBreakpointGutter
useEditorCommands
```

### 14.2 `src/services/tauri/lsp.ts`

拆分为：

```text
lsp/protocol.ts       JSON-RPC 类型与 framing
lsp/client.ts         WebSocket、pending、cancel、reconnect
lsp/documents.ts      document lifecycle/version
lsp/converters.ts     LSP ↔ Monaco 转换
lsp/providers.ts      Monaco provider registration
lsp/sessionPool.ts    project/language 复用
```

### 14.3 `src-tauri/src/commands/ide.rs`

拆分为：

```text
commands/ide/files.rs
commands/ide/git.rs
commands/ide/terminal.rs
commands/ide/watcher.rs
commands/ide/lsp.rs
```

### 14.4 前端 workspace

`codez/index.tsx` 和 `AssistantPanel.tsx` 建议将 watcher、tab model、session stream、Git refresh scheduler 分离为独立 store/hook，避免一个组件同时承担持久化、订阅、调度和渲染。

## 15. 分阶段实施计划

### 阶段一：正确性与安全，建议立即处理

1. 修复 Monaco model URI，删除 diagnostics 的首 model fallback。
2. 将 LSP 生命周期从 onMount 中拆出并支持多文档。
3. 修复 initialize race、UTF-8 Content-Length、pending rejection。
4. 主 Agent 关闭 `bypass_permissions`，建立最小权限回归测试。
5. Mermaid 使用 strict mode，启用 CSP，并 sanitize SVG。
6. Git status 改用 `-z` 字节解析。
7. 修复 nested repo diff/path 和被忽略的 git_root。
8. 修复 `extensions.css` 注释语法。

阶段一验收门槛：

- 所有新增回归测试通过；
- production build 无 warning；
- 两文件、多语言、多 repo 手工 smoke 通过；
- Agent 写文件/执行命令按策略触发确认或拒绝；
- 恶意 Mermaid/Markdown 测试无法执行脚本或越权 invoke。

### 阶段二：核心体验稳定化

1. project/language LSP session pool。
2. AI completion 真取消、并发限制、generation guard。
3. Inline edit inverse-edit 回滚与冲突处理。
4. Git operation queue、staged/worktree diff、结构化错误。
5. 拆分 `chat_turn.rs`、`CodeEditor.tsx`、CodeZ workspace。
6. 核心集成测试加入 CI。
7. React Hook warnings 清零。

### 阶段三：性能和产品化

1. WorkZ/CodeZ/Settings/Extensions/Graph/Browser 路由级 lazy loading。
2. Monaco contribution 和 worker 按语言加载。
3. Mermaid 单一路径动态加载。
4. Bundle、启动、LSP、completion 性能预算进入 CI。
5. Extension Host compatibility suite 和资源限制。
6. Tool capability metadata、事务回滚和完整审计日志。

## 16. 建议的完成定义（Definition of Done）

每个核心修复 PR 至少满足：

- 提供对应自动化回归测试，不只手工验证。
- 描述对 Windows、macOS、Linux 路径和进程行为的影响。
- 不引入新的 lint/type/build warning。
- destructive 或 external-side-effect 行为具有明确权限决策。
- 异步逻辑包含 cancellation、timeout 和 stale response 处理。
- 对性能敏感的改动提供前后指标。
- 更新相关架构文档或代码注释。
- 验证多标签、多 repo、大文件和项目切换场景。

## 17. 最终结论

AgentZ 的核心优势是已经具备真正 AI IDE 所需的大部分基础构件，而不是单纯在编辑器旁添加聊天框。当前最主要的问题不是功能缺失，而是部分关键链路的生命周期、协议正确性和权限模型尚未完全闭合。

最高收益的工作顺序是：先修 LSP URI/生命周期/协议，再修 Agent 权限和 Git 路径解析，同时收紧 Mermaid/CSP；随后补齐集成测试和 cancellation；最后进行模块拆分和加载性能优化。完成阶段一后，编辑器智能能力、Git 多仓库可靠性和 Agent 安全性都会获得显著提升，也能为后续功能扩展建立稳定基线。
