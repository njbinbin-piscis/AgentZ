# 闪退与内存增长排查（2026-09-28）

## 已确认并修复的代码问题

- 文件监控每个变更创建一个原生线程，线程内打开 SQLite 并更新索引。改为每个监控器一个后台工作线程，200 ms 合并重复路径，最多保留 1024 个待处理路径；溢出后使用一次完整重建，避免静默漏掉文件。监控器释放后工作线程退出（正在执行的批次会先完成）。启动/停止使用同一个锁，避免重复创建。
- 后端终端日志只有 5000 行上限，不换行输出与超长行仍会无限增长。增加总计 2 MiB、单行 64 KiB 上限，保留最近内容，按 UTF-8 边界截断。
- 前端扩展 OutputChannel 使用无限字符串拼接。每个通道改为保留最近 512 Ki 个 UTF-16 code units；host/debug 日志同时限制单条为 4096 个 code units，保留原有条数上限。
- 编辑器 LSP 启动完成晚于组件卸载时，会重新注册全局 provider 和 WebSocket。增加生命周期序号校验，释放未完成连接，失败路径也断开连接。修复应用关闭事件与工作区浏览器事件异步订阅晚于 cleanup 的监听器泄漏。
- 增量索引先读取整个文件再判断大小。改为读取前检查普通文件和大小，并使用限长读取，防止文件在检查后增长。图索引也使用同样的读取保护；编辑器单文件预览限制为 10 MiB，超限返回错误。
- 索引遍历的 20000 文件上限只在进入目录时检查，单个大目录可突破限制。现在逐项检查上限。代码搜索只保留前 50 个以内的最佳候选，不累积全部命中片段。
- 图索引原先每处理一个文件都复制整个文件名索引。改为每次重建只建立一次共享只读查找表，减少打开大项目时的重复分配和 CPU 消耗。

这些是能够从代码与回归测试确认的资源问题；没有 Windows 进程堆转储，不能断言它们就是所有黑屏/OOM 的唯一原因。

## 本机 Linux 证据

本机为 Ubuntu 24.04.1、VMware Virtual Platform，系统 WebKitGTK 为 2.52.6。

启动系统已安装的 `/usr/bin/agentz-desktop` 时，捕获到 `VMware: No 3D enabled`、`Could not get DRI3 device`、`failed to create dri2 screen`。该次启动返回 137，未捕获到 Rust panic 或内核 OOM 记录。137 本身不能证明是 OOM，也不能独立证明显卡错误导致退出。

使用 `WEBKIT_DISABLE_DMABUF_RENDERER=1 LIBGL_ALWAYS_SOFTWARE=1` 对比：35 秒和后续 60 秒观察均未提前退出，EGL/DRI 错误未再出现。60 秒测试仅监测主进程，RSS 从约 182 MiB 上升并趋于约 235 MiB；不代表整个 WebView 进程树或长期内存稳定性。现有工作区状态指向本项目，但测试没有自动操作文件夹选择对话框，尚未完成用户原始“打开项目”操作的完整复现。

WebKit 官方问题记录包含 DMA-BUF 路径不可用时禁用该路径的处理方式：
https://bugs.webkit.org/show_bug.cgi?id=291332

新编译的 Linux 程序支持按需使用：

```sh
./agentz-desktop --software-rendering
```

已安装旧版本可直接进行相同的对比：

```sh
WEBKIT_DISABLE_DMABUF_RENDERER=1 LIBGL_ALWAYS_SOFTWARE=1 \
  RUST_BACKTRACE=1 /usr/bin/agentz-desktop > /tmp/agentz-linux.log 2>&1
```

仅对该次进程生效，不改变系统显卡配置。未对全部 Linux 用户默认关闭硬件渲染。新增 Rust panic 日志记录位置与 backtrace；原生 WebKit 崩溃和 SIGKILL 仍需系统日志/进程转储。

## 回归验证与后续复测

自动化覆盖：重复文件事件合并、十万路径溢出后的有界队列、停止工作线程释放回调、连续无换行的多字节终端输出、日志总字节/行数上限、文件读取边界、扩展输出截断。

Windows 需重新编译后进行长时间复测，区分 `agentz-desktop.exe`、`msedgewebview2.exe` 与扩展宿主的内存增长；持续终端输出、扩展日志输出以及快速切换文件/项目应分别测试。Linux 需在软件渲染模式下重复手动打开原项目、编辑和切换项目；当前对比只支持渲染兼容性方向，尚不能认定原始闪退已完全解决。

最终检查结果：`cargo test -p agentz-desktop --lib --bins`：87 通过、1 忽略；`npm test`：21 通过；`npm run typecheck` 和 `npm run build` 通过。修改的前端文件 ESLint 为 0 错误，仍有项目原有的 unused-disable / Hook 依赖警告；Vite 有大 chunk 提示。未生成或安装新的 Windows 安装包，也未替换本机系统安装版。
