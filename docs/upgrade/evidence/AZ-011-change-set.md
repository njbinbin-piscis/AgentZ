# AZ-011 完整工作区变更集

2026-10-05。实现位于agentz-task-state::changes；新增agent_task_change_set IPC及TypeScript调用封装。入口以稳定task id读取持久登记、验证实际worktree，再使用登记的base SHA；调用者不能换根或改基点。

采集当前工作区相对冻结基点的净变化，来源标记committed/staged/unstaged/untracked（非忽略），保留旧路径、Git状态及binary标记；Git name-status/numstat使用NUL格式。明确净变化语义：已提交又恢复为基点的文件不会作为最终差异列出。删除、改名、二进制、中文/空格路径已在真实Git夹具覆盖；tab/newline解析使用平台无关NUL单测，Windows不宣称能创建不合法文件名。

结构化错误code/message保留environment_invalid、git_launch_failed、git_failed、unsupported_path、invalid_git_output、file_read_failed和worker_failed。非法ref不返回空成功；非UTF8路径明确失败。运行时目录.agentz/.codez/.piscis不进入用户改动，排除路径单独返回，其他隐藏文件不按IDE索引规则过滤。

已验证：领域crate 9项测试通过，包括真实Git变更集、暂存区未变、非法ref及NUL解析；clippy --all-targets -D warnings通过。前端44项回归、typecheck与修改文件lint通过。

首轮夹具失败原因：多个待删除/改名文件具有同一blob，Git可合法将另一个同内容文件识别为改名。夹具改为独特blob后通过。首轮宿主集成引用构建期间修改前的领域库，列出.agentz/journal.db；最新运行时排除逻辑在领域测试通过，宿主重跑尚待完成。不是将失败记录抹除。

范围限制：旧branch-only API与评审界面仍未迁移（AZ-013），不宣称用户已经在现有评审UI看见全部变更；一致性锁定、应用前再校验、归档/合并预检属于后续任务。当前仅只读采集，不修改index或用户文件。

最终宿主验收：2026-10-05，task_journal_routing集成通过。登记入口实际返回A环境的created.txt与shared.txt、冻结base SHA，排除运行时数据库；B分支失效返回结构化environment_invalid。最新领域9项测试与clippy严格检查通过。原生评审文件核对仍属AZ-013验收。
