# AgentZ 系统评估报告：专业 IDE 视角 × Agent Harness 完备度

| 项 | 内容 |
|---|---|
| 评估日期 | 2026-10-19 |
| 评估对象 | AgentZ 桌面端 `0.9.19`（`main`，`1b17637 release: 0.9.19 symbol graph, DeepWiki, and anchored repo memory`）+ 固定内核 `piscis-engine v0.8.68`（`vendor/piscis-engine` 审计覆盖/patch） |
| 评估立场 | 专业 IDE 开发者（编辑器/语言服务/调试/扩展/发行） + Agent Harness 工程（执行循环/工具/上下文/权限/恢复/评测） |
| 证据来源 | 源码盘点（`src/`、`src-tauri/`、`crates/agentz-task-state`、`crates/agentz-host`、`extension-host/`、`vendor/piscis-engine/`）、`docs/upgrade/plan.json|tasks.md|progress.md` 83 项台账、`docs/upgrade/reassessment-2026-10-13.md`、`docs/agentz-product-maturity-review-2026-10-04.md`、`docs/ai-ide-engineering-audit.md`、`docs/upgrade/acceptance-0.9.12.md`（46 项未验收）、0.9.13–0.9.19 发布说明、CI 配置 |
| 证据边界 | **本报告为静态审查 + 台账证据复核，不是新的实机验收。** 本轮未重跑完整 `cargo test --workspace` / 生产构建 / 真实模型长任务 / 原生 GUI 旅程 / macOS·Linux 真机 / 签名安装器。凡台账标注 `current_source_read_only_no_tests_rerun`、`native_unverified`、`unreviewed candidate` 者，本报告一律不提升为“已验证”。结论区分三级：**已实现（源码存在）≠ 已自测（单测/集成通过）≠ 已验收（原生/真机/真实模型证据）**。 |

---

## 0. 执行摘要（一页结论）

### 0.1 总判断

> **AgentZ 已越过“聊天框 + 工具调用”的原型阶段，形成了一个功能覆盖罕见地宽的 AI 原生桌面工作台雏形；但以专业 IDE 的交付标准衡量，它仍是“开发者预览 / 受控内测”级产品，而不是可放心托付长任务、可对外宣称稳定支持的 1.0 级 IDE。**

分水岭不在功能数量，而在**契约的闭合程度**：任务↔环境绑定、变更评审完整性、权限实际执行、人机交接、崩溃恢复、跨平台与发行——这些“最后一公里”的契约，恰是台账中大量 `preparing / review / blocked / deferred` 项集中的地方（83 项中仅 37 项 `done`，且多项 `done` 在 2026-10-13 复评中被限定为 `bounded_source_review`，需在当前源码上重跑）。

### 0.2 双维评分（10 分制，IDE 工程惯例：7 分=可日常自用，8.5 分=可团队推广，9+ =成熟商业 IDE）

| 维度 | 分数 | 一句话依据 |
|---|---:|---|
| **IDE 壳与编辑器**（Monaco/文件/标签/布局） | 6.5 | 壳完整、双工作台（CodeZ/WorkZ）成立；单 WebView 渲染、巨型组件、脏缓冲冲突未闭合（AZ-041）拉低日常可靠性 |
| **语言智能**（LSP/补全/导航/符号图） | 6.0 | LSP 桥 + 崩溃恢复（AZ-042 done）+ tree-sitter 符号图/CodeMap 是亮点；真实语言服务旅程、增量同步、扩展语言生态未验收 |
| **调试与终端**（DAP/PTY） | 5.5 | DAP broker、PTY 生命周期代码齐备；AZ-043/AZ-044 均 `preparing`，无原生录屏/多终端串扰实测 |
| **Git 与变更评审** | 7.0 | M1 已修 F02/F03 类缺陷：完整 ChangeSet（committed/staged/unstaged/untracked）、journal 按环境路由、归档快照与合并预检（AZ-011~016 done）；但评审 UI 原生核对仍靠豁免 |
| **扩展生态**（VSIX/Extension Host/Webview） | 5.0 | Node 扩展宿主 + VSIX 贡献点摄入在架构上成立且罕见；Webview 隔离 AZ-032 仍 `review`、6 个真实扩展兼容档案 AZ-046 `deferred`——“能装”≠“兼容” |
| **远程与浏览器/桌面自动化** | 4.5 | SSH/VFS/端口转发/远程 LSP、CDP 浏览器面板均有实现；AZ-045 远程断连 `blocked`（缺真实 SSH）、AZ-053 浏览器抢占 `pending`、AZ-054 桌面操作 `not_started` |
| **Agent Harness 执行循环** | 7.5 | 本系统最强项：场景化 Harness（main/koi/fish/scheduler/debug/headless）、分层预算与三级压缩、重试分类、流式、计划/记忆/钩子可插拔，设计完备度接近成熟 Agent 框架 |
| **工具系统** | 7.0 | 内核中立工具 20+、IDE 耦合工具、MCP、用户工具、连接器齐全；但工具面过大、部分工具（email/ssh/elevate 等高危）与权限执行的对齐靠 AZ-028/034 逐项兜底 |
| **上下文工程** | 7.0 | LayeredPrompt、StateFrame、滚动摘要、micro/auto/full 三级压缩、图谱/RepoWiki/锚定记忆注入，思路现代；缺公开的长会话退化曲线与评测（AZ-062 pending） |
| **权限与安全** | 6.0 | 四级权限契约、持久审批意图、Windows AppContainer 参考策略、DPAPI、加密失败关闭（AZ-027~030、034 done）是真投入；秘密全出口脱敏 AZ-031 `blocked`、macOS/Linux 凭据 AZ-077/078 `blocked`、Webview 隔离未实测，构成 P1 风险群 |
| **持久化、恢复与预算** | 7.0 | Run/Attempt 状态机、耐久事件+断流游标、启动对账、取消闭环、任务级预算预留（AZ-017~024 done）超出多数同类产品；但 AZ-026 内核契约版本仍 `preparing`，预算未覆盖全部模型入口（标题/记忆/curator 等旁路） |
| **多 Agent 与工作流** | 5.0 | Pool/Koi/Fish/Team/Workflow 概念完整甚至超前；成员环境隔离 AZ-070、团队预算 AZ-071 均 `deferred/partial`，多 Agent 收益 AZ-075 无对照数据——**当前是“有多 Agent 的 UI 与调度”，没有“多 Agent 更可靠”的证据** |
| **可观测与评测** | 5.0 | 事件溯源、诊断导出候选、AZ-004 的 12 任务×3 真实内核基线（自动 33/36、语义复核 36/36）是好底子；趋势门禁 AZ-062、诊断关联 AZ-061 均 `pending`，无桌面端真实任务成功率的持续度量 |
| **性能与资源** | 4.5 | 有 `path_filter`、刷新合并、perfCounters 等点状优化；主 chunk 曾达 ~5.95MB（未压缩构建）、AZ-059 首屏/AZ-060 长任务资源均 `blocked/pending`，无冷启动与 2 小时 soak 基线 |
| **测试与质量门禁** | 5.5 | 前端 ~45 文件/310 测试（有界并发下，2026-10-13）、任务状态域 152+2 测试、内核 350+ 测试均可观；但核心生命周期 E2E 门禁 AZ-056 `blocked`、CI 仅 Ubuntu、默认并发 Vitest 曾 worker 退出——**门禁存在，门禁未合上** |
| **发行与跨平台** | 3.5 | 全场最低：未签名未公证、无可信更新（AZ-065 `deferred`）、数据迁移/备份恢复 AZ-063 `pending`、macOS/Linux 从未真机验收（AZ-058 `blocked`）。当前分发实质是“Windows 未签名安装包 + 免责声明” |
| **产品信息架构（CodeZ/WorkZ 双台）** | 7.0 | “人操作的工作台 / 人委派的工作台”双视图 + 同任务交接，是区别于 Cursor 类单视图产品的真正差异化；但交接本身 AZ-036 `preparing`、AZ-037 `deferred`，卖点尚未兑现为闭环 |

**综合：IDE 完备度 ≈ 5.9/10，Agent Harness 完备度 ≈ 6.7/10，产品可交付度（面向外部用户）≈ 4.5/10。** 三者差距本身就是诊断：**内核与 Harness 的设计跑在产品验收前面约一个大版本，发行工程跑在后面约两个大版本。**

### 0.3 给决策者的三句话

1. **不要再加新功能面。** 浏览器、桌面操作、团队、网关渠道（飞书/微信/Slack 等 8+ 渠道）、市场、技能进化等面已经铺得比验收能力宽得多；下一个周期的边际收益全部在“合上契约”。
2. **1.0 的定义应是验收事件，不是版本号。** 0.9.7→0.9.19 两周内发了 13 个版本、且 0.9.12 起连续多版自带“未通过上线验收门禁”声明——发版节奏与验收节奏已经脱节。建议冻结功能，以 AZ-056（E2E 门禁）+ AZ-057（Windows 长稳）+ AZ-063/064（迁移/签名）为 1.0 闸门。
3. **最值钱的差异化是“可托付的长任务”，不是“更多的 Agent”。** 耐久运行（AZ-017~025）已是同类中少见的深度，应把单 Agent 可靠闭环打磨成招牌，再谈多 Agent 收益（AZ-075）。

---

## 1. 系统全景（评估对象界定）

### 1.1 架构

```
┌──────────────────────────── 单 Tauri WebView (React 18 + TS) ───────────────────────────┐
│  App.tsx (47KB, 全局壳)                                                                  │
│  ├─ CodeZ 工作台: Monaco 编辑器 / 文件树 / Git / 搜索 / 终端(xterm) / LSP 客户端 / DAP   │
│  │   调试视图 / 浏览器面板 / 扩展 UI(Webview) / CodeMap(reactflow+elk) / 设置 / 工作流设计 │
│  └─ WorkZ 工作台: 会话任务 / Pool 团队看板 / 工作流运行 / 产物抽屉 / 交付评审              │
│  服务层: src/services/tauri/* (72 文件, 按域拆分的 invoke 封装 + 纯函数单测)               │
└──────────────────────────────────────┬───────────────────────────────────────────────────┘
                                       │ Tauri invoke (~300 命令, lib.rs generate_handler)
┌──────────────────────────────────────▼───────────────────────────────────────────────────┐
│  Rust 宿主 (src-tauri, Tauri 2)                                                            │
│  ├─ commands/ (68 文件): chat/chat_turn(3.2k 行)/agent_task/ide/session/workflow/pool…     │
│  ├─ runtime/: durable_run / event_replay / approval / owner_lease / task_budget / koi …    │
│  ├─ crates/agentz-task-state: Run/Attempt 状态机·预算预留·审批·产物·归档·外部动作幂等 (SQLite)│
│  ├─ 基础设施: LSP manager / DAP broker / PTY(portable-pty) / watcher(notify+path_filter)  │
│  │   远程(SSH/VFS/转发/devcontainer) / 浏览器(chromiumoxide CDP) / gateway(8+ IM 渠道)      │
│  └─ tools/: IDE 耦合工具 ~18 (LSP/图谱/符号/仓库记忆/chat_ui/plan UI/delegate…)             │
└──────────────────────────────────────┬───────────────────────────────────────────────────┘
        ┌──────────────────────────────┼──────────────────────────────┐
        ▼                              ▼                              ▼
┌─ piscis-engine v0.8.68 ─┐  ┌─ Node Extension Host ─┐   ┌─ 外部面 ─────────────┐
│ (vendor 固定+审计 patch) │  │ VS Code API 兼容层     │   │ MCP servers / 连接器  │
│ piscis-kernel: AgentLoop │  │ (apiFactory/mainThreads│   │ (OAuth) / ClawHub /   │
│ (loop_.rs ~313KB!)       │  │ /Webview/Debug/Notebook│   │ 云市场 / RobotZ 浏览器 │
│ Harness 配置·工具注册表   │  │ stdio JSON-RPC)        │   │ (git 固定 v0.1.13)    │
│ PolicyGate·LLM 客户端    │  └────────────────────────┘   └───────────────────────┘
│ 中立工具 20+·Pool 协调器  │
└──────────────────────────┘
```

### 1.2 规模感（盘点口径，非精确 LOC 审计）

- 前端：`src/` 数百文件，含 ~45 个 Vitest 测试文件；`App.tsx` 单文件 47KB、`chat_turn.rs` 3,249 行、内核 `loop_.rs` 约 313KB——**三处“上帝文件”是可维护性的主要税源**（见 §6.4）。
- 宿主命令面：`lib.rs` 注册约 300 个 Tauri 命令，覆盖文件/Git/终端/LSP/DAP/会话/任务/池/工作流/市场/网关/技能进化全域——命令面之宽在同类产品中罕见，也意味着**攻击面与回归面同样宽**。
- 预装资产：`bundled/preinstall/` 含大量从 CodeBuddy 导入的 agents/skills/commands——内容供给丰富，但也带来来源、版本与权限声明的治理负担（已有 `verify:preinstall / lint:preinstall --strict` 入口，是好的开端）。

### 1.3 产品定位判断

README 的 “Cursor-like AI IDE” 类比便于理解入口，但低估了自身：AgentZ 实际形态是 **IDE（CodeZ）+ 任务委派台（WorkZ）+ 多 Agent/工作流运行时 + 扩展/技能/连接器平台**的四合一。这个定位野心合理（对标 Cursor + Codex/Devin 式委派的合流趋势），但四条产品线共享同一验收带宽，是当前“处处有实现、处处差验收”的结构性原因。

---

## 2. IDE 能力评估（专业 IDE 开发者视角）

评判基准：以 VS Code / Cursor / JetBrains 家族的**日常工程可用性**为准绳——不是“有没有这个面板”，而是“在 10 万行仓库、脏工作区、崩溃恢复、多标签并发下是否可信”。

### 2.1 编辑器内核与文档模型 【6.5/10】

**已成立：** Monaco 集成（含 `@monaco-editor/react`、主题与 VS Code 主题转换、Markdown/Mermaid/KaTeX 渲染）、多标签（`EditorTabs`、tabNavigation 有单测）、编辑器热退出恢复（`editorRecovery` 有 integration test）、未保存缓冲保存、行内编辑（Cmd-K）与 AI ghost-text 补全、图像预览、Notebook（`.ipynb`）编辑器。

**未闭合（按影响排序）：**

1. **脏缓冲 × Agent/外部写入冲突（AZ-041，`preparing`）**——这是 IDE 的“宪法问题”：Agent 写文件时用户正开着未保存的同一文件，谁赢、如何提示、磁盘 base 是什么。当前仅有 `editorConflict.ts` 助手与单测，typing/save/watcher/恢复四路交错未在当前源码复跑。**在闭合前，“人与 Agent 同改一仓”的核心卖点带有数据丢失风险，应在 UI 上明示。**
2. **单 WebView 渲染架构**：编辑器、聊天流、Markdown、图布局、扩展 RPC 共用一个渲染线程（`docs/performance.md` 自认）。长会话无虚拟列表（仅 `content-visibility` 轻优化），大仓库 CodeMap 用 reactflow+elk 在主线程布局——卡顿上限由最重的那块决定。这是与 Electron 多进程（Cursor/VS Code）的**架构级差距**，靠懒加载/分 chunk 只能缓解（AZ-059）。
3. `App.tsx`、`codez/index.tsx`、`AssistantPanel.tsx` 等巨型组件使编辑器状态（标签、watcher、Git 刷新、布局）耦合在同一状态树，焦点切换反复刷新类问题（0.9.12 UT-R03）即为此类耦合的症状。

### 2.2 语言智能 【6.0/10】

**已成立：** LSP 桥（WebSocket ↔ 语言服务器）、多语言管理、诊断/补全/跳转的 Monaco provider 接线；**LSP 崩溃恢复 AZ-042 `done`**（恢复预算、重连后重置、多轮崩溃回归 10/10）是超出多数自研 IDE 的细节；tree-sitter（Rust/TS/JS/Python/Go 等）驱动的符号图、调用图、CodeMap 与 0.9.19 的 DeepWiki/Repo Memory，构成了“结构理解层”，对 Agent 的 `graph_search / symbols / codebase_search` 工具直接供血——**这是 AgentZ 相对纯 LSP 路线的一个真差异点：图谱既给人看，也给 Agent 用。**

**未闭合：** LSP `didChange` 仍发全量文档（`docs/performance.md` 自认，300ms 防抖）；2026-09 审计的 LSP-01~04 类缺陷（URI 一致性、标签切换重连、initialize 竞态、Content-Length 字节长度）需在当前版本以真实 TS/Rust 双服务器旅程的明确证据归档，而非仅靠代码阅读（AZ-082 已修 LSP 返回路径过滤，但 live LSP 另测）；语言覆盖依赖用户自装语言服务器/扩展，无“开箱即用的 TS/Python 体验”验收。

### 2.3 调试（DAP）【5.5/10】

**已成立：** 独立 DAP broker（`dap.rs`/`dap_frame.rs`）、调试控制器与断点存储（`src/extensions/debug/`，含 `dapClient.test.ts`、`debugController.test.ts`）、调试视图 UI。

**未闭合：** AZ-043 `preparing`——launch/attach、断点命中、异常退出无残留进程、文件跳转到**正确任务环境**，均无原生证据。调试是 IDE 中最吃“进程树与路径映射正确性”的功能，在 AZ-036（CodeZ 打开任务环境）未闭合前，调试的“环境正确性”无从谈起。对专业用户而言，**未验收的调试器比没有调试器更危险**（断点打错环境、进程残留）。

### 2.4 集成终端 【5.5/10】

**已成立：** portable-pty 多终端、xterm + fit/web-links 插件、终端生命周期逻辑（`terminalLifecycle.ts` 有单测）、资源快照命令（`ide_terminal_resource_snapshot`）、关闭时进程收割与内核 Job Object 树终止契约（AZ-022 在 Windows 经真实两级 cmd 树验证，是硬证据）。

**未闭合：** AZ-044 `preparing`——多终端 × 任务切换的 cwd 串扰、关闭任务后的 PTY 回收，需在当前 Job/terminal 改动上重跑原生案例。终端 cwd 与任务环境的绑定同样受制于 AZ-036。

### 2.5 Git 与变更评审 【7.0/10，IDE 侧最成熟域】

M1 里程碑（Gate 已过）确实修掉了 2026-10 审查的 F02/F03/F04/F05 群：完整 ChangeSet 采集（四类变更 + NUL 路径 + 改名/二进制/结构化错误，AZ-011）、journal 按任务环境路由（AZ-012）、归档快照（独立 index + 原始 blob + 可达 ref，AZ-014）、受约束清理（拒绝跨任务路径、分步失败可见，AZ-015）、合并事务预检（仓库互斥、冻结 HEAD 授权、运行租约，AZ-016）、评审 UI 区分变更来源与读取失败（AZ-013）。

**残余风险：** 上述 `done` 在复评中均标注 `bounded_source_review`、当前源码未逐项重跑；评审与归档的**原生 GUI 核对**由 ADR-007/008 豁免而非实测。工程判断：**后端契约可信度高，前端呈现可信度待一次原生 A/B 双任务旅程即可显著提升**——这是全系统性价比最高的一次验收投资（同时解锁 AZ-056 的一半）。

### 2.6 搜索、导航与代码理解 【7.0/10】

文件搜索/全局搜索面板、命令面板式交互、符号查找与调用图（`symbol_find / symbol_call_graph`）、CodeMap 图布局、代码库索引与语义搜索（`codebase_index_build/search`）、RepoWiki/DeepWiki 生成——面向人和 Agent 的双通道导航完整。风险点在索引新鲜度与大仓性能：索引 worker 与图构建的内存峰值、关闭回收属 AZ-060 的未验收范围；`path_filter` 的排除规则（宿主预检 × 内核遍历一致性，AZ-081 done）做得细致，值得肯定。

### 2.7 扩展系统 【5.0/10】

**架构成立且野心正确：** Node sidecar 扩展宿主（stdio JSON-RPC、`apiFactory/mainThreads` 桥、Webview/DAP/Notebook/Debug 贡献点）、VSIX 摄入与安装管理、兼容层 DTO/类型转换、扩展 UI store（有单测）。这条路线（兼容 VS Code 扩展 API 子集）是 Cursor 之后所有 AI IDE 的必修课，AgentZ 起步不晚。

**但兼容性是证据生意，不是 API 名单生意：**

- AZ-032 Webview 隔离仍 `review`：当前 CSP 已收紧为 `script-src 'self'` 且有 `agentz-resource:` 受控协议、每面板令牌与 Rust 路径白名单（代码层证据），但**真实 Tauri 中“能渲染 ∧ 不能触达宿主 DOM/IPC/其他面板”的正反实测未做**。Webview 是 IDE 的头号 RCE 面，此项未闭合前应限制第三方 Webview 扩展的分发声明。
- AZ-046 `deferred`：6 个真实扩展（语言/格式化/Git/调试类）的安装-运行-升级矩阵为零。**当前对扩展的任何“兼容”表述都属未验证声明**，支持矩阵应明确标注 `未验证` 分级（AZ-005 已定义分级，待 AZ-067 落实到发布文档）。

### 2.8 远程开发 【4.5/10】

SSH 目标管理、密钥配置、远程 FS/VFS、远程 LSP、端口转发、devcontainer、扩展同步——面铺得完整。但 AZ-045 `blocked`：断连重连、主机密钥变更拒绝、重连零重复写，无一有真实 SSH 环境证据。远程开发的故障模式（网络分区、半开连接、远端进程残留）无法靠本地代码阅读推演，**此域当前只能标“实现候选”，不能标“支持”**。

### 2.9 浏览器与桌面自动化 【4.0/10】

内置 CDP 浏览器面板（导航/截图/点选/视口/滚动）+ RobotZ `browser` 工具（git 固定 `v0.1.13`）对 Agent 做 Web 任务是实用的；但 AZ-053 指出的代际引用失效（页面导航后旧 element ref 必须明确失效）、用户抢占后 Agent 必须停手，修复仅存在于 RobotZ 兄弟仓工作区、**未固定进 AgentZ 依赖**——这是典型的“演示能跑、托付翻车”型缺口。AZ-054 Windows 桌面操作（UIA）`not_started`、AZ-055 跨任务前台资源租约 `deferred`：在焦点身份与抢占门控设计出来之前，**不建议开启任何桌面自动操作能力**，宁缺毋滥的判断（台账原文）是正确的。

### 2.10 交互、可用性与无障碍 【5.5/10】

中英双语 i18n（有语义一致测试）、键盘路径（模式切换快捷键、BottomPanel 键盘测试）、首用引导候选（AZ-040/AZ-066）、通知收件箱候选（AZ-073）均有投入。但 AZ-047 的“键盘走完核心旅程 + 错误必含下一步动作 + 中英一致”整体走查未做；AZ-040 的 5 人首用走查、AZ-066 的干净机 5 次首任务均为零记录。对 IDE 而言，**可用性不是锦上添花：审批卡、权限提示、冲突对话框的可理解性直接决定用户会不会点“全部允许”从而架空安全模型**。

---

## 3. Agent Harness 完备度评估（核心专章）

Harness 指包裹 LLM 的全部工程层：执行循环、上下文组装、工具调度、策略门禁、状态持久化、恢复与预算。**这是 AgentZ 最值得认真夸的部分，也是与“套壳聊天 IDE”的分界线。**

### 3.1 执行循环与场景体系 【7.5/10】

内核以 `HarnessConfig` 统一组装执行环境，并提供场景工厂：`for_main_chat / for_main_headless / for_koi（子 Agent）/ for_fish（无状态短任务）/ for_scheduler / for_debug`。每场景显式声明：持久化有无、确认标志、摘要/状态帧接入、计划状态、流式、视觉委托。这种“**场景即配置、能力按需接线**”的设计，避免了多数产品“一条 loop 打天下、到处 if”的腐化路径。

循环本体（`loop_.rs`）具备：SSE 流式转发、fallback 模型链、同模型瞬时重试（可配，网关场景降为 1）、取消打断退避（AZ-023 经本地 HTTP 故障矩阵验证：401/403 与配额=永久、5xx=瞬时、429=限流，退避取消有上下界断言）、工具结果回执（`tool_receipt.rs`）、计划未完成时阻止模型自行退出。可插拔点齐全：`LoopStrategy`、`CompactionStrategy`、`ContextManager`、`MemoryPlugin`、生命周期 Hooks（宿主据此挂文件 journal）。

**扣分项：** `loop_.rs` 约 313KB 单文件是内核级的维护风险——任何 harness 行为的评审、回归定位、二开 patch（当前 vendor patch 模式）都贵；且 AZ-026“发布并接入内核契约版本”仍 `preparing`，意味着**当前桌面端跑的内核版本与契约的对应关系尚未正式冻结复核**。Harness 设计 8 分、内核工程化 6.5 分，综合 7.5。

**与主流对照：** 场景化 harness + 可插拔压缩/记忆/循环策略，设计完备度不低于 Claude Code / Codex 类产品的公开可观察水平；差距在**规模化实证**（见 §3.7 评测）。

### 3.2 上下文工程 【7.0/10】

- **分层提示**：`LayeredPrompt`（L0…Lhint）替代单块 system prompt，宿主侧再拼装 Agent/计划模式/待办/技能等上下文（`system_prompt.rs`）。
- **分层预算与三级压缩**：`LayeredBudget` 按上下文窗口推导总量，三档阈值（默认 micro 60% / auto 80% / full 95%）+ 单工具结果 token 上限 + 可选摘要模型；压缩策略可插拔，另有 `compaction_eval.rs / bench_compact.rs`——**为压缩建评测与基准，这在同类产品中少见，值得单独肯定。**
- **状态帧与滚动摘要**：`StateFrameProvider`、`SummaryStore`（p6/p7 设计）让长会话以结构化状态而非纯文本滑窗续命。
- **仓库理解注入**：符号图、CodeMap、RepoWiki/锚定仓库记忆（`repo_memory` 工具、0.9.19）为 Agent 提供项目级先验，方向正确（与 Cursor 的 codebase indexing、Codex 的 repo map 同赛道）。

**未闭合：** 长会话的实际退化曲线（多少轮后任务成功率衰减）、压缩后关键约束丢失率，无持续评测（AZ-062）；上下文组装的 token 归因（哪层吃多少）未见面向用户的可视化，`ContextUsageRing` 仅是总量环。**上下文工程“机制完备、度量缺席”。**

### 3.3 工具系统 【7.0/10】

- **内核中立工具（20+）**：文件读写/编辑/ diff/搜索/列表、shell、code_run、进程控制、web_search/web_fetch、email、ssh、pdf、vision_context、memory_store/recall、plan_todo/plan_write、pool_org/pool_chat、用户 JSON 工具动态加载，全部经 `NeutralToolsConfig` 按开关注册、桌面与 CLI 共用同一注册路径——可移植性设计干净。
- **宿主耦合工具（~18）**：LSP 诊断、代码库搜索、图谱探索/搜索、符号、终端读取、仓库记忆、chat_ui 交互卡、计划模式 UI、delegate、远程 FS、技能管理、设置目录等，是 AgentZ 的差异化工具带（尤其图谱三件套与 chat_ui 结构化交互）。
- **扩展通道**：MCP 工具动态注册、连接器（OAuth 服务暴露为 MCP 工具）、用户工具、技能（Skills）与子 Agent 人格（Koi/Fish/Agent 包）。

**风险与扣分：**

1. **工具面过大且高危工具默认面宽**：`shell / code_run / process_control / elevate / ssh / email / web_*` 并存时，安全完全押注在 PolicyGate 与审批的实际执行上（见 §3.4）。工具的**最小权限按场景裁剪**（Plan 模式禁用写类工具已有实现 `PLAN_MODE_DISABLED`，是好范例）应推广到所有场景与子 Agent。
2. 工具契约版本化未冻结（AZ-026），内核工具与宿主工具的 schema 漂移靠人工评审；同轮重复编辑 Undo 等内核缺陷需走 vendor patch 流程（AZ-079 已走通一次，流程成本可见）。
3. 工具结果的**结构化程度**不一：recovery_v1 的结构化 outcome（AZ-022）应成为所有有副作用工具的统一回执标准（与 `tool_receipt.rs` 合流），目前仍是点状达标。

### 3.4 策略、权限与沙箱 【6.0/10】

**契约层（强）：** AZ-027 冻结四级授权 × 六类资源（文件/进程/网络/浏览器/桌面/连接器）的范围/期限/撤销/升级矩阵；AZ-028 持久审批意图绑定参数摘要+任务+环境+有效期，重启作废旧授权，参数变化重批（宿主集成 17 项 + 领域 18 项通过）；审批后路径替换（junction TOCTOU）再校验 AZ-080、内核遍历排除目录 AZ-081、LSP 返回路径过滤 AZ-082——**这组“审批后世界会变”的防御，说明团队真正理解 Agent 权限的难点不在弹窗、在时序。**

**执行层（中）：** Windows AppContainer 参考策略（AZ-034）实现工作区 ACL、无网络/越界拒绝、Job 超时取消收割并有自动化覆盖——但仅 Windows、仅 `code_run/shell` 接入；worktree 不宣称沙箱的表述诚实。非 Windows 无 OS 级隔离，LSP/扩展进程不在隔离内。

**缺口层（弱）：**

- AZ-031 秘密引用与全出口脱敏 `blocked`：模型/MCP 出口已有候选，但跨出口扫描（日志/诊断/导出/app_control/对话）未完成——**在完成前，“密钥不进模型上下文”只能算设计目标。**
- 2026-09 审计的 AGENT-01（ToolContext 默认绕过权限）一类问题，必须在当前版本以“逐执行入口 `require_binding` 传播清单”（AZ-008 复评原文）的形式归档证据；权限系统的可信度=最弱入口的可信度。
- 连接器 OAuth 生命周期 AZ-051 `pending`：PKCE/刷新/逐调用校验有候选，真实 provider 的过期-撤销-在途调用旅程未跑。连接器是 Agent 触达用户真实资产（邮箱/日历/代码托管）的通道，此项应按 P1 而非 P2 对待。

### 3.5 状态、事件与恢复 【7.0/10，本系统护城河】

`crates/agentz-task-state` 是全系统工程含量最高的自研件：

- **Run/Attempt 状态机**分离执行状态与交付状态，非法跳转穷举 7×7 拒绝（AZ-017，147+2 测试）；
- **耐久事件**带 task/run/attempt/environment 标识与递增序号，断流游标补齐、去重、跨任务隔离（AZ-018，149+4+6 测试）；
- **启动对账**：OS 独占锁 owner 租约 + 杀进程故障注入下僵尸 `running` 标 `interrupted`、不重放工具、无租约标 `unverifiable`（AZ-019）；
- **取消闭环**：排队/执行双路径，`cancelling→cancelled` 宽限收尾、无协作工具强制中断、真实 shell/code_run 夹具（AZ-021，五轮复核）；
- **外部动作幂等**：意图+单次 permit+丢结果不对账不盲重发（AZ-025，12 个 SQLite 夹具含双连接竞态）——**这是多数 Agent 产品完全缺席、而真实事故（重复发信/重复发布）恰恰高发的地方**；
- **计划持久化**修正续聊清除语义（AZ-020）、进程/工具恢复契约（AZ-022）、模型重试分类（AZ-023）、任务级预算预留与 fail-closed（AZ-024）。

**扣分：** AZ-025 仅领域层、无宿主 provider 适配；AZ-052 外部发送流程 `deferred`，意味着幂等契约尚未保护任何真实发送；预算未覆盖自动标题/记忆/技能复盘/curator/inline edit 等旁路模型调用（AZ-024 复评自认）——**长任务的真实成本仍可能从这些侧门漏出**；团队/池化事件不在 AZ-018 范围。

### 3.6 多 Agent 与编排 【5.0/10】

形态谱系完整：Koi（有状态子 Agent 人格）、Fish（无状态短任务）、Pool（蜂群看板/协调器）、Team 模板、无协调者确定性工作流（节点持久化+人工续跑）、Gateway 无头回合（IM 入口）。`pool_org/pool_chat` 工具 + 协调器配置（超时/worktree）表明不是纯 UI 玩具。

但按“多 Agent 是否比单 Agent 更可靠地完成任务”的工程标准：成员环境隔离 AZ-070、团队共享预算与取消扇出 AZ-071 均未落地（台账明言现有 swarm 路径共享同一目录、预算层明确拒绝多成员）——**即当前多 Agent 恰运行在单 Agent 刚修好的那些坑（串环境、预算复制）之上**；AZ-075 对照评测（单 Agent vs 只读并行 vs 实现团队）协议已备、数据为零。结论：**编排机制 6.5 分，编排可信度 3.5 分。** 在 AZ-070/071 闭合前，团队模式应默认关闭或明确标实验。

### 3.7 可观测性与评测 【5.0/10】

正面：耐久事件天然提供回放式可观测；诊断导出候选含清单与脱敏（AZ-061）；AZ-004 建立了 12 项固定任务 ×3 的真实内核基线（含耗时/token/价格估算、自动与语义复核分列）——**有基线意识的产品不多。**

缺口：基线是 2026-10 的历史测量（`historical_limited_measurement`）、仅 synthetic 内核任务、无桌面真实旅程；评测趋势门禁 AZ-062 `pending`（缺失样本不当成功等规则未接稳定 harness）；无按版本的成功率/成本/人工干预回归曲线。**没有持续评测，多模型 fallback、压缩阈值、工具改动的每一次“优化”都是不可证伪的。** 这是 Harness 从 7 分走向 8.5 分的必经门。

### 3.8 记忆与技能进化 【6.0/10，潜力项】

长期记忆插件接口、memory_store/recall 工具、技能来源与授权变更记录（AZ-033：来源/版本/SHA-256/新增工具失败关闭）、技能进化（草稿晋升/锁定/钉选/curator 周期整理/修订与用量列表）构成“越用越懂你”的叙事。风险：记忆/技能的**错误固化与权限漂移**（技能升级悄悄扩权）已有失败关闭设计，但真实使用数据下的记忆污染率、召回精度无度量；curator 等后台模型调用不在任务预算内（同 §3.5）。潜力大、证据早，维持实验定位。

---

## 4. 横切面评估

### 4.1 安全总评 【6.0/10】

纵深已成形：CSP 收紧（`default-src 'self'`、`object-src 'none'`、受控 `agentz-resource:` 协议）、Webview 面板令牌与路径白名单、秘密静态加密（ChaCha20-Poly1305）+ Windows DPAPI 主密钥保护 + 加密失败关闭（AZ-029/030）、连接器令牌 OS 保护封存（0.9.11/12）、诊断导出脱敏、AppContainer 执行隔离、审批 TOCTOU 系列防御。

**未合上的三道门（按利用可能性排序）：** ① AZ-032 Webview 真实隔离实测（第三方扩展代码的执行边界）；② AZ-031 秘密全出口扫描（密钥经日志/导出/模型上下文泄漏）；③ AZ-077/078 macOS Keychain / Linux Secret Service 真机适配（当前两平台凭据保护仅交叉编译检查）。在 ①② 闭合前，不建议开放第三方扩展与连接器的公开分发。

### 4.2 可靠性与数据安全 【6.5/10】

文件 journal + 分轮 Undo（含同轮重复编辑修复 AZ-079）、归档快照可恢复、编辑器热退出恢复、工作区关闭门禁、会话状态自愈（0.9.17 的 `SessionRunningGuard` + 列表对账，修“假 running”一类细节，说明团队在认真处理状态一致性）——数据面投入扎实。**最大未爆弹是 AZ-063：版本化数据迁移与备份恢复仍 `pending`**（离线加密备份候选，缺两个历史版本夹具、缺写入静默与一致性快照）。对一款持有用户任务史、凭据与产物的桌面产品，**没有经演练的迁移/恢复，任何一次 schema 升级都是赌局**；此项应与签名并列 1.0 闸门。

### 4.3 性能 【4.5/10】

已知事实：生产构建曾因压缩故障产出未压缩主 chunk ~5.95MB（gzip ~1.18MB）；构建需 `NODE_OPTIONS=--max-old-space-size=4096` 且 CI 注明重型前端 OOM 风险；单 WebView + Monaco/Mermaid/KaTeX/elk 同线程。已有点状治理（懒加载 CodeMap、刷新合并、path_filter、perfCounters）。AZ-059（首屏前后对照+离线检查）`blocked`、AZ-060（2 小时资源增长）`pending`、AZ-057（Windows 基准，含硬件/冻结构建/真实端点）`blocked`。**在基线建立前，一切“流畅/卡顿”的讨论都无据可依；性能是当前证据最薄的工程域之一。**

### 4.4 测试与 CI 【5.5/10】

资产：前端 ~45 文件/310 测试（有界并发口径）、Rust 任务状态域与内核数百测试、扩展宿主 smoke、脚本级门禁测试（`upgrade-release-gate.test.mjs`）、台账自身的 `validate/render` 治理。CI（Ubuntu）覆盖前端 lint/typecheck/test/build + 扩展宿主 smoke + Rust clippy（`-D warnings`）/test——**静态门禁的密度高于同规模项目平均。**

缺口：① 核心生命周期 E2E（AZ-056，隔离创建→双任务→续聊→重启→评审→归档）`blocked`，发布不被其阻断；② CI 无 Windows/macOS 桌面行为验证（打包≠验收）；③ Vitest 默认并发曾 worker 异常退出，说明测试基础设施本身有 flake；④ 0.9.12 起多版在“未通过上线验收门禁”下仍发布（owner 豁免），**门禁的权威已被豁免惯例稀释**——这是流程风险，不是技术风险，且更难修。

### 4.5 发行工程 【3.5/10】

版本治理有亮点：版本一致性检查脚本、每版 release-notes/checklist、台账驱动的发布决策、诚实标注未签名与未验收（文档诚信值得肯定）。但硬缺口整齐排列：未签名/未公证（AZ-064 `blocked`，缺证书）、无自动更新与坏签名拒绝（AZ-065 `deferred`）、`release-readiness` 未接入 `release.yml`（0.9.17 发布说明自认“否则安装包任务不会开始”）、macOS/Linux 安装包从未真机运行。**当前发行形态只适合“已知风险的自愿内测”，任何公开下载页都应同等显著地标注这一性质。**

### 4.6 工程治理与可维护性 【6.5/10】

罕见的双面：治理面是同规模项目中的优等生——83 项原子台账（依赖/验收/证据/复核人/回退齐全）、证据文件成体系、独立复核多轮 BLOCK→PASS 的记录、ADR 豁免留痕、复评不追认历史 `done` 的纪律。**这套“不自欺”机制是 AgentZ 最被低估的资产。**

代码面则相反地承压：`App.tsx` 47KB、`chat_turn.rs` 3.2k 行、内核 `loop_.rs` ~313KB、宿主 ~300 命令平铺注册；领域服务拆分（TaskService/EnvironmentService/EventStore 的建议自 2026-10-04 审查提出）仅部分落地于 `agentz-task-state`，宿主 `commands/chat*.rs` 仍是多职责聚合。另需注意：**当前工作区存在大量未跟踪/未提交文件（`git status` 显示核心源码多为 `??`）**，源码基线与发布物的对应关系依赖台账指纹而非提交历史——对审计、回滚与协作都是实质风险，应优先收敛入仓并恢复“发布=某个提交”的纪律。

---

## 5. 竞争坐标（定性，非跑分）

| 对照 | AgentZ 的相对位置 |
|---|---|
| **VS Code + Copilot 类** | 编辑器成熟度、扩展兼容证据、调试/远程的实机验证全面落后（对方以十年计）；Agent 任务模型（环境绑定/耐久运行/交付评审）反而更完整。**不应正面比 IDE 完备度，应比“委派可信度”。** |
| **Cursor** | 单视图 Agent 体验、补全（Tab）与索引的打磨深度落后；双工作台（CodeZ/WorkZ）+ 同任务人机交接 + 开放扩展宿主是差异点，但交接未闭环（AZ-036/037），差异点暂为“势能”。 |
| **Codex / Claude Code 类 CLI Agent** | 对方胜在 harness 的大规模实证与极简可靠；AgentZ 的耐久运行/审批/幂等设计在纸面同级甚至更细，但缺同等量级的真实任务成功率数据背书。**GUI 的价值必须用“评审/交接/产物”兑现，否则打不过 CLI 的可靠+廉价。** |
| **Devin 类云端 Agent** | 云端沙箱与异步托付是对方主场；AgentZ 的本地数据主权 + 本地仓库/凭据 + IDE 内接手是主场。桌面操作/浏览器托付（AZ-053/054）未成前，不宜在此维度宣传。 |

---

## 6. 风险登记（Top 10，按 期望损失 = 影响 × 可能性）

| # | 风险 | 级别 | 触发场景 | 对应台账 |
|---|---|---|---|---|
| R1 | 脏缓冲被 Agent/外部写入覆盖，用户未保存工作丢失 | **P0** | 人与 Agent 同改一文件（核心卖点场景本身） | AZ-041, AZ-037 |
| R2 | 第三方 Webview 扩展触达宿主或跨面板泄漏 | **P0**（开放分发前） | 安装带 Webview 的 VSIX | AZ-032, AZ-046 |
| R3 | 密钥经某一未扫描出口（日志/导出/对话/MCP）泄漏 | **P1** | 导出诊断、粘贴对话、连接器调用 | AZ-031, AZ-061 |
| R4 | 数据迁移失败或备份不可恢复，用户任务史/凭据损坏 | **P1** | 任一次 schema 升级 | AZ-063, AZ-065 |
| R5 | CodeZ 打开任务环境未绑定，人在错误目录编辑/调试/开终端 | **P1** | WorkZ→CodeZ 交接 | AZ-036, AZ-043, AZ-044 |
| R6 | 团队模式成员串环境/预算按成员复制，成本与改动失控 | **P1**（开启团队前） | Pool/Team 跑真实仓库 | AZ-070, AZ-071 |
| R7 | 外部发送无宿主幂等适配，丢响应后重复发送/发布 | **P1** | 连接器/gateway 发送类动作 | AZ-052（依赖 AZ-025 宿主化） |
| R8 | 未签名分发 + 无可信更新，用户装到被篡改或无法安全升级的包 | **P1** | 公开分发扩大 | AZ-064, AZ-065 |
| R9 | 长任务资源增长/单 WebView 卡顿随会话变长劣化，无基线无预算 | **P2** | 2 小时级任务、长对话 | AZ-057, AZ-059, AZ-060 |
| R10 | 内核契约未冻结 + vendor patch 漂移，升级内核引入静默回归 | **P2** | 内核版本迭代 | AZ-026；`loop_.rs` 巨型文件 |

R1/R2 的共同特点：**都发生在产品最想宣传的场景里**（人机同改、装扩展）。这决定了它们不是“已知问题列表”的一员，而是发布声明的限定词。

---

## 7. 路线建议（以验收事件为里程碑，不以版本号）

### 阶段 A：合上单 Agent 可托付闭环（1.0 闸门，建议冻结新功能面）

1. **交接与编辑安全**：AZ-036（四类操作任务绑定）→ AZ-041（脏缓冲冲突）→ AZ-037（人类编辑进下一轮上下文）。验收：一次真实 bug 修复旅程“WorkZ 委派 → CodeZ 接手改一行 → Agent 续跑感知 → 评审应用”全程录像存证。
2. **原生 E2E 门禁**：AZ-056 落地并接入发布流水线（失败阻断），素材直接复用阶段 A1 的旅程 + AZ-003 的 A/B 夹具。
3. **安全收口**：AZ-032 真机正反测试 → AZ-031 全出口秘密扫描 → 据此决定第三方扩展/连接器分发范围。
4. **数据与发行**：AZ-063 两历史版本迁移夹具 + 恢复演练；AZ-064 取得签名/公证并把 `release-readiness` 接入 `release.yml`；AZ-065 更新通道可后置，但**降级保护（ADR-004）必须随 1.0 生效**。
5. **性能基线**：AZ-057 冻结构建跑冷启动 + 2 小时 soak，产出 AZ-059/060 的预算数字；无数字不做性能宣称。
6. **仓库卫生**：未跟踪源码收敛入仓，恢复“发布物 ↔ 提交”可追溯；`chat_turn.rs` 与 `App.tsx` 按域拆分（借 AZ-035 的 Task DTO 服务化东风推进，不单独立项大重构）。

**1.0 宣称条件（建议）**：A1–A5 全部以证据关闭；三平台中至少 Windows 完整旅程 + 另两平台安装/启动/核心旅程记录（AZ-058）；支持矩阵按 AZ-005 分级如实标注；多 Agent/桌面操作/远程维持“实验/未验证”标注。

### 阶段 B：把 Harness 优势变成可引用数据

- AZ-062 评测趋势接稳定 harness：固定任务集（含真实仓库 bug 修复/重构/研究/文档四类）按版本回归，公开内部成功率/成本/干预曲线；
- 预算覆盖全部模型入口（含标题/记忆/curator/补全），成本可归因到任务；
- AZ-045 真实 SSH 旅程、AZ-043/044 调试/终端原生回归，补齐 IDE 日常面验收。

### 阶段 C：受控扩大（每项默认关闭、证据开启）

- AZ-070/071 团队隔离与共享预算 → AZ-075 对照评测达标后才默认开放团队；
- AZ-053 浏览器抢占（固定 RobotZ 新版本）→ AZ-054/055 桌面操作与资源租约；
- AZ-072~074 自动化/通知/工作流检查点在 AZ-025 宿主幂等与预算约束下开放；
- AZ-068 真实 Beta 与支持分流在 A 阶段闸门后启动。

---

## 8. 结论

AgentZ 的罕见之处，在于它同时做了三件难事：一个真 IDE 的壳、一个设计现代的 Agent Harness、以及一套不自欺的验收台账治理。前两者决定它**值得继续**，第三者决定它**能够继续**——只要台账中那些 `blocked/preparing` 被当作发布声明的边界而非待办列表的装饰。

以专业 IDE 标准打分，它今天是一款 **5.9 分的 IDE、6.7 分的 Harness、4.5 分的可交付产品**。三者中最短的板（发行与跨平台验收）决定它现在只能内测；最长的板（耐久运行 Harness）决定它一旦补齐验收，就有资格讲一个与 Cursor/Codex 都不同的故事：**不是聊得更快的编辑器，而是托付得住的本地工作台。**

下一个版本最值得写的发布说明只有一行：**“本版本通过了 AZ-056 核心生命周期门禁。”** 在此之前，版本号从 0.9.19 涨到 0.9.29，都不改变本报告的结论。

---

## 附录 A：台账状态摘要（2026-10-18/19 口径）

- 总计 83 项：`done` 37、执行/评审 11、阻塞 11、其余为 `preparing/pending/deferred`；Gate 仅 M0、M1 `passed`，M2–M9 与 MB（真实任务基线）均 `pending`。
- 46 项未验收清单与用户测试用例见 `docs/upgrade/acceptance-0.9.12.md`（UT-001~046、UT-R01~R07），至今多数未获回写证据。
- 2026-10-13 复评对多项历史 `done` 附加限定：`bounded_source_review / historical_* / current_source_read_only_no_tests_rerun`——本报告评分已按“限定后的可信度”计。

## 附录 B：本报告与既有文档的关系

- `docs/agentz-product-maturity-review-2026-10-04.md`（F01–F12 审查）：其 M0/M1 建议已大部落地，本报告 §2.5/§3.5 为其更新注脚；F 类问题未在当前源码逐项重验者，本报告不宣称关闭。
- `docs/ai-ide-engineering-audit.md`（2026-09-28，0.6.3）：其 LSP/Git/性能结论为历史基线，本报告以台账证据更新状态，未重跑其全部复现。
- `docs/upgrade/reassessment-2026-10-13.md`：本报告的测试口径（310 前端测试、152+2 域测试等）引自该复评日志，未在本轮重跑。
