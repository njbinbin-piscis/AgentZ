# AgentZ

[English](README.md) | **简体中文**

> 本文是 [README.md](README.md) 的中文翻译。两者不一致时以英文版为准。

AgentZ 是一个桌面 AI IDE（当前源码版本 **1.0.52**；本地工程与自动化验收门禁已通过，参见 [1.0.0 发布决策](docs/upgrade/release-decision-1.0.0.md)）。它把编辑器和自治智能体放在同一个应用里，共用一套 agent 内核：人可以在编辑器里逐处改代码，也可以把一个目标交给智能体，在隔离的 git worktree 里做完再评审。

界面有三种一等模式，顶栏切换：

| 模式 | 做什么 |
| --- | --- |
| **CodeZ** | 以编辑器为中心。Monaco、文件树、搜索、Git、终端、LSP、调试，右侧是可开关的 AI 对话。接近 Cursor。 |
| **WorkZ** | 以任务为中心。提交目标后智能体自己计划、改代码、跑工具；任务跑在独立分支上，完成后看 diff，合并、开 PR 或丢弃。也可以把多个智能体组成团队。接近 Codex。 |
| **BotZ** | 以智能体为中心。每个 Bot 有长期对话、记忆和定时例程；团队在各自的办公室目录里协作，主管拆任务，成员做完后评审、退回或合并。 |

内核来自 [`piscis-engine`](https://github.com/njbinbin-piscis/piscis-engine)（`piscis-core`、`piscis-kernel`、`piscis-ide-tools`，当前钉在 `v0.8.71`）。AgentZ 自己实现桌面宿主、IDE、扩展宿主和远程开发。仓库许可证是 [Apache-2.0](LICENSE)。

安装包从 GitHub Releases 下载，目前**未签名**，需要手动安装。步骤和各平台的系统提示见 [RELEASE.md](RELEASE.md)。

## CodeZ

打开一个本地文件夹，或从状态栏连接远程主机后打开远程目录。

- **编辑。** 多标签 Monaco 编辑器，Markdown / 图片预览，浅色与深色主题，界面中英文。
- **工程。** 文件树、全局搜索、Git（暂存、提交、分支、嵌套仓库）、集成终端（xterm + PTY）。关闭标签会释放对应的编辑器模型。
- **语言与调试。** LSP 经 WebSocket 桥接到语言服务器。调试走 DAP：断点、调用栈，调试适配器可以跑在本机或远程。
- **对话。** 流式 Markdown、工具调用折叠显示、停止、排队下一条消息、会话列表（新建、切换、删除、分叉）。从检查点分叉时可以选择仅复制上下文或复制完整历史。用 `@路径` 引用文件，`@codebase` / `@graph` 引用代码索引和代码图。选中代码后按 ⌘K / Ctrl-K 做行内编辑，结果以绿/红 inline diff 呈现，Enter 接受、Esc 撤销。Tab 补全显示 ghost text。
- **代码地图。** 依赖图和体量热力图，背后是增量代码图索引（[docs/graph-schema.md](docs/graph-schema.md)）。
- **内置浏览器。** 页面截图和元素可以送进对话；浏览器工具使用 [RobotZ](https://github.com/njbinbin-piscis/RobotZ)（`robotz-browser` v0.1.14）。
- **扩展。** 真正执行 `.vsix` 里的 JavaScript，而不是只读 `package.json` 贡献点。见下方「扩展宿主」。

## WorkZ

- **任务。** 每个任务是一次内核会话。提交目标后，计划、编辑和工具调用流式显示在步骤里。
- **隔离。** 任务在项目同级的 `../.agentz-worktrees/task-<id>` 上新建 `workz/task-<id>` 分支，主工作区不被直接改写。多个任务可以并行。评审面板对 `base...branch` 做 diff，然后合并（no-ff）、用 `gh` 开 PR，或删除 worktree 和分支。
- **团队。** 工作室里编写可复用的智能体（角色、提示词、技能和工具）和团队。团队有两种跑法：蜂群协作，或在工作流设计器里把节点连成图（分支、重试、人工确认后续跑）。协作板显示池里的活动。
- **内置种子。** 首次运行会写入 Architect、Coder、Reviewer、Researcher、Writer，以及团队 `fullstack-squad`、`research-duo`。
- 普通会话可以不绑定项目，工作目录在设置里指定。

## BotZ

- **Bot。** 资源库里创建的智能体出现在名单里。点开一个 Bot 是和它的长期对话：它记住要点，你也可以直接改这段记忆。每日 token 预算用完后要等到次日，或在侧栏调高上限。
- **例程。** 给 Bot 定时任务，每天固定时间或每隔若干分钟跑一次提示词，也可以立即运行。
- **办公室。** 每个办公室是一个团队，绑一个本地工作目录，彼此隔离。可以从团队模板填成员和协作约定。远程目录不能建办公室。
- **协作。** 给主管布置目标后，主管拆分并分到成员桌上。看板上分待办、进行中、阻塞或待评审、完成。评审可以引用 diff 里的行，退回修改、通过，或通过并合并。

## 智能体能力

三种模式跑的是同一个 agent loop，差别在工具面、上下文和产物落在哪里。

- **模型。** Anthropic、OpenAI、DeepSeek、通义千问、MiniMax、智谱、Kimi，以及自定义 OpenAI 兼容端点。可配置多套命名模型、温度、top_p、思考模式、流式开关和视觉输入。密钥只存在本机。
- **策略。** 严格 / 均衡 / 开发者三档。一次任务的模型与工具迭代有上限（默认 200）。计划待办未完成时是否自动续跑可以在设置里开关；智能体向你提问时不会自动续跑。
- **技能、规则、钩子、MCP。** 技能市场走 ClawHub，也支持本地目录或清单安装。项目规则读 `.agentz/rules/`（也认 `.cursor/rules/`）。`hooks.json` 在回合前插入钩子。设置里的 MCP 服务器（stdio 或 SSE）把工具注入对话。
- **子代理。** 主智能体可以把一段只读调研委派出去，有预算和超时，不能再往下委派。可选一个 Flash 小模型专门跑这类轻任务。
- **仓库 Wiki 与代码索引。** 索引把源码切成窗口做关键词检索（不需要 embedding key）。Wiki 从索引生成模块和架构概览。
- **开发环境线索。** 回合开始时扫描 Rust、Node、Python、Go、Java/Kotlin、.NET、C/C++、Tauri、PHP、Ruby、Dart/Flutter 等工具链，缺的会作为待办交给智能体，用 `devenv` 工具核对、处理或忽略。
- **设置变更可回滚。** `app_control` 按字段分级：密钥和授权开关锁定，安全和路由类每次都要确认。设置面板里有变更记录，可以单条回滚。
- **消息渠道。** 助理可以接飞书 / Lark、微信、钉钉。连接器把外部服务以 OAuth 或 API key 接成 MCP。

## 远程开发

状态栏左下角的 **⇄ 远程**（没打开文件夹时也在）打开连接对话框：

1. 选择 SSH、Dev Container、Docker 或 WSL。SSH 可以填 `user@host`，或 `~/.ssh/config` 里的 Host。非 22 端口会在 `~/.ssh/config` 里追加一个别名。
2. 免密失败时输入一次密码。AgentZ 把本机公钥装到远程的 `authorized_keys`，之后不再要密码，密码本身不保存。
3. 从远程 home 逐级选目录，或直接输入路径。打开后部署 **agentz-server**（扩展宿主的同一份 `host.js`），再进入该目录。

远程项目的路径形如 `agentz-remote://ssh-remote+host/home/me/proj`。文件、搜索、Git、文件监视、终端、语言服务、调试和智能体工具都走这条连接。远程没有 Node ≥ 18 时，会上传一份随应用打包的 Linux Node。工作区侧扩展同步到远程的 `~/.agentz-server/extensions/`。代码索引在本机维护一份增量镜像。

限制：

- 探测连接 15 秒超时。新的主机密钥按 `accept-new` 接受，**已改变的密钥会被拒绝**。
- 端口自动发现只在 Linux 远程上可用（读 `/proc/net/tcp`）；手动转发在各目标上都可以。连接之后新开的 ≥1024 端口会自动转发。
- 打包的 Node 是 glibc 构建。只提供 musl 的镜像（如 Alpine）需要系统里已有 Node。
- 索引镜像最多约 20 秒同步一次，搜索结果可能短暂落后于远程。

传输细节见 [extension-host/README.md](extension-host/README.md) 的 Remote targets 一节。

## 扩展宿主

`extension-host/` 是一个干净实现的 VS Code 兼容扩展宿主：Node 侧车进程执行扩展 JavaScript，经 Rust 把 NDJSON RPC 转到渲染进程。它不拷贝 VS Code 或 Theia 的源码。扩展从 Open VSX 或用户提供的 `.vsix` 安装。

已经接通的能力包括：命令、文档与编辑器、工作区与配置、消息、状态栏、快速输入、输出通道、补全 / 悬停 / 定义 / 引用 / 格式化 / 代码操作 / 诊断、树视图、Webview、终端、源代码管理、任务、调试和测试、笔记本序列化。

原生 `.node` 模块需要与宿主 Node ABI 匹配的预编译包。同一份 `host.js` 在远程上作为 agentz-server 的控制面（`fs.*`、`exec`、`proc.*`）。

## 仓库结构

```
AgentZ/
├── src/                        # Vite + React + TypeScript 界面
│   ├── App.tsx                 # CodeZ / WorkZ / BotZ 切换、项目与设置
│   ├── i18n/                   # 英文、中文
│   ├── extensions/             # 渲染侧扩展 RPC、远程 URI
│   ├── services/tauri/         # 前端到宿主的命令封装
│   └── workspaces/
│       ├── codez/              # 编辑器、对话、Git、终端、浏览器、远程对话框
│       ├── workz/              # 任务、团队、协作板、工作流
│       └── botz/               # Bot 名单、长期记忆、例程、团队办公室
├── src-tauri/                  # Tauri 2 桌面宿主（agentz-desktop）
│   └── src/
│       ├── commands/           # IDE、对话、Git、远程、扩展、市场、工作流…
│       ├── remote/             # SSH / Docker / WSL、部署、端口转发、代码镜像
│       ├── gateway/            # 飞书、微信、钉钉
│       └── lsp/                # LSP ↔ WebSocket
├── extension-host/             # Node 扩展宿主，兼远程 agentz-server
├── crates/
│   ├── agentz-host/            # 内核连通性冒烟二进制
│   └── agentz-task-state/      # 持久化的任务与环境标识
├── vendor/                     # 内置的 piscis-engine 与 secret-service 源码
├── bundled/                    # 预装技能等资源
└── docs/                       # 设计说明、评审与发布记录
```

## 开发

需要 Node 20+、Rust（`rust-version` 见 `src-tauri/Cargo.toml`，工作区把 warning 当错误）、以及 Tauri 在当前系统上的依赖。Linux 上构建需要：

```bash
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev \
  libjavascriptcoregtk-4.1-dev librsvg2-dev \
  libayatana-appindicator3-dev patchelf build-essential xdg-utils
```

```bash
npm install
npm run typecheck          # tsc --noEmit
npm test                   # vitest
npm run tauri dev          # 构建扩展宿主、启动 Vite，再跑桌面壳
cargo check -p agentz-desktop
cargo run -p agentz-host   # 不启动界面，只验证内核能链上
```

`npm run tauri dev` 会执行 `beforeDevCommand`：先 `npm run build:exthost`，再起 Vite（端口 5273）。打包：

```bash
npm run tauri build
# 产物在 target/release/bundle/
```

发布用标签触发。`.github/workflows/release.yml` 在推送 `v*` 时为 Linux（x86_64 / aarch64 的 `.deb` 与 AppImage）、Windows（x86_64 / aarch64 的 NSIS `.exe`）和 macOS（Apple Silicon 与 Universal 的 `.dmg`）构建并挂到 GitHub Release。Windows 上 `src-tauri/tauri.windows.conf.json` 把打包目标限定为 NSIS，`.exe` 是唯一的 Windows 安装包。Windows 安装包的支持范围见 [AZ-005 支持矩阵](docs/upgrade/evidence/AZ-005-support-matrix.md)，已知问题见 [docs/known-issues.md](docs/known-issues.md)。版本同步还包括锁文件、README、RELEASE 和对应发布说明/清单；运行 `node scripts/upgrade-version-check.mjs` 检查它们和本地文档链接，并更新 [CHANGELOG.md](CHANGELOG.md)。

构建通过 `Cargo.toml` 的 `[patch]` 段编译 `vendor/piscis-engine` 里的内核。修改内核时，保持 vendor 副本与上游 `piscis-engine` 仓库一致，再更新钉住的 tag。

可选环境变量见 [.env.example](.env.example)。常用的有：

| 变量 | 作用 |
| --- | --- |
| `AGENTZ_CONFIG_DIR` | 覆盖全局配置目录 |
| `RUST_LOG` | 日志级别，例如 `info` |
| `CODEZ_AUTO_MODEL_ROUTING` | 设为 `1` 时，计划用更快的模型、执行用更强的模型 |
| `CODEZ_EXT_HOST_JS` / `CODEZ_NODE` | 指定扩展宿主脚本和 Node 可执行文件 |
| `CODEZ_CHROME` | 内置浏览器使用的 Chrome / Chromium |

无 GPU 的虚拟机（例如 VMware 的 `vmwgfx`）会自动关掉毛玻璃和无限循环动画。若要强制开关，把 localStorage 的 `agentz-graphics` 设为 `low` 或 `full`。

## 配置与数据

全局配置（`config.json`、智能体、团队、日志）在 Tauri 的应用数据目录：

- Linux：`~/.local/share/com.agentz.desktop/`（或 `$XDG_DATA_HOME/com.agentz.desktop`）
- macOS：`~/Library/Application Support/com.agentz.desktop/`
- Windows：`%APPDATA%\com.agentz.desktop\`

`AGENTZ_CONFIG_DIR` 可以指到别处。日志在该目录的 `logs/`。

项目数据在 `{项目}/.agentz/`：会话库 `piscis.db`、规则、钩子、代码索引、Wiki。从旧的 CodeZ 应用迁过来时，把 `com.codez.desktop` 的数据目录换成 `com.agentz.desktop`，并把项目里的 `.codez/` 改名为 `.agentz/`。

没有配置 API key 时，对话会直接返回错误，并在设置里提示先填写密钥。

## 文档

- [CHANGELOG.md](CHANGELOG.md) — 版本变更
- [RELEASE.md](RELEASE.md) — 安装未签名包、签名与自动更新的后续计划
- [CONTRIBUTING.zh-CN.md](CONTRIBUTING.zh-CN.md) — 构建、测试与提交改动
- [SECURITY.zh-CN.md](SECURITY.zh-CN.md) — 报告安全漏洞
- [extension-host/README.md](extension-host/README.md) — 扩展宿主与远程控制面
- [docs/graph-schema.md](docs/graph-schema.md) — 代码图的节点、边和增量更新
- [docs/known-issues.md](docs/known-issues.md) — 已知问题
- [docs/agentz-design.md](docs/agentz-design.md) — 早期设计笔记。远程开发、扩展宿主和工作流是后来加上的，以本 README 和代码为准

英文文档为准。中文译文与英文文件放在一起，命名为 `*.zh-CN.md`；两者不一致时以英文版为准。`docs/upgrade/` 下的发布记录属于历史档案，保留原语言。

## 参与贡献

欢迎提交 issue 和 pull request。请先阅读 [CONTRIBUTING.zh-CN.md](CONTRIBUTING.zh-CN.md)，并遵守[行为准则](CODE_OF_CONDUCT.zh-CN.md)。

## 许可证

[Apache-2.0](LICENSE)。第三方声明见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
