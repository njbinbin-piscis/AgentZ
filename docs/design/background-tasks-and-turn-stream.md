# 后台任务、回合状态与消息流:调查结论与重设计

状态:草案,待评审。日期:2026-10-27。

本文基于对当前代码的通读(行号以 2026-10-27 工作区为准),记录已确认的缺陷、尚未定位的问题,以及建议的目标设计和分阶段实施计划。凡标注"已确认"的结论都有代码证据;标注"推测"的需要埋点或复现才能定论。

---

## 1. 背景:近期暴露的问题

| # | 现象 | 当前判断 |
|---|------|----------|
| P1 | 用户发消息报 `task queued/running or goal mutation active`,界面看不到任何运行中的会话 | 已确认:后台任务完成后自动发起的续跑回合(`host-bg-*`)占着会话门禁,但对前端不可见(1.0.27 已加 `host_turn_started` 缓解) |
| P2 | 后台任务启动了 dev server 后,agent 在同一回合里输出一句话就永久卡住;关掉 server 后 agent 继续迭代 | 高度怀疑:前台命令结束后在等管道 EOF,而写端被 Job 之外的进程(server)持有。已加 5 秒上限,未复现验证 |
| P3 | 点停止要近一分钟才停下 | 未定位。代码上可确认的上限约 8s(工具宽限)+10s(取消确认),其余耗时来源不明 |
| P4 | 助手文字被工具卡片从词中间切开,后续段落之间反而没有工具 | 未定位根因(见 §4.3)。已排除"切换会话"(用户确认未切换) |

这些问题有一个共同点:**状态分散在多处、各自推进,没有单一事实来源**。进程状态在内核注册表,回合状态在门禁 + 前端内存集合 + 数据库会话行,消息流位置靠前端计数器推断。任何一处不同步都会表现为"卡死"或"错乱"。

---

## 2. 现状调查

### 2.1 进程启动路径

| 路径 | 位置 | 输出 | 进程树控制 | 等待 EOF 是否有上限 |
|------|------|------|-----------|---------------------|
| Windows 沙箱(`shell` / `code_run`) | `vendor/piscis-engine/piscis-kernel/src/proc_windows_sandbox.rs` `spawn_sandboxed_command` L451–638 | 匿名管道,`CreatePipe` 时 `bInheritHandle: true`(L517–524),子进程用 `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` 只继承三个句柄(L538–559) | Job + `KILL_ON_JOB_CLOSE`(L505–515) | 原先无上限(`t.join()`);现在 `CAPTURE_DRAIN_TIMEOUT = 5s` |
| Unix 后台(`shell`/`code_run` 仅允许 `background: true`) | `tools/background_process.rs` `spawn_std_detached` L129–214 | 管道 | `setsid` 新会话,`killpg` | **无上限**:读线程 `join()`(L207–212) |
| `process_control` 启动 | `tools/process_control.rs` L204–237 | 管道或 null | 无 Job;无注册表时完全不跟踪 | 无上限 |
| IDE 终端 | `src-tauri/src/commands/ide.rs` L1706–1895 | PTY | Windows Job | 不涉及 |
| 其他(git、LSP、rg、远程等) | `piscis_kernel::proc::tokio_command`/`std_command`,少量直接 `Command::new` | 各异 | 无 | — |

**已确认的结构性问题**:沙箱的子进程侧管道句柄在父进程中短暂处于"可继承"状态。沙箱自己的 `CreateProcessW` 用了句柄白名单,但同一进程里其他地方用 `std::process::Command`(Rust 标准库在 Windows 上 `bInheritHandles = TRUE` 且不使用白名单)启动的进程,只要恰好落在这个窗口内,就会继承这些写端。被继承者不在命令的 Job 里,命令结束时杀不掉它,于是读端永远收不到 EOF。

这与 P2 的"关掉 server 才继续"一致,但时序(server 是否恰好在那个窗口启动)尚未证实。另一种同样会导致 P2 的情况是:命令自己把一个进程启动到 Job 之外(例如通过 `start`/服务管理器脱离)。

### 2.2 后台任务模型(内核)

核心在 `vendor/piscis-engine/piscis-kernel/src/background.rs`:

- `BackgroundRegistry`:每工作区默认最多 4 个(`AGENTZ_BG_MAX_PER_WORKSPACE`),默认 `max_runtime` 3600s(`AGENTZ_BG_MAX_RUNTIME_SECS`)。
- `BgEntry`:会话、工作区、标签、`notify_on`(`Exit`/`Ready`)、`ReadySpec`(`Port`/`LogSubstring`)、kill 标志、stdout/stderr 的 `SharedStream`(256 KiB 内存环 + 8 MiB 日志文件 `.agentz/logs/bg/{id}.log`)。
- 事件:`BgEvent::Ready` 来自 `probe_ready`,`BgEvent::Terminal` 来自执行线程结束。
- **注册表条目永不删除**(没有 `remove`),只是状态变为终态。

agent 可见的工具(描述摘自 schema):

| 工具 | 行为 |
|------|------|
| `shell` / `code_run` | 前台默认 120s / 60s,上限 300s;"超时后转入后台注册表,不杀";`background: true` 立即后台;可带 `ready_port`/`ready_log`/`notify_on` |
| `wait` | 宿主侧一次性等待后台任务结束或就绪,默认 120s,上限 600s;也可纯延时 |
| `process_wait` | 单任务等待,默认 60s,上限 600s |
| `process_output` | 按游标增量读取输出 |
| `process_kill` | 杀进程树,最多等 30s 确认 |

这套工具的**能力**已经接近目标设计,问题在于语义不清和上层策略(见 2.4、2.5)。

### 2.3 时间参数一览

| 机制 | 默认 | 位置 |
|------|------|------|
| 单个工具调用超时 | 120s | `loop_.rs` L55 `TOOL_TIMEOUT_SECS` |
| shell 前台等待 | 120s(≤300s) | `shell.rs` L21–23 |
| code_run 前台等待 | 60s(≤300s) | `code_run.rs` L14–15 |
| 取消宽限 | 8s | `loop_.rs` L58 `CANCEL_GRACE_SECS` |
| 取消确认 | 10s | `post_turn.rs` L527–528 |
| 回合总时长 | 未设置时约 1 年 | `chat_turn/mod.rs` L856–858 |
| 后台最长运行 | 3600s | `background.rs` L28 |
| 续跑停泊 | 30min,每 2s 轮询 | `background_continuation.rs` L30 |
| 每会话自动续跑预算 | 6 次 | `background_continuation.rs` L28 |

**已确认的冲突**:工具调用超时(120s)与 shell 前台默认等待(120s)相同。工具层先超时时,沙箱工作线程可能仍在 `wait_foreground`/`finish` 中,前台转后台的交接可能被打断。

### 2.4 后台任务完成后的自动续跑

`src-tauri/src/runtime/background_continuation.rs`:

1. `on_event` 发 `background_task` UI 事件,再异步 `handle_completion`。
2. `notify_on == Ready` 时就绪触发续跑;任何任务结束都会触发续跑。
3. `try_claim("{bg_id}:{kind}")` 去重,`claim_budget` 扣预算(每会话 6 次)。
4. 生成合成提示 `<background_task_completed>`,持久化为 pending,`dispatch_when_idle` 轮询门禁空闲后以 `task_key = host-bg-{bg_id}` 发起一个完整的 agent 回合。

已确认的缺陷:

- **续跑回合就是一个普通回合**,持有会话门禁直到结束,期间用户消息被拒(P1)。1.0.27 起前端通过 `host_turn_started` 显示运行中并排队,但只有 CodeZ 处理了这个事件,WorkZ 等界面没有。
- **一个服务消耗两次预算**:`ready` 和 `terminal` 是两个 claim key,各扣一次。
- **重启恢复的去重键不一致**:`restore_pending` 固定用 `terminal`(L336–337),可能重复续跑。
- **服务也会触发续跑**:dev server 被杀(哪怕是用户手动杀)也会开一轮新回合,这正是 P1 那次的触发方式。
- 应用退出时没有清理后台进程(未找到对 `background_runs` 的全局 kill),下次打开时数据库里标为 `interrupted`,但操作系统进程可能还活着。

### 2.5 回合状态:三份互不同步的"事实"

| 状态 | 所在 | 谁写 |
|------|------|------|
| 门禁 `TaskActivity` | 后端内存,`(项目, session_id)` 一把锁 | `chat.rs` 回合入口、心跳、计划变更 |
| 会话行 `status = running` | 项目数据库 | 回合开始/结束;读取时用 `is_active` 修正"数据库说在跑但门禁空闲"这一个方向 |
| `SessionTurnBookkeeping.running` | 前端内存 | 只在前端自己调用 `chatSend` 时设置 |

前端决定"排队还是直接发送"只看自己那份。任何后端发起的回合(续跑、心跳)、页面刷新、多窗口,都会让前端认为空闲而直接发送,然后被门禁拒绝。1.0.27 的 `host_turn_started` 只补了续跑这一种来源。

### 2.6 消息流(CodeZ)

- 文字增量:`text_delta` 先进 `streamPendingRef`,每 80ms 由 `flushStreamDelta` 追加到最后一条助手消息;`streamLenRef` 在收到时立即累加(`useAssistantSession.ts` 约 L754–759)。
- 工具位置:`tool_start` 时 `upsertToolStep(prev, evt, streamLenRef.current)` 记录 `textOffset`(约 L761–763),**没有先 flush**,并且偏移量是在 React 执行 updater 时才读取 ref,而不是事件到达时。
- 渲染:`AssistantMessageList.tsx` 对运行中的最后一条消息使用实时 `toolSteps`(替换已持久化的工具);`ToolTrace.tsx` `interleaveTools` 用 `min(text.length, offset)` 切分文字。
- 历史:`src-tauri/src/commands/session.rs` 合并相邻助手行时插入 `\n\n`,偏移量用 Rust `chars().count()`(Unicode 标量),前端切分用 UTF-16 下标。
- 对照:WorkZ(`useWorkzSessionStore.ts` L738–768)在任何非 `text_delta` 事件前先 flush,并用已提交文字的 `length` 作为偏移;BotZ 逐事件同步追加,都没有这个"计数器领先于已显示文字"的结构。

---

## 3. 参考模型:Cursor 的 shell 工具约定

以下是 Cursor 对 agent 暴露的 shell 工具的外部约定(内部实现不在本文范围):

1. **"等多久"和"进程活多久"是两个参数。** 每条命令带"最多阻塞多久"的值,超过后命令转入后台继续跑,不会被杀;启动服务直接设为 0。
2. **输出写入文件,而不是靠管道读到 EOF。** 每个终端的输出持续写入一个文件,agent 随时读取。调用方从不需要"等管道关闭"才能返回。
3. **监视是主动且有上限的。** 有专门的等待工具:等某个后台任务结束,或等输出出现某个模式,自带超时。也可以只在输出匹配时通知。
4. **结束时通知,不自动开回合。** 后台任务结束发一条通知,由正在进行或下一次的 agent 回合消费,不会自己抢占会话。
5. **停止是显式操作。** 长时运行进程不会因等待超时被杀;需要硬上限时由 agent 在命令本身设(如 `timeout`),或主动结束。

AgentZ 已经具备第 1、3、5 条的大部分能力,缺的是第 2 条(底层仍依赖管道 EOF)和第 4 条(结束即自动开新回合)。

---

## 4. 问题定位状态

### 4.1 P2:前台命令卡死(已缓解,待验证)

- 已做:`finish()` 在 Job 清空后最多再等 5 秒 EOF,超时则返回已捕获输出并记 warn(`proc_windows_sandbox.rs` `join_capture`)。
- 未做:`spawn_std_detached` 的读线程 `join()` 仍无上限;句柄继承窗口仍在。
- 验证方法:复现时检查日志中是否出现 "pipe still held open by a process outside the command's Job"。

### 4.2 P3:停止耗时(未定位)

可确认的上限约 18 秒,用户观察约 60 秒。候选:

- 一批并发只读工具(`READ_TOOL_MAX_CONCURRENCY`)或串行写工具,每个都消耗自己的宽限期;
- 不检查取消标志的工具(等待子任务、`wait`/`process_wait` 的阻塞等待)只能等到宽限期结束;
- 前台命令结束后的 `finish()`(root 退出 5s + Job 清空 5s + EOF 5s)在宽限期之外执行;
- `cancellation_completion.wait_requested` 等满上限。

需要埋点:取消请求时间、每个在途工具的名称与结束时间、`agent turn finished` 时间。

### 4.3 P4:文字被工具切开(未定位)

已排除:

- 切换会话(用户确认未切换)。
- 后端事件乱序:内核按"本轮全部文字增量 → 工具开始"顺序发送(`loop_.rs` `llm_call_unified` L2228–2286,`ToolStart` 在 `execute_single_tool` L2505–2511),宿主收集器按序转发(`post_turn.rs` L323–435)。
- 80ms 合并导致的偏移领先:它只会让工具**暂时**排在已刷新文字的末尾,刷新后 `text.length` 追上偏移,位置自动恢复正确。这是闪烁,不是持久错位。

截图的持久错位要求偏移量**小于**它真实的位置,即:文字里有一段内容没有计入 `streamLenRef`,或者 `streamLenRef` 在回合中途被归零而文字保留。代码中 `streamLenRef` 被归零的位置只有回合开始、`host_turn_started`、会话切换/新建三处,文字被整体替换的位置有 `syncMessagesFromDb`(回合结束、续跑结束、错误路径)。尚未找到能在不切换会话的运行中回合里触发它们的路径。

待验证的假设:

1. 错误路径:用户在回合进行中发送的消息被门禁拒绝后,`runTurn` 的 `finally` 在"队列为空且正在查看"时调用 `syncMessagesFromDb`,把运行中回合已持久化的内容加载进来,但运行中回合的计数器没有同步。
2. 两个生产者向同一会话发事件:例如委托/子任务工具的事件与父回合交错。
3. 回合中途的模型重试或降级重新流式输出,导致计数与显示不一致。

需要埋点:为每个 `agent_event` 记录序号、类型、`streamLenRef`、最后一条消息 `text.length`、当前 `taskKey`,以及所有 `setMessages` 整体替换的调用点。下次复现时导出这段日志即可定论。

---

## 5. 目标设计

### 5.1 原则

1. **后端是唯一事实来源。** 回合是否在跑、在跑哪个、后台任务状态,都由后端推送快照;前端只渲染,不推断。
2. **等待与生命周期分离。** "agent 等多久"永远不决定"进程活多久"。
3. **永不无界等待。** 任何等待(锁、EOF、进程退出、回合)都有上限和可观察的超时结果。
4. **后台事件是输入,不是触发器。** 后台任务结束产生通知,由回合调度器决定何时、以何种方式交给 agent,而不是直接开一个新回合抢占会话。

### 5.2 进程层

- **输出一律落盘**:每个进程的 stdout/stderr 直接重定向到日志文件(Windows 用文件句柄作为子进程标准句柄,Unix 用 `File`),宿主通过跟随文件读取。前台命令"完成"的判定只看进程(Job)状态,不再依赖管道 EOF。这从根上消除 P2 一类问题。
- **句柄继承收口**:所有启动进程的地方统一走 `piscis_kernel::proc` 的封装;Windows 上统一使用 `PROC_THREAD_ATTRIBUTE_HANDLE_LIST`,并在一个进程内全局锁住"创建可继承句柄 → 启动 → 关闭"这一窗口,禁止裸 `std::process::Command`(CI 加 lint/grep 检查)。
- **进程树归属**:Windows 一律 Job,Unix 一律新进程组;记录 pid/pgid 以便应用重启后清理残留。

### 5.3 任务模型:三种意图

agent 启动命令时必须声明意图,工具 schema 用一个 `mode` 字段表达:

| 模式 | 用途 | 等待 | 寿命 | 结束时 |
|------|------|------|------|--------|
| `foreground` | 一次性命令(构建、测试、脚本) | 最多 `wait_secs`(默认 60,上限 300) | 超过等待时间**自动转为 `job`**,不杀 | 返回结果 |
| `job` | 有限时长的长任务 | 立即返回 `bg_id` | `max_runtime_secs`(必填,默认 1800,上限 4h),到期杀进程树 | 产生 `task_finished` 通知 |
| `service` | 长期运行的服务(dev server、watch) | 立即返回,或等到 `ready`(端口/日志模式,带上限) | 不设上限;绑定会话或应用,会话关闭或应用退出时清理 | 只在**异常退出**时通知;用户/agent 主动停止不通知 |

监视工具保持现有能力,统一命名与语义:

- `task_wait(ids | scope, until = exit | ready | pattern, timeout_secs)`:宿主侧阻塞等待,检查取消标志(200ms 粒度),返回"满足/超时/取消"之一。
- `task_output(id, cursor, max_bytes)`:读日志文件增量。
- `task_stop(id)`:杀进程树,有确认上限。
- `task_list()`:当前会话的任务快照。

工具描述里明确写出:启动服务用 `service`,不要在前台命令里用 `&`/`start` 启动服务;等待用 `task_wait`,不要用 `sleep` 轮询。宿主侧检测到前台命令里出现常见的后台化写法时,返回提示而不是执行。

### 5.4 后台通知如何交给 agent

取消"任务结束即开新回合"的默认行为,改为:

1. **回合进行中**:通知进入该会话的通知队列,在下一次工具结果返回时以系统消息附加给模型("后台任务 X 已结束,退出码 Y,输出尾部……")。agent 在同一回合内处理,不需要新回合。
2. **会话空闲**:
   - `job` 结束:默认只在界面和会话里显示通知卡片,由用户决定是否"让 agent 处理";可在设置里开启"自动继续",开启后才发起续跑回合。
   - `service` 异常退出:同上,只显示,不自动继续。
3. 自动续跑回合与用户回合走**同一个调度队列**(见 5.5),绝不绕过排队直接抢门禁。
4. 去重键为 `bg_id`(一个任务至多一次续跑),预算只在真正发起回合时扣。

### 5.5 回合调度与状态同步

- **后端会话调度器**:每个 `(项目, session_id)` 一个 FIFO 队列。用户消息、续跑、心跳都作为条目入队;队首获得门禁执行。`chat_send` 不再因门禁被占而报错,而是返回"已排队"及其位置。门禁变为调度器的内部实现。
- **会话状态事件**:调度器在任何变化时广播 `session_state { session_id, running: { turn_id, task_key, origin: user|bg|heartbeat, started_at }, queue: [...] }`。前端的 `running`/队列显示完全由它驱动;前端启动时调用一次 `session_state_list` 获取快照。
- **停止**:针对 `turn_id` 而不是前端记住的 `taskKey`,这样停止对任何来源的回合都有效。停止请求立即广播 `stopping` 状态,让界面马上有反馈。

### 5.6 消息流

- **事件带序号**:每个回合的 `agent_event` 带 `(turn_id, seq)`。前端按序号应用,检测到缺口时从持久化流(`durable_stream`)补齐,而不是猜测。
- **单一 reducer**:回合的文字和工具放在同一个状态对象里,由一个纯函数 `reduce(turnState, event)` 推进(参考 WorkZ/BotZ 的做法):
  - `text_delta` 追加到当前文字段;
  - `tool_start` 结束当前文字段并开启工具段;
  - 下一个 `text_delta` 开启新的文字段。
  这样不再需要"文字偏移量"。显示合并(80ms 节流)只作用于渲染,不影响 reducer 状态。
- **持久化同构**:后端历史也以"段列表"(文字段、工具段)返回,取代 `text_offset`,消除 `\n\n` 和 UTF-16/Unicode 标量的差异。
- **整体替换只在回合边界**:`syncMessagesFromDb` 只在没有运行中回合时调用;回合运行中重新进入会话时,先加载持久化段,再从持久化流的最后序号继续订阅。

---

## 6. 分阶段实施

| 阶段 | 内容 | 风险 | 验收 |
|------|------|------|------|
| 0 埋点 | §4.2、§4.3 的事件日志;取消耗时日志;`join_capture` 超时 warn 已有 | 低 | 下次复现能从日志定位 P3/P4 |
| 1 止血 | 前台读线程等待上限补齐到 `spawn_std_detached`;CodeZ 在非 `text_delta` 事件前先 flush 并在事件到达时同步捕获偏移;服务被停止不触发续跑;`ready` 与 `terminal` 共用一个续跑名额;`restore_pending` 去重键修正 | 低 | 单元测试 + 手工场景 |
| 2 回合调度器 | 后端会话队列、`session_state` 事件、按 `turn_id` 停止;前端移除自有的 running 推断,改为订阅 | 中 | 用户消息在任何后端回合运行时都排队而非报错;多窗口一致 |
| 3 进程层 | 输出落盘;句柄继承收口与 lint;应用退出清理与重启残留清理 | 中高(跨平台) | 前台命令在有 Job 外持有者时仍按时返回;无孤儿进程 |
| 4 任务模型 | `foreground/job/service` 模式、统一工具命名与描述、通知交付改为回合内附加 | 中 | agent 能启动服务后继续对话,并通过 `task_wait`/`task_output` 监视 |
| 5 消息流 | 事件序号、单一 reducer、段式持久化 | 中 | 文字与工具顺序在实时与历史中一致 |

阶段 1 与阶段 0 可以同时做;阶段 2 是后续所有界面状态问题的前提,建议优先于 3–5。

---

## 7. 测试计划

- **进程层**(Windows 为主):启动一个继承了前台命令管道写端的外部进程,断言前台命令在上限内返回;命令内用 `start` 脱离 Job 启动进程,断言不卡;应用退出后无残留进程。
- **调度器**:续跑回合运行时用户发送,断言排队且按序执行;停止对续跑回合生效;前端刷新后状态从快照恢复。
- **续跑策略**:服务被 `task_stop` 停止不产生续跑;`ready` + `terminal` 只扣一次预算;重启恢复不重复续跑。
- **消息流**:reducer 对"文字—工具—文字"、并发只读工具、重试重新流式、缺口补齐的单元测试;实时渲染与历史加载对同一回合产生相同的段列表。
- **取消**:多个在途工具时停止的端到端耗时上限(目标 ≤ 10s)。

---

## 8. 未决问题

1. `service` 的生命周期绑定到会话还是项目窗口?会话切换时服务是否保留?
2. 自动续跑默认是否关闭?关闭会改变现有用户的体验。
3. 输出落盘后,大输出的磁盘占用与轮转策略(当前日志上限 8 MiB)。
4. 远程开发(`remote_dev.rs` 目前拒绝后台任务)是否纳入同一模型。
