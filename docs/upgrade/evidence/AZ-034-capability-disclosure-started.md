# AZ-034 执行能力边界盘点（2026-10-06）

## 已确认的入口事实

- 固定内核的 `code_run` 在 Windows 通过 `cmd.exe /C`、在其他目标通过 `sh -c` 启动；`shell` 也直接启动系统解释器。两者使用当前宿主进程的 OS 用户身份。一次性审批、ToolContext workspace 及 WorkZ worktree 不构成子进程文件系统/进程/网络边界。
- WorkZ 工作目录隔离本身的 Rust 模块已有注释说明它不是文件系统或进程安全沙箱。
- 工具目录原先将 `code_run` 描述为“Run code snippets in a sandbox”，与真实行为冲突。

## 本轮实现

将工具目录的 code_run hint、内核 `code_run`/`shell` 描述和 headless system prompt 改为明确说明：子进程以当前 OS 用户运行、当前没有操作系统隔离；file-tool 的 workspace 限制不会约束 shell/code_run 子进程。增加 Rust 回归测试覆盖这些说明。此修改是能力边界披露，不是 OS 沙箱实现或平台支持声明。

验证：

- `cargo test -p piscis-kernel sandbox_disclosure --locked`：2 passed，0 failed。
- `cargo test -p piscis-kernel code_run_discloses_it_has_no_os_sandbox --locked`：1 passed，0 failed。
- `cargo test --manifest-path src-tauri/Cargo.toml --lib builtin_catalog_does_not_claim_code_run_is_sandboxed --locked`：1 passed，0 failed。首次断言查错 DTO 字段（label 是工具 ID，展示文本字段为 hint），随即按真实 DTO 修正并重跑通过；首次测试结果不能视作通过证据。

## 仍未完成

AZ-034 要求的越界文件、进程和网络操作真实拒绝尚未实现或验证，因此任务保持 doing。Worktree 不作为沙箱。Windows AppContainer 是可行候选研究方向：Microsoft 文档说明网络能力需显式授予，文件访问通过 package/capability SID 与 DACL 决定；文档同时要求以 `STARTUPINFOEX` / process-thread attribute 启动 AppContainer 进程。当前代码基于 Tokio `Command`，还没有这条受限启动链，也没有 workspace ACL 生命周期实现，故不能宣称 Windows 参考策略可用。参考：[AppContainer 启动指南](https://learn.microsoft.com/en-us/windows/win32/secauthz/implementing-an-appcontainer)、[AppContainer 隔离说明](https://learn.microsoft.com/en-us/windows/win32/secauthz/appcontainer-isolation)。

下一步：评估可在 Windows 上端到端实测的受限进程启动适配器及可撤销/任务专属目录授权；优先建立真实 negative tests，失败时明确报告 unsupported，避免降级到未隔离执行。

## 2026-10-06 Windows 运行时探针补充

增加 Windows-only Rust 探针测试，使用当前锁定的 `windows 0.58` API 创建一次性 AppContainer（零 capability），通过 `STARTUPINFOEX` 的 security-capabilities attribute 启动 `cmd.exe`。测试在宿主临时目录写入唯一 canary，AppContainer 子进程执行 `type <canary>` 并返回非零；测试清理 profile、SID、attribute list、进程句柄和 canary。实际当前 Windows 主机结果：`cargo test -p piscis-kernel windows_appcontainer_child_cannot_read_a_user_temp_file --locked`，1 passed，0 failed，运行 0.44s（编译 2m45s）。这证明 AppContainer 启动及一个用户目录默认拒绝负例，不证明工作区读写授权、网络或进程工具策略，更未接入 code_run/shell。


## 2026-10-06 Windows AppContainer 目录与网络正反例

扩展同一个 Windows-only 宿主集成测试：使用用户临时目录中兄弟隔离的 `work` / `outside` 文件夹。只给独立 AppContainer package SID 授予 `work` DACL：父临时目录只授予 traverse，`work` 及其文件获得访问 ACE。AppContainer 子进程能够读取 `work/existing-source.txt`、在 `work` 中创建文件；读取同级 `outside/private.txt` 返回非零。随后绑定本机 `127.0.0.1:0` TCP listener，在另一个无 network capability 的 AppContainer 中运行 Windows 自带 `curl.exe` 尝试访问该 listener，客户端失败且服务端从未接受连接。整个测试没有向外部地址发请求。

实际结果：`cargo test -p piscis-kernel windows_appcontainer_child_is_confined_to_explicit_directory --locked`，1 passed，0 failed；覆盖 AppContainer `cmd.exe`/`curl.exe` 子进程、目录内读与新建、目录外拒绝、loopback 网络拒绝。单测 4.52s（增量编译 38.90s）。测试目录和每次临时 AppContainer profile 在测试后清理。

仍未覆盖或实现：此 API 探针目前在 `#[cfg(test)]` 中，没有接入 `code_run`、`shell` 或 LSP；没有验证取消/超时后的派生进程组回收、真实 WorkZ worktree/文件树 DACL 可逆生命周期、reparse/junction 与 ACL 竞态、GUI 能力状态或发布构建。Windows 参考策略仍不得对用户任务宣称已启用。

## 2026-10-06 Windows 子进程生命周期探针加固

为避免后续把一次性启动探针误当成可安全取消的执行器，测试启动链新增 Job Object：先配置 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`，以 `CREATE_SUSPENDED` 创建 AppContainer 子进程，在恢复线程前将其加入 Job Object；超时会终止根进程，统一清理会关闭 Job Object。这样探针不会留下失联的执行进程，派生进程在 Windows 默认 Job 继承语义下随 Job 生命周期受控。

实际结果：`cargo test -p piscis-kernel windows_appcontainer_child_is_confined_to_explicit_directory --locked`，1 passed，0 failed；覆盖目录内读取/创建、同级越界读取拒绝和 loopback 网络拒绝。含新增 Windows API feature 的增量构建及测试总耗时 2m54s，单测 4.79s。

边界：该 Job Object 目前仍位于 Windows `#[cfg(test)]` 探针中；虽已断言一个故意派生的子进程在 Job 关闭后未能写出延迟标记，仍未接入 `code_run`/`shell`，所以不构成生产取消/超时保证。生产适配器还需支持输出管道、超时/取消后子进程收割、可逆的任务专属 ACL 及失败关闭路径，并按真实工具入口验收。

## 2026-10-06 Windows Job Object 派生进程清理负例

增强同一 Windows-only 测试：AppContainer 根 `cmd.exe` 启动另一个延迟写标记的 `cmd.exe` 后退出；测试在根命令退出时查询 Job Object，确认仍有根进程记录和派生进程（活动数大于 1），随后关闭启用 kill-on-close 的 Job Object，等待超过子命令延迟，再确认标记文件没有出现。由此验证此测试启动链能在根进程退出后回收派生进程，且清理顺序有效。

实际结果：`cargo test -p piscis-kernel windows_appcontainer_child_is_confined_to_explicit_directory --locked`，1 passed，0 failed，单测 10.34s（含 5 秒派生进程负例）。一次中间失败来自错误假定根进程句柄 signaled 后 Job Object `ActiveProcesses` 必须是零；Windows 仍将已退出但句柄未关闭的根进程计入，因此改为断言根进程加派生进程数量大于 1，并验证关闭 Job 后延迟标记未创建。最终通过结果仅对应最终测试。

## 2026-10-06 可复原 workspace ACL 租约

将测试里的临时 ACL 修改抽成 `piscis_kernel::proc::windows_sandbox::WorkspaceAclLease`。它在任何 DACL 修改前递归预检整棵 workspace，发现 symlink/reparse point 即拒绝；对父目录仅授予 traverse，对根目录、既有文件和子目录给唯一 package SID 限定访问；保存每个既有对象的原始 DACL。结束时恢复既有 DACL，并从运行期间创建的新对象撤销该 SID；显式 `finish()` 将清理失败返回调用方，`Drop` 再作一次尽力回滚。测试夹具现通过这份可复用实现，而非独立的测试 ACL 代码。

当前 Windows 正反测试在所有子进程与 Job 清理后逐字节比较 parent、workspace root、既有源文件的 DACL 与修改前快照；目录内读写、同级拒绝、loopback 拒绝和派生进程清理继续通过。结果：`cargo test -p piscis-kernel windows_appcontainer_child_is_confined_to_explicit_directory --locked`，1 passed，0 failed，10.34s。

边界：ACL 租约 API 尚未从 `code_run` 或 `shell` 调用；应用仍以普通宿主命令路径执行，故生产能力未启用。并发宿主进程修改工作区、执行期间制造 reparse point、清理失败恢复以及真实工具取消/超时仍需继续验证；不能将本测试等同于产品执行隔离。
