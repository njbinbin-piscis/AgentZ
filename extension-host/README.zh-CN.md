# AgentZ Extension Host

[English](README.md) | **简体中文**

> 本文是 [README.md](README.md) 的中文翻译。两者不一致时以英文版为准。

AgentZ 的一个净室实现、兼容 VS Code 的**扩展宿主**。它让真实的 `.vsix` 扩展
JavaScript 运行在一套 `vscode` API 实现之上，并通过结构化 RPC 协议与 AgentZ
渲染进程（Monaco + React）通信——即“Theia 式兼容宿主”方案。

## 架构

```
Tauri renderer (React + Monaco)          Tauri host (Rust)         Node sidecar (this package)
┌───────────────────────────┐  ext_host  ┌──────────────────┐  stdio  ┌──────────────────────────┐
│ MainThread* bridges        │◀──events──▶│ ext_host.rs      │◀──────▶ │ extensionHostProcess      │
│ rpcProtocol (renderer copy)│  invoke    │ broker (stdin)   │  NDJSON │ rpcProtocol + ExtHost*    │
│ Monaco language registry   │            │ vsix.rs (unpack) │         │ vscode API factory        │
└───────────────────────────┘            └──────────────────┘         │ 3rd-party extension JS    │
                                                                       └──────────────────────────┘
```

- **传输**：按行分隔的 JSON（NDJSON）。渲染进程无法直接与 Node 进程通信，因此由
  Rust 宿主代理：sidecar 的 stdout 行通过 Tauri 事件通道 `agentz:ext-host` 发出，
  渲染进程通过 `ext_host_send` 命令回传帧（→ sidecar stdin）。
- **协议**：`src/common/{proxyIdentifier,rpcProtocol,protocol,dto}.ts`。
  `MainThread*` 形态运行在渲染进程，`ExtHost*` 形态运行在这里。方法名以 `$` 开头，
  按数字代理 id 路由。渲染进程在 `src/extensions/common/` 下保留一份字节级兼容的
  `common/` 模块副本。
- **API 表面**：`src/host/apiFactory.ts` 用 `ExtHost*` 服务和 `types-impl.ts` 中的
  具体类型组装出 `vscode` 命名空间。`require('vscode')` 在 `extensionService.ts`
  中按扩展拦截。

## 构建

```bash
npm install
npm run build      # -> dist/host.js (+ dist/smoke.js)
npm run smoke      # 进程内回环测试（无需 Tauri）
npm run typecheck
```

Rust 宿主启动的是 `dist/host.js`：`node <resources>/extension-host/host.js`。
`ext_host.rs`（`resolve_host_js`）中的解析顺序：显式参数 → `$CODEZ_EXT_HOST_JS` →
开发构建（crate 旁边的 `extension-host/dist/host.js`，只要存在就优先使用）→
打包资源（`extension-host/host.js`）→ 相对于可执行文件 / 当前目录的旧路径。
Node 可执行文件（`node_bin()`）：`$CODEZ_NODE` → 打包的固定版本运行时
（`resources/node/<platform>-<arch>/node[.exe]`）→ 系统 `node`。

## 打包（Node 运行时）

Tauri 不内嵌 Node。对于发布构建：

1. `tauri.conf.json` 把 `dist/host.js`（及其 source map）作为
   `extension-host/host.js` 打入应用资源（`bundle.resources`），并且
   `beforeBuildCommand` 会运行 `npm run build:exthost`。
2. `npm run fetch-node`（`scripts/fetch-node.mjs --remote`）把固定版本的 Node.js
   运行时（`NODE_VERSION`，当前为 22.11.0）下载到 `src-tauri/resources/node/`：
   一份为宿主平台解压好的运行时，加上供远程目标使用的 linux-x64 / linux-arm64
   压缩包。该目录作为应用资源打包，且发布工作流在每个打包任务中都会运行
   `fetch-node`，因此发布构建自带 Node。若缺少它（例如跳过了 `fetch-node`
   的本地构建），宿主会回退到系统 `node`。

## 许可策略

- 本宿主是 **MIT 净室实现**。它*没有*引入 `microsoft/vscode` 或
  `eclipse-theia/theia` 的源文件；公开的 `vscode` 扩展 API 契约和 RPC 模式在此
  重新实现。
- Eclipse Theia（EPL-2.0）仅作为 MainThread 侧设计的*参考*；未复制任何 Theia
  源码，因此项目默认保持 MIT。
- 扩展从 **Open VSX**（以及用户提供的 `.vsix`）安装，以避开 VS Code Marketplace
  的服务条款。

## 兼容级别（专业版）

已实现的能力桥接：命令、文档/编辑器、工作区 + fs + 配置、消息、状态栏、快速输入、
输出通道、语言功能（补全 / 悬停 / 定义 / 引用 / 高亮 / 符号 / 格式化 / 代码操作 /
签名帮助 / 诊断）、树视图、webview、终端、SCM 提供者、任务（通过 PTY）、调试
（DAP）、测试以及笔记本序列化器。

已知限制：原生（`.node`）模块需要 ABI 匹配的预编译版本。

## 远程目标（agentz-server）

同一个 `host.js` 同时充当 **agentz-server**：带有 `$agentz` 键的 stdin 行是控制面
（`src/server/agentzServer.ts`：`info`、`fs.*`、`search`、`git.discover`、
`ports.list`、`tcp.*`、`exec`、`proc.*`、`shutdown`），在 stdout 上以
`{"$agentz":"res"|"event"}` 应答，其余都是扩展 RPC。Rust 代理
（`commands/ext_host.rs`、`remote/`）通过以下方式之一启动它：

| 目标 | 传输 |
|---|---|
| 本地 | `node host.js`（有打包的固定版本 Node 则用它，否则用系统 `node`） |
| SSH | 系统 `ssh -T host sh -c …`（遵循 `~/.ssh/config`、agent、ProxyJump） |
| Docker / Dev Container | `docker exec -i <id> sh -c …`（通过 `@devcontainers/cli` 执行 `devcontainer up`） |
| WSL | `wsl.exe -d <distro> -- sh -c …` |

连接时，bundle 按内容寻址上传到 `~/.agentz-server/<hash>/server.js`；若目标缺少
Node >= 18，还会一并上传 `src-tauri/resources/node`（`npm run fetch-node`）中的
固定版本 Linux 压缩包。`extensionKind` 允许工作区侧运行的扩展会以 tar 同步到
`~/.agentz-server/extensions/`。远程文件在渲染进程中以
`agentz-remote://<authority>/<path>` 寻址，并在 RPC 边界改写为 `file:` URI
（`typeConverters.ts`）。

### IDE 中的远程工作区

远程文件夹以 URI 本身作为项目目录打开，例如
`agentz-remote://ssh-remote+box/home/me/proj`。由于 IDE 以 `${projectDir}/${rel}`
构造文件路径，每个 `ide_*` 命令都会收到远程 URI 并据此路由
（`crate::remote::resolve`）：

| 功能 | 远程实现 |
|---|---|
| 文件树 / 读 / 写 / 新建 / 重命名 / 删除 | 服务器上的 `fs.*` |
| 在文件中查找 | `search`（远程有 ripgrep 则用它，否则用 grep） |
| 外部变更重载 | `fs.watch` → `ide-file-changed`；重连后重新挂载 |
| Git 面板 | 通过 `exec` 在远程运行 `git`；嵌套仓库由 `git.discover` 发现（深度 4，带缓存） |
| 终端 | 本地 PTY 运行 `ssh -tt` / `docker exec -it` / `wsl --cd`——真正的远程 TTY |
| 语言功能 | 远程主机上的扩展，加上内置 LSP（`remote/lsp.rs`）：服务器经目标传输在远程启动，URI 在桥接处 `agentz-remote://` ↔ `file://` 互相改写 |
| Agent 工具 | `tools/remote_fs.rs` 用远程版本替换 `file_*` / `shell` |
| 调试 | 带 `remoteDir` 的 `dap_start` 在远程启动适配器（目标传输）；`dapClient` 为断点 / 栈帧映射 `agentz-remote://` ↔ POSIX 路径 |
| 代码索引 | `remote/mirror.rs` 维护一个增量的本地源码镜像（`~/.agentz/remote-mirror/v2-<hash>`，文件 <512 KB，剔除依赖目录）；codebase / graph / symbol 索引、对应的 agent 工具以及 `@codebase` / `@graph` 都在其上运行 |
| Agent 上下文 | `.agentz/rules` / `.cursor/rules` 和 `@file` 提及从远程读取 |
| 端口转发 | `ports.list`（Linux `/proc/net/tcp`）；SSH 使用 `ssh -N -L`，Docker/WSL 经服务器隧道（`tcp.*`）。连接后新打开的 ≥1024 端口会自动转发并发出通知 |
| 仅密码主机 | “设置密钥登录”一次性安装你的公钥（`remote/ssh_setup.rs`） |

在连接仍在部署时发出的命令会等待其完成（`ExtHostManager::wait_for_authority`）。
状态栏显示远程徽标，链路断开时变为重连按钮；渲染进程也会按退避策略重试。

远程限制：宿主连接是非交互式的，因此仅密码登录的主机需要先完成一次性密钥设置。
新主机密钥会被自动接受（`accept-new`）；变更的密钥会被拒绝。端口自动检测仅支持
Linux（手动端口在任何平台都可用）。索引镜像最多每 20 秒重新同步一次（在索引重建 /
工具使用时），因此结果可能短暂落后于远程。仅 musl 的镜像（Alpine）需要系统 Node，
因为打包的压缩包是 glibc 构建。
