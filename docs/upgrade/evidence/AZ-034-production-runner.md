# AZ-034 Windows AppContainer 工具接入与验证

日期：2026-10-06（Asia/Shanghai）  
状态：实现进入真实内核工具入口并通过 Windows 宿主自动化；AZ-034 仍为 `doing`，本文不代表桌面发行验收或跨平台验收。

## 实现

- 新增生产 `run_appcontainer_command`，给每次调用创建零 capability 的临时 AppContainer；以 `CREATE_SUSPENDED` 启动解释器，在恢复首条指令前放入 kill-on-close Job Object。
- `workspace_root` 与 `cwd` 分开验证。canonical cwd 必须是现存目录且位于 canonical workspace 根内；授予 package SID 的临时 DACL 仅覆盖 workspace 根。进程 cwd 去除 Windows `\\?\` 本地盘前缀后传给 CreateProcess，UNC/非本地盘 cwd 失败关闭。
- `WorkspaceAclLease` 在改 ACL 前递归拒绝 reparse point 和多 hard-link 文件，预先打开并保留对象句柄；生产 DACL 快照/修改/恢复使用 `GetSecurityInfo` / `SetSecurityInfo` 操作已打开对象，并以 `FILE_FLAG_OPEN_REPARSE_POINT` 打开最终路径分量。既有对象的 ACL 和 DACL 继承保护位会恢复；结束时通过句柄撤销新建对象上的唯一 package SID。显式清理失败返回错误；Drop 再尽力恢复。没有 unsandboxed fallback。
- 受限启动器使用独立 stdout/stderr/stdin 管道和句柄白名单、允许列表环境变量与显式覆写、输出上限、超时及协作取消；超时/取消或根进程退出后终止 Job、等待进程退出、排空输出并恢复 ACL。
- `code_run` 和 `shell` 已改为调用同一受限启动器。非 Windows 平台明确返回拒绝；`shell` 的 `elevated=true` 明确拒绝，不再触发 UAC 或自动提权。两工具与 Tauri builtin catalog 的描述已同步披露 Windows-only、workspace scope、无网络能力和提权限制。

## 本机验证

本机为 Windows x86_64 MSVC。以下命令均在 `C:\Projects\AgentZ` 执行：

- `cargo test -p piscis-kernel --locked`：344 passed，0 failed，1 ignored（需真实 `$PISCIS_EVAL_DB` 的评估）；关联 pool integration：25 passed，0 failed；doc-test：1 ignored。
- `cargo clippy -p piscis-kernel --lib --locked -- -D warnings`：通过。
- `cargo test -p piscis-kernel appcontainer --locked`：3 passed，0 failed，覆盖 code_run、shell 工具入口和内核 AppContainer 探针。
- `cargo test -p piscis-kernel production_runner --locked`：4 passed，0 failed，覆盖嵌套 cwd 仍可读取 workspace 根文件、workspace 外同级文件拒绝、无 network capability 下 loopback 拒绝、hard-link workspace 失败关闭、超时/取消 Job 收割。
- `cargo test -p piscis-kernel shell_ --locked`：4 passed，0 failed，覆盖 shell 描述、shell 实际受限命令、workspace 外读取拒绝及拒绝提权。
- `cargo test --manifest-path src-tauri/Cargo.toml --lib builtin_catalog_discloses_command_sandbox_boundary --locked`：1 passed，0 failed（Windows Tauri 测试构建 8m02s）。
- `rustfmt --edition 2021 --check` 对本次改动的四个 Rust 文件通过；`git diff --check` 对相关代码与清单通过。

## DACL 继承状态与失败回滚补测

继续审查 ACL 恢复发现：只保存并回写 ACL 字节不能保留 DACL 是否受保护、是否继续继承父目录 ACE。测试第一次按字节恢复后检测到文件 ACL 的继承标志发生变化，说明原恢复逻辑会改写安全描述符语义。实现现在随原 DACL 一并记录 `SE_DACL_PROTECTED`，恢复时显式传入 `PROTECTED_DACL_SECURITY_INFORMATION` 或 `UNPROTECTED_DACL_SECURITY_INFORMATION`，保留继承状态。

新增 Windows 故障注入：工作区内测试文件暂设 null DACL，验证 runner 在授予阶段失败关闭，并核对此前已处理的父目录和 workspace ACL 完整恢复；测试再恢复测试文件原 ACL，逐字节核对 ACL 且核对继承保护状态。该回归测试通过。Microsoft 文档指出设置 ACL 时的保护/取消保护信息会控制可继承 ACE 的传播，相关语义见[自动传播可继承 ACE](https://learn.microsoft.com/en-us/windows/win32/secauthz/automatic-propagation-of-inheritable-aces)、[SetNamedSecurityInfo](https://learn.microsoft.com/en-us/windows/win32/api/aclapi/nf-aclapi-setnamedsecurityinfoa)及[ACE 继承](https://learn.microsoft.com/en-us/windows/win32/secauthz/ace-inheritance)。

更新后的实际验证：`cargo test -p piscis-kernel --locked` 为 345 passed、0 failed、1 ignored；pool integration 25 passed，doc-test 1 ignored。`cargo clippy -p piscis-kernel --lib --locked -- -D warnings` 通过；本次改动涉及的四个 Rust 文件 `rustfmt --edition 2021 --check` 通过。此前未执行的桌面、Linux target、release、LSP 与外部语言服务器验收状态不变。

## 稳定句柄 ACL 操作与替换夹具

生产代码已从按名称调用 `GetNamedSecurityInfoW` / `SetNamedSecurityInfoW` 改为句柄 API。runner 在首次修改前打开并保留父目录、workspace 和预扫描对象的 no-follow 句柄，先保存全部原 ACL，再逐对象修改；清理时对仍存在的新对象也通过句柄撤销临时 SID。新增确定性替换测试：打开 workspace 文件句柄后，把原文件移到 sibling 并将另一个文件换到原路径，再经旧句柄修改 ACL；测试确认更新只影响原对象，replacement path 上的文件 ACL 和继承状态不变。该 Windows 测试通过。

这缩小了路径替换窗口，但没有关闭整个 workspace 生命周期的竞态：递归阶段已通过 `GetFileInformationByHandleEx(FileIdBothDirectoryRestartInfo/FileIdBothDirectoryInfo)` 从保留的目录句柄枚举名字，并通过 `NtCreateFile` 的 `RootDirectory` + `FILE_OPEN_REPARSE_POINT` 相对打开子项；不再用 workspace 路径枚举后再按路径打开后代。Microsoft 将这两个目录枚举信息类定义为目录句柄查询，并说明后续调用会从同一 handle 继续枚举（[GetFileInformationByHandleEx](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getfileinformationbyhandleex)、[FILE_ID_BOTH_DIR_INFO](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_id_both_dir_info)）。workspace parent 已从卷根逐段解析，ancestor path replacement 测试确认保留 parent handle 仍对应原目录。租约期间对象仍可能由宿主侧并发移出、重命名或新增；保留句柄继续绑定原对象，因此即使对象离开 workspace，恢复动作仍会操作该对象 ACL。因此不把这项改造记作整个对象生命周期 TOCTOU 已关闭，AZ-034 继续保持 `doing`。

Microsoft 的 API 文档说明 `GetSecurityInfo` / `SetSecurityInfo` 接受对象句柄，而 `CreateFileW` 的 `FILE_FLAG_OPEN_REPARSE_POINT` 使最终 reparse point 以 link 本身打开；这里关于 ancestor-chain 未绑定的限制，是由当前调用路径与该 API 契约推断，未声称做过竞态攻击利用：[GetSecurityInfo](https://learn.microsoft.com/en-us/windows/win32/api/aclapi/nf-aclapi-getsecurityinfo)、[SetSecurityInfo](https://learn.microsoft.com/en-us/windows/win32/api/aclapi/nf-aclapi-setsecurityinfo)、[CreateFile](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilea)。

## 从卷根锚定路径与清理重试

本轮继续收窄根路径解析：canonical 本地盘 workspace 现在从卷根开始逐段用相对 no-follow 句柄打开祖先和 workspace parent，每段都检查不是 reparse point/非目录，并在 ACL 租约期间保留祖先句柄。只有 workspace 的直接 parent 获得临时 traversal ACE；不会为锚定路径而改写卷根或更上层祖先 DACL。Windows 测试把已解析 parent 移走并在旧位置建替代 workspace，确认保留 parent handle 仍打开原 workspace。workspace root 句柄不允许 delete sharing，另一个夹具验证租约期间重命名 workspace root 会失败，租约结束后再释放句柄。卷根自身不能作为 workspace。

清理失败注入新暴露 `WorkspaceAclLease::active` 状态赋值反向：成功清理会继续保持 active，失败清理反而关闭重试。已修正为仅在全部 cleanup 成功后关闭 lease。新增真实 Win32 API 故障夹具：让 workspace 根的 ACL 恢复成功，再用没有 `WRITE_DAC` 的子文件句柄触发实际 `SetSecurityInfo` 错误；runner 对外报清理失败、保留 active lease，并继续处理其它对象。替换为有权限句柄重试后所有恢复成功并关闭 lease。

本轮最终 Windows 验证：`cargo test -p piscis-kernel --locked -- --test-threads=1` 为 350 passed、0 failed、1 ignored；pool integration 25 passed；新增根路径锚定、root rename 保护和真实 cleanup 重试测试均通过。`cargo clippy -p piscis-kernel --lib --locked -- -D warnings` 通过。

## 越界移动拒绝与祖先句柄锁定复核（2026-10-06）

新增真实 AppContainer 负向用例：命令尝试把 workspace 内文件移动到同级 `outside` 目录，进程返回非零，源文件仍在且目的文件不存在。它证明受限进程自身不能通过移动操作把文件带出 workspace；这不约束拥有宿主权限的其他进程，也不代替并发宿主增删对象的清理策略。

根路径夹具扩展为在保留从卷根打开的祖先链和 workspace root 句柄期间，尝试移动直接 parent 与更高一级选定 ancestor；两次重命名均被 Windows 拒绝。释放 workspace root 后，只保留祖先链，再尝试移动该 higher ancestor 仍被拒绝。该测试实测了选定层级的句柄锁定行为；更高层级、其他文件系统/卷的行为仍不据此推断。

本轮最新完整验收：`cargo test -p piscis-kernel --locked -- --test-threads=1` 为 351 passed、0 failed、1 ignored；pool integration 25 passed，doc-test 1 ignored；`cargo clippy -p piscis-kernel --lib --locked -- -D warnings`、四文件 `rustfmt --edition 2021 --check` 与相关文件 `git diff --check` 均通过。ignored 项仍需真实 `$PISCIS_EVAL_DB`，不计作通过。

补充的租约期间宿主对象生命周期夹具：创建临时 AppContainer SID 并授予 workspace ACL 后，由宿主在租约内创建文件、目录及嵌套文件，核实临时 SID 均被继承；将其中的文件和目录树移到同卷 sibling 后，移动对象仍含该 SID；调用真实 `WorkspaceAclLease::finish()` 后，移动对象、嵌套后代及留在 workspace 的新文件均不再含临时 SID。该测试说明当前 Windows 本机同卷、租约按序结束的路径会清理新增与已移出对象上的 SID。它没有模拟 finish 枚举期间并发新增、跨卷移动/复制、其他文件系统或真实同时写入，不能推广到这些情况。

加入该夹具后的最新完整验收：`cargo test -p piscis-kernel --locked -- --test-threads=1` 为 352 passed、0 failed、1 ignored；pool integration 25 passed、doc-test 1 ignored；Clippy、四文件 rustfmt check 与相关文件 diff check 通过。真实数据库评估仍因缺少 `$PISCIS_EVAL_DB` 被忽略。

## 清理枚举窗口的确定性注入

把 cleanup 真实路径拆成内部 `finish_inner`，生产 `finish()` 只传 no-op；仅 `#[cfg(test)]` 暴露一个枚举完成后的注入点，不改变生产顺序。夹具在 `collect_current_handles` 已结束、SID 撤销和原 ACL 恢复尚未开始时，宿主新建工作区文件并立即移到同卷 sibling。实测文件在移动后仍带有临时 SID；`finish_inner` 随后恢复原 DACL，测试确认该枚举后创建且已移出的对象最终不再含临时 SID。此前同一 lease 夹具还验证枚举前创建的留存文件、文件移出、目录树移出、以及嵌套后代均在 cleanup 后清除临时 SID。

注入后的最新完整 Windows 验证：内核 `cargo test -p piscis-kernel --locked -- --test-threads=1` 为 353 passed、0 failed、1 ignored；pool integration 25 passed、doc-test 1 ignored；Clippy 与 rustfmt/diff checks 通过。此夹具只模拟一个同卷、单线程时序边界；不是多线程压力、跨卷、非 NTFS 或多用户测试。当前主机只有 C: NTFS 与 D: UDF（D: 无可用空间），不能进行跨卷可写运行。

句柄改造后的完整验证：`cargo test -p piscis-kernel --locked` 为 346 passed、0 failed、1 ignored；pool integration 25 passed，doc-test 1 ignored。新增 `acl_mutation_stays_bound_to_open_handle_after_path_replacement` 1/1 通过；`cargo clippy -p piscis-kernel --lib --locked -- -D warnings`、四文件 `rustfmt --edition 2021 --check` 与相关文件 `git diff --check` 通过。

## 未验证与剩余工作

- 新增的目录句柄枚举与相对 no-follow 子项打开已在 Windows 本机验证：`relative_child_open_ignores_workspace_ancestor_path_replacement` 1/1 通过；对照原目录与替代目录中的独有文件名，确认替换后的 workspace 路径不会重定向原句柄的枚举。这个夹具验证重命名后的确定性路径替换，不模拟持续并发写入。
- 更新后 `cargo test -p piscis-kernel --locked -- --test-threads=1`：347 passed，0 failed，1 ignored；pool integration：25 passed，doc-test：1 ignored。首轮默认并行完整测试中，既有 `windows_appcontainer_child_is_confined_to_explicit_directory` 的 Job Object 延迟子进程夹具检测到 marker（346 passed、1 failed）；该用例单独重跑 1/1 通过，随后串行全量重跑通过。此处保留失败记录，不把它描述为代码无关。
- 更新后 `cargo clippy -p piscis-kernel --lib --locked -- -D warnings`、四文件 `rustfmt --edition 2021 --check` 及相关文件 `git diff --check` 通过。

- `cargo check -p piscis-kernel --target x86_64-unknown-linux-gnu --locked` 未进入项目编译：本机 Rust target 缺少 `core` 标准库（rustc 报 `E0463`）。因此只以代码和测试描述 Linux unsupported 行为，不能声称 Linux 构建/运行通过；没有安装工具链组件。
- 尚未启动 AgentZ/Tauri 桌面验证用户可见工具目录和真实 agent session；未做 release/profile 打包、重启或 GUI 验收。
- 句柄相对目录枚举/子项打开、卷根锚定的祖先解析、确定性 ancestor path replacement、拒绝受限进程向 sibling 移动文件、选定 parent/higher-ancestor 重命名受阻、真实 `SetSecurityInfo` Access Denied 后恢复重试，以及同卷 lease cleanup 对枚举前/枚举后创建并移出对象的临时 SID 清理已有 Windows 自动化；测试层级以上的 ancestor、真正并发而非确定性插入的创建/删除、跨卷移动/复制、真实 worktree 并发写入、恶意并发 reparse/junction 替换、部分对象恢复后发生瞬时 API 故障时的恢复策略、低权限用户和多卷目录仍未验收。hard-link 静态别名已拒绝；此次句柄改造不等于解决上述剩余竞态。
- LSP 等外部服务器不由此 shell/code_run runner 托管；外部服务器进程/网络隔离不能从这两项工具测试推断。需按单独能力边界处理。

## 后续

下一步按验收清单复核支持边界和工具披露；受用户指示跳过的原生 GUI 验收继续记录为未验证，LSP/外部服务器、release 与跨平台运行也分别未验收。
