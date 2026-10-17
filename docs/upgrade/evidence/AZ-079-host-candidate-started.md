# AZ-079 宿主候选回归启动

2026-10-05，Codex。核对台账、核心4/4证据及无运行构建；用户继续68项，GUI暂跳过未验收；无适用AGENTS.md。主实现仅AZ-079，未覆盖内核原秘密保存修改。

新增candidate_repeated_edits_undo_to_turn_start_through_host_hooks：实际ApprovalHooks/FileJournal在同一回合连续写两次，然后Undo All应恢复original且不可重放。该用例明确ignore并注明需要AZ-079候选，固定v0.8.68的默认测试不冒充已修复。

启动cargo test -p agentz-desktop --config .cache/upgrade-kernel-local.toml --test approval_execution --test task_journal_routing -- --include-ignored，session8545，日志.cache/upgrade-undo-host-cycle-034.log。本记录时尚未完成，未计通过。命令finally恢复Cargo.lock原字节并核对hash；运行期间不启动其他Cargo依赖更新操作。

后续先检查此进程与日志，避免重复构建。候选回归与固定可分发依赖接入分别记录；不发布、部署或对外发送。
