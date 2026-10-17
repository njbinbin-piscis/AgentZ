# AZ-034 验收复核

日期：2026-10-06（Asia/Shanghai）  
复核：Codex 自检（本路线图本轮唯一实施参与者）  
结论：任务定义的 Windows 参考策略验收条件通过；不代表跨平台、原生 GUI、LSP 或发行验收通过。

## 验收矩阵

| 条件 | 证据 | 结论 |
| --- | --- | --- |
| 越界文件操作被 OS 拒绝 | AppContainer 不能读 sibling canary；不能把 workspace 文件移动到 sibling；工作区内读写正向用例通过 | 通过（Windows 当前主机） |
| 网络边界实际生效 | 零 capability AppContainer 不能连接 host-local loopback；runner 不增加网络 capability | 通过（Windows 当前主机） |
| 进程收割与失败关闭 | CREATE_SUSPENDED 后先加入 kill-on-close Job；取消、超时与子进程回收用例通过；ACL setup/cleanup 出错不执行 unsandboxed retry | 通过（Windows 当前主机） |
| ACL 生命周期可恢复 | 原 DACL 与继承保护状态恢复；真实 `SetSecurityInfo` 失败保持 lease 可重试；新对象留在工作区、同卷移出及枚举后注入对象的临时 SID 清理测试通过 | 通过（按序 cleanup、当前 Windows 文件系统） |
| 不支持平台披露边界 | `code_run` / `shell` 的非 Windows 描述明确为 unavailable，且命令不执行；相关条件测试在当前 Windows 主机验证 Windows 文案分支 | 代码与 Windows 构建通过；非 Windows target 未构建/运行 |
| worktree 不冒称 OS 沙箱 | `src-tauri/src/commands/agent_task.rs` 模块文档明确说 worktree 仅减少目录干扰、不是 filesystem/process sandbox；[AZ-002 环境契约](AZ-002-identity-adr.md)也明确普通 local/worktree 不构成沙箱 | 通过（文案与契约复核） |

## 验证记录

- `cargo test -p piscis-kernel --locked -- --test-threads=1`：353 passed，0 failed，1 ignored。
- `cargo test --manifest-path src-tauri/Cargo.toml --lib builtin_catalog_discloses_command_sandbox_boundary --locked`：1 passed（先前证据记录）；该用例只核验工具目录能力披露，不是 native GUI 或真实 agent session 验收。
- `cargo clippy -p piscis-kernel --lib --locked -- -D warnings`：通过。
- 四个相关 Rust 文件 `rustfmt --edition 2021 --check`：通过；相关文档 `git diff --check`：通过。
- 独立实现与安全边界详情见 [AZ-034 Windows AppContainer runner evidence](AZ-034-production-runner.md)。

## 保留边界

当前结论适用于本机 Windows x86_64、临时 AppContainer profile 和本机可写 C: NTFS。真实数据库评估因未设置 `$PISCIS_EVAL_DB` 忽略。D: 为无可用空间的 UDF 只读卷，不能做跨卷测试；Linux Rust target 缺少 `core`，未构建。按用户指示跳过原生 GUI；LSP/外部 language server 不由此 runner 托管，仍需单独定义权限策略。也未做 release/profile 打包、低权限用户、多用户、压力竞态和回滚演练。

这些限制没有被记作通过项。AZ-034 的验收条件要求一个 Windows 参考策略能够实际拒绝越界操作、明确 unsupported 平台边界且不把 worktree 说成沙箱；本轮所述验收条件由宿主自动化与现有契约证据覆盖。平台发行与 GUI/LSP 验收按路线图其他交付独立跟踪。
