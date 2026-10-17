# AZ-010 明确任务起点

2026-10-05，Asia/Shanghai，Codex 实施。取消宿主 `ensure_git_repo` 自动 init 和 Initial commit。非 Git、无 HEAD 的仓库返回明确错误与“使用本地任务/显式初始化或提交”的提示；不把用户已暂存内容偷渡进初始提交。

WorkZ 创建隔离任务前提供两种选择：

- 包含当前本地源码：以当前 HEAD 为冻结 base SHA，捕获最终普通源码字节，包含暂存、未暂存、删除及非忽略新增文件。独立临时 index、raw hash-object、commit-tree 保存一个新任务起点，不移动源 HEAD、不提交源 index；不复制未跟踪的 ignored/runtime 文件。所选内容成为任务的独立初始提交，而不是保留原暂存分类。
- 仅提交、不含本地改动：可输入分支或提交，空值为 HEAD。界面选择即明确排除本地修改；旧客户端或未经明确排除的调用在脏目录返回 TASK_START_LOCAL_CHANGES，不能静默忽略。

冻结实际 base/start SHA；commit SHA 不能充当合并 checkout 目标，SHA/非本地分支起点的交付目标使用当前本地分支。工作源码只允许当前 HEAD 基点。detached 源目录仍需明确命名交付目标，不能声称已有完整合并预检；AZ-016 将负责目标/脏状态/事务安全。

复用 AZ-014 捕获和原始恢复写入，拒绝 symlink/submodule/不支持条目；读取状态使用 GIT_OPTIONAL_LOCKS=0，防止只读预检刷新用户 index。起点 ref `refs/agentz/starts/<nonce>` 与 schema1 `.agentz/start-snapshots/<nonce>.json` 保存模式、输入 ref、冻结对象及遗漏路径；checkout 失败保留 ref 并报告，不递归删文件。既有目标目录明确拒绝，登记失败仍保留 checkout 供恢复。

首次环境创建失败不会把未创建的乐观 session 留成必须“恢复旧隔离任务”的状态；可以调整起点或关闭隔离再启动。已有登记的任务仍复用原环境，不按界面新起点重定向。

验证记录：

- 新增两项真实 Git 案例：非 Git 不产生 .git；unborn 仓库暂存文件/index 原样保留且无 HEAD；dirty/staged 目录未确认排除时拒绝；显式提交模式只复制提交内容；working 模式复制最终 CRLF/二进制/中文文件并保留原 HEAD、index 原始字节、源分支与文件；历史 SHA 起点冻结、交付目标仍为 main；旧 SHA + local source 拒绝；既有目标不覆盖。
- 14 项领域回归通过（16.92s），包含 archive/cleanup 的共享捕获回归。其后增加禁止 optional index 刷新，最终回归会话 2639 待收集。
- 前端 typecheck 退出 0，9 文件/54 项测试通过（9.17s）；包含起点模式/参考值传递，旧 A/B 路由与评审用例仍通过。
- 宿主 `cargo check -p agentz-desktop --locked` 退出 0（1m51s），日志 `.cache/upgrade-check-cycle-005.log`；后续 optional-lock 环境变量改动仍需补查，不扩张先前通过范围。
- 生产构建会话 38706 正在运行，日志 `.cache/upgrade-build-cycle-005.log`；不得重复启动。修改文件 ESLint 会话 71157 待收集。

最终代码检查补充：会话 2639 退出 0，14 项领域测试通过（16.71s），严格 clippy 通过（3.84s），已含 GIT_OPTIONAL_LOCKS=0。会话 3256 最新宿主检查退出 0（34.80s），日志 `.cache/upgrade-check-cycle-005-final.log`。ESLint 退出 0，WorkZ 保留 4 项既有 hooks 依赖 warning，不记作“零告警”。台账验证及隔离的升级工具回归通过。

最终生产构建：会话 38706 退出 0，`tsc && vite build` 成功（Vite 7m）；日志 `.cache/upgrade-build-cycle-005.log`，仍有既有大 chunk/动态与静态导入混用提示。按本项创建起点与 Git 数据不被隐式更改的范围自检完成，不将生产构建冒充原生或发行验收。

未验证：原生起点选择/失败重试点击、真正模型任务、跨平台、完整交付目标事务。恢复 taskId 的正式流程和忽略资源复制策略不在本项范围。当前尚未将生产构建计为通过。

复盘：普通 Git checkout 可能执行换行/内容过滤，因此与源码归档共用原始 blob 写入；scratch index 放在系统临时目录，避免捕获主目录时将新 scratch 文件误判为遗漏集合变化。初版 Rust lifetime 与 DropdownOption 字段检查失败已修正；失败未算作验收。
