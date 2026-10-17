# AZ-015 保留式任务归档

日期：2026-10-05，Asia/Shanghai。负责人与审核：Codex 自检。按对象约束与失败恢复范围完成；未宣称原生桌面验收或完整恢复旅程通过。

旧 force remove / branch delete 入口改为完整 checkout 保留：验证持久任务绑定、真实 Git 归属、宿主管理路径和 task 分支，保存 AZ-014 原始源码快照，再通过 Git worktree move 移至主项目 `.agentz/retained-worktrees/<archive-id>`。忽略文件、运行时 journal、用户 index 和分支随完整目录保留；不使用遗漏源码快照来授权删除，不递归删除原始用户数据。暂不提供永久清除。

执行与 journal 操作持有共享租约，归档必须取得独占租约；chat 在排队前取得租约。当前为宿主进程级保守互斥，其他项目的执行也会阻止归档，环境级并发细化留给后续路线图。归档/源码快照/恢复的阻塞 worker 自己持有租约，避免调用方取消等待后提前解锁。外部编辑器、终端和其他进程不在此租约内，不声称安全沙箱。

schema1 receipt 在移动前记录 planned、成功后记录 archived；错误返回 stage、message、archive_id 和已存在的 retained_root。锁定 worktree 移动失败保留源目录和已发布快照。移动后元数据失败仍给出保留目录，不伪报成功。任务列表按旧绑定根过滤已迁移 checkout，旧会话环境失效而不自动重定向。正式重新接手/绑定仍需独立恢复流程。

界面将“丢弃任务”改为“归档任务”，允许归档未提交内容；确认说明保留目录、历史与分支。成功或部分移动失败显示保留路径并禁用后续目录操作。稳定 taskId 必须传入；旧客户端缺少身份时明确拒绝。

已执行验证：

- `cargo test -p agentz-task-state --locked --target-dir target/task-state-verification`：12 项通过，末次 18.87s。真实 Git 案例覆盖主目录/其他任务/任意分支拒绝、锁定目录移动失败、ignored/runtime/index/HEAD/branch 保留、receipt 持久状态；租约互斥与释放有独立测试。
- `cargo clippy -p agentz-task-state --locked --target-dir target/task-state-verification --all-targets -- -D warnings`：通过，39.58s。
- `npm test -- --reporter=dot`：9 文件、53 项通过，包含 9 项评审组件用例和部分归档失败路径。
- `npm run typecheck`：退出 0；修改文件 ESLint：退出 0。
- `cargo check -p agentz-desktop --locked`：通过，2m34s；其后新增 worker 租约所有权改动需再检查，不将先前通过覆盖未检查改动。

正在运行：`cargo test -p agentz-desktop --test task_journal_routing --locked`（会话 81442）。新增真实 kernel hooks 写入 journal 后迁移 checkout、重新读取历史及拒绝旧 session 的案例；结果尚未返回，不计入通过。

最新宿主检查已排队：会话 72444，`cargo check -p agentz-desktop --locked`，日志 `.cache/upgrade-check-cycle-004.log`；等待上述集成构建释放 Cargo 锁。后续先收集这两个会话，避免重复启动编译。

补充故障验证：新增在完整目录已移动后阻止最终 receipt 发布的真实 Git 场景，返回 `receipt_after_move`、archive_id 与实际 retained_root，ignored/journal 字节保留，源码快照可读取，磁盘 receipt 仍如实为 planned，旧路径已消失。定向清理测试通过（15.62s），未将此部分失败冒充 archived 成功。

最终结果：会话 81442 退出 0，真实 journal 集成 2 项通过（7.76s，编译 7m59s）；迁移后实际 journal 历史 before/after 内容保留，旧任务 journal 路由拒绝失效路径，主目录文件不变。会话 72444 退出 0，最新宿主检查通过（4m12s，含等待锁），日志 `.cache/upgrade-check-cycle-004.log`。

未验证：原生 UI/IPC 点击、桌面运行中归档拒绝、跨平台、真实模型、归档后正式恢复任务接手与发布。AZ-009/AZ-013 原生验收仍保持 review。

复盘：Git for Windows 参数需规范化 DOS/UNC 路径；源码快照与完整 checkout 保留是不同恢复范围，不能把源码遗漏列表当作已备份运行时。测试与构建的源版本变化分别记录，不把构建中的后续修改算作已通过。
