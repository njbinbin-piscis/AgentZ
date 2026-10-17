# AZ-008 后端执行环境权威与校验

每轮chat_send根据session/task登记解析环境，前端workspace_dir不能覆盖已有绑定。缺少强制隔离绑定、失效目录、分支变化、不同Git common dir或不在worktree登记中时返回错误；不回退主项目。

队列等待前校验一次，在获得执行槽后再次验证，期间改绑也拒绝。Git校验子进程kill_on_drop，超时不留下校验进程。未绑定的CodeZ和自由目录保持原兼容路径；历史隔离任务需要明确恢复绑定。

验证：`cargo check -p agentz-desktop --locked`通过（1m）；`cargo test -p agentz-task-state --locked`7项通过。涵盖跨项目、不同仓库/分支、未登记/重定向、损坏记录、环境唯一性、空格路径；真实Git夹具确认合法worktree后切分支被拒绝，移走目录后解析失败且登记保留。

这不是原生桌面E2E，也不是系统沙箱证明；工具运行期间的用户分支变更与系统权限边界仍属于后续生命周期/权限任务。前端稳定session UUID绑定由AZ-009接入。

复盘：登记与验证在独立领域crate测试，宿主仅采集真实Git/FS事实并调用相同规则；不会使用UI自报的仓库身份。
