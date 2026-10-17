# AZ-020 existing WorkZ 单agent 计划IPC候选

仅修改已注册commands/chat_plan.rs与本证据；不改注册、Cargo、台账，不运行构建。允许既有 SOURCE_WORKZ 单agent会话；继续允许CodeZ/legacy。拒WorkZ-team、pool、heartbeat、remote未知来源，以及plain WorkZ附带team_id/pool_id或session.pool_session_id。缺失会话不ensure/create，损坏meta拒绝。

managed登记优先：load任务登记后使用真实resolve_task_workspace验证环境，登记workspace权威，不用free metadata替换。常规WorkZ无登记/cwd时仅existing session.workspace或scopeproject，通过与主turn相同require_binding策略检查，不能rendererworkspace参数选路径。

free scope通过host workz_overview.free_dir规范路径与opaque scopeproject匹配且scope来源RestoredWorkspace确认；当前scope enum无free subtype，未依赖renderer布尔值。meta.cwd必须持久存在、绝对本地路径，Session.workspace_root也必须存在且canonical相等，缺失/不一致明确恢复错误，绝不猜overview.default_work_dir或project fallback。非free无登记但存在cwd拒绝，不能用metadata绕scope限制。

实际主turn的ApprovalHooks对workspace!=project要求登记binding；本IPC保留相同策略，因此外部free cwd未登记仍会拒绝。当前主turn未见写Session.workspace_root而前端仅保存meta.cwd，历史free session可能缺一致workspace，同样明确unsupported/recovery。本包不放松resolver伪造不同PlanBinding，也不声称free外部目录已全面支持；需后续正式hostfree执行政策及持久workspace接线。

新增2夹具（未运行）：真实memoryDB测试plainWorkZ/source/team/pool/unknownmissing/malformedmeta；temp本地目录测试free可信标志、绝对匹配、缺失workspace、错cwd、相对路径拒绝与managed优先交由resolver。已有CodeZ-only内部helper夹具保留，生产选择改为existing_task，统一共3test。rustfmt与限定diff-check已实际exit0；主线程负责最新编译和实跑，GUI/模型未验收。

Final root39340 actual registered host compilation and commands::chat_plan tests:3 passed0 failed0 ignored,2m20s compile then0.25s fixtures. Validates CodeZ compatibility, existing single-agent WorkZ sources/metadata rejection, strict free-cwd consistency and no path guessing. It does not demonstrate native IPC execution, GUI/model journeys or working external free directories.
