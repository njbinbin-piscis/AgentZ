# AZ-018 宿主身份绑定事件查询设计

2026-10-07，只读设计。主线程已报告 14 项 Run 测试与严格 Clippy 通过；本代理没有重新运行。本文不修改产品、台账、构建或 AZ-020 当前宿主服务，不能表示宿主断流游标已经交付。

## 文件归属与集成顺序

| 原子包 | 独占文件 | 交付边界 |
| --- | --- | --- |
| HOST-CURSOR-01 | 新 `src-tauri/src/runtime/run_events.rs` 及内嵌测试 | 查询 authorizer、managed/direct fresh 校验、范围绑定及 DTO；不改 chat_turn |
| HOST-CURSOR-02 | 新 `src-tauri/src/commands/run_events.rs`、新 `src/services/tauri/runEvents.ts` | 单次分页 IPC 与严格错误映射；不可直接接收文件路径或数据库路径 |
| HOST-CURSOR-03 | 主线程独占 `src-tauri/src/state.rs`、runtime/commands mod.rs、lib.rs | host 注册上下文及 IPC 注册；与 AZ-020 接线串行，避免共享修改 |
| HOST-CURSOR-04 | 主线程重新认领 `crates/agentz-task-state/src/run.rs` | 真正只读打开现有数据库、只读 event query、capture coverage DTO；变更后重审契约指纹 |

开始前逐文件确认认领；依赖项保持 preparing，仅主线程统一测试。存储已有 `replay_events(&mut self, run_id, attempt_id, after, limit)` 可复用查询语义，但 `RunStore::open` 会创建目录/数据库和初始化 namespace，因此**不能**把直接调用 open 的 IPC 命名为无副作用只读读取。

## Renderer 输入与宿主授权

建议输入：`contextHandle`、`runId`、`attemptId`、可选 cursor、limit。contextHandle 是宿主登记项目/会话上下文的不可猜测句柄，映射到 canonical project、任务/session 和可读身份；不接受 `projectDir`、`workspaceRoot`、environmentId、数据库路径或本地 home 覆盖。句柄不存在/已撤销先拒绝，不能先枚举磁盘探测记录。

注册上下文必须来自已经验证的用户项目选择/宿主执行上下文，不可由 renderer 提交任意目录后自动签发。现有 `data_scope::require_project_dir` 仅验证目录存在，**不是**项目授权白名单，不能单独作为新读取接口的安全依据。AppState 当前也不能被假定已有此句柄注册表；HOST-CURSOR-03 必须显式实现生命周期、窗口归属及撤销。只读请求不得为了授权而创建 `.agentz` 或打开全局 home 数据库。

宿主从授权根目录的既有 runs.db 读取 Run，核对 taskId 属于句柄任务/session，再从 attempts 证明请求 Attempt 属于 Run；task/environment 由数据库派生。读他人 Run、跨 session、跨项目、猜测 Attempt 均拒绝。旧 Attempt 的合法历史读取可允许，执行恢复和审批签发不由本接口提供。cursor 是分页位置而非权限票据，客户端修改 cursor 不能扩大身份范围。

## Managed 与 direct 的 fresh 规则

Managed：从该授权 project 的 TaskEnvironment 得到 workspace，核对 Run.environment_id；调用现有 `agent_task::validate_task_environment`/`resolve_task_workspace` 检查 canonical 路径、Git common-dir、worktree 注册及分支。不得采用 renderer 的 workspace。环境已清理或交接改变则返回 `environment_changed`，不退回 project/home 读取。

Direct：用授权句柄的 canonical project/workspace 和 Run.task_id 调用现有 `approval::resolve_binding`，取新 direct fingerprint；核对 Run.environment_id 的 `direct:{fingerprint}`。该函数默认 attempt_id=run_id，查询授权时不能据此覆盖历史 Attempt；实际请求 attempt 始终来自 attempts 关联。direct Run 没有固定 TaskEnvironment，不因此伪造登记；分支切换造成指纹不同即拒绝正常 live replay。

如产品需要分支已变或环境删除后的历史记录访问，应独立设计只读 archive/history 授权，并明确标记 stale environment；不得弱化 fresh live query，也不得让历史读取票据复用为执行/审批权限。当前包可先失败关闭，不能声称该历史产品路径已实现。

查询时短时持有宿主 environment access 读取租约阻止宿主 cleanup/relocation，分页完成后再作 fresh 重校验；人类或外部进程的 Git 改动不受该内存租约控制，前后比较是检测机制而非 OS 沙箱。校验变化则丢弃该页并返回确定错误。SQLite读取放 spawn_blocking，不能跨 await 持有数据库事务或全局锁。

## 分页协议与旧历史覆盖标记

输出建议：`runId`、`attemptId`、`environmentId`、events、`nextCursor`、`hasMore`、`execution`、`delivery`、`captureCoverage`。每页 1..100 条且序列化总字节限制 1 MiB；库的 1000 条上限不是 IPC 推荐上限。若下一条单独超出页上限，明确返回 `event_too_large` 而非无限重复空页。按 snapshot 查询 limit+1 或等效 bounded 方法判断 hasMore；不猜测下页状态。cursor 仅在消费端成功应用页面后推进；重发页以 event_id/sequence 去重，不触发副作用。

存储 cursor 必须精确匹配权威 task/run/attempt/environment；sequence 为 scoped exclusive position。不同 Attempt 不共享 cursor，retry 返回新 Attempt 身份，由调用方显式开始其流。Stop只改变执行状态，读取终态历史仍允许；进程重开可从已应用 cursor 重放，不能因此自动重启执行。事件尚未存在时可返回空页与当前 Run 状态，但不得说任务从未运行。

旧 v1 激活前历史未捕获：建议 captureCoverage 枚举 `from_attempt_start`、`partial_history`、`unknown_legacy_or_uncaptured`。只有 scoped sequence 1 是真实 `run.created` 或新 Attempt 的 `run.retry` 才可标 from_attempt_start；首条是中途 execution/delivery 事件标 partial_history；空历史标 unknown。旧 Attempt 无事件和未执行不能仅凭空页区别。该标记是存储证据的解释，不伪造 migration 事件，也不承诺旧历史完整。

## 错误与必须测试的拒绝边界

固定错误码：`context_revoked`、`run_not_in_context`、`attempt_not_in_run`、`environment_changed`、`cursor_scope_mismatch`、`cursor_ahead`、`event_gap`、`capture_not_initialized`、`unsupported_schema`、`event_too_large`、`invalid_page_limit`。对外不返回任意绝对路径/SQL/底层文件内容。不存在和未经授权不能用错误差异泄露其他项目状态。

必须夹具：恶意renderer传home/路径参数被协议拒绝且没有磁盘创建；授权A句柄猜B Run拒绝；managed worktree替换/branch变化；direct分支指纹变化；同Run旧Attempt合法页但crossAttempt cursor拒绝；页重复应用不重复sideeffect；停止后重开读取terminal页；namespace未激活与旧v1空capture不误判；future schema与gap明确失败；page字节限制、超大单事件和hasMore边界；租约撤销与请求完成竞态。需要真实桌面及断流旅程时另外验收，不能用 IPC 模拟代替。

## 尚未实现的产品边界

当前事件原语仅存储 create/execution/retry/delivery 生命周期，不包含全部模型token/tool流；新分页接口不能宣称可恢复所有聊天显示。实时通知应在事务 commit **之后**发送只读唤醒信号，由消费端基于cursor重新查询；通知丢失可轮询恢复。权限、持久cursor消费、首次载入UI及legacy coverage接线都待实现。本文没有启动这些包，也不影响主线程当前 AZ-020 服务开发。
