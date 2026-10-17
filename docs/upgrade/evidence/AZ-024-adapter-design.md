# AZ-024 最小观察式 LLM adapter 接线设计

2026-10-09。只读方案，未修改产品或台账，未运行Cargo、GUI或模型。依据AZ-024-budget-integration-baseline.md、当前budget/reservation契约及普通local chat。不是硬费用预算完成证据。

## 第一接线范围与文件

建议只启用普通本地单任务的显式观察式候选，不设置收费上限、不改变provider/model/fallback或默认调用行为。新 `src-tauri/src/runtime/llm_accounting.rs` 包装 `Box<dyn LlmClient>`；`runtime/mod.rs` 最小导出；`commands/chat_turn.rs` 在ApprovalHooks binding==early_binding校验之后、Harness.into_agent_loop之前包装client。初始化观察式policy为明确observe字段/现无限policy的可信宿主配置，不能伪造用户金额。初始化必须由显式候选控制路径负责，不在readonly查询中建表。

当前client构建 `chat_turn.rs:2311`，绑定核对2469，into_agent_loop2502。包装器捕获canonical project、完整early_binding、provider标识、boot/adapter实例ID；所有请求使用捕获身份，不在await后解析current renderer session/环境。Run/task预算总key仍canonicalProject+rootTask，run/attempt仅归因。

该client包括main、fallback、纠偏、proactive/overflow摘要及结构化摘要合并的逻辑调用。Harness本次vision_delegate=None，但直接图片仍可能出现在主请求；观察式可记录unknown token，不宣称可靠多模态预算。delegate/call_fish、Koi/headless、自动标题/记忆/技能复盘不经过本client，仍明确未覆盖。不能发布“全Task费用”文案。

## 最小接口

建议 `ObservedClient::new(inner, CapturedAccountingScope, Arc<AccountingWriter>) -> Result<Self>`；scope只由宿主构造，字段包括完整Binding、canonical project、boot、provider来源，不接模型自行传入scope。client实现原 `complete(req)` / `stream(req,tx)`，不改变主内容/工具事件。

一次实际逻辑调用开始前生成UUID operation ID，并创建唯一usage ID（例如同operation下单次final ID）；内存重放同完成记录复用ID，不每次SQLite重试新ID。fallback/纠偏是新的调用，新operation，不能把整个turn当一个。ID不从prompt内容hash推导：相同prompt连续两次是真实两次消耗。kind先 `llm.logical_request`，不要按prompt猜main/summary；明确purpose需后续kernel typed字段。provider/model快照不含api_key/baseurlquery或prompt/tool参数。

观察式record仅记录Measurement的可信已知token、单调Instant duration、unknown cost与measurement source；price_source=None，cost=None。current Usage仅允许source最多256byte，不应塞大段请求/隐私。可使用受限版本化source标识保存provider/model归因，或先另增typed可验证metadata字段；不能将原始request存进账本。

## usage 真实性是首个接线前置缺口

当前 `LlmResponse` 两个u32字段没有usage存在性；OpenAI缺字段默认0，stream Done也能含默认0。**装饰器不能从收到Done/Ok或正数推断整个usage已知。** 无内核改动的最小诚实包只能全部tokens=None，记录逻辑请求数量、active_ms与unknownprice/token数量；不能把当前0字段当真实0结算。这可作为观察式子包，但token预算仍未接线。

需要真实token计量时，最小上游契约在 `piscis-kernel/src/llm/mod.rs` 增加显式 `reported_usage: Option<ReportedTokenUsage>`（checked非负整数，两个计数字段都实际存在且有效才Some），response和stream结束携带同样有效性；更新所有client与fixture构造，保持内容/现UI计数兼容。OpenAI、Anthropic等各provider parsing必须分别验证缺字段/负/浮点/溢出，未知为None；不可只修OpenAI宣称全provider。该修改需源仓piscis-engine与固定vendor同步及版本化patch，独立包由主线程协调。

## stream、取消与账务失败

stream包装转发原chunk到原tx，以代理channel检查真实usage有效标记。不要把token/UI每chunk当usage，不重复累计多次Done；两个冲突终端usage应报计量错误。只有完整成功且usage标记可信才写Some；partial/error/取消的tokens保持None，耗时用Instant，不用UTC相减。

LlmClient future可能被外层timeout/cancel直接Drop，因此普通函数finally不是可靠结算点。观察式候选至少需要明确每次operation的宿主owned registry：调用前登记内存active record；正常完成原子claim completion；主turn收尾在cancel/timeout后排空未完成项写unknown一次（所有operation UUID保留），不能只依赖AgentEvent::Done。异常Join/early-return同一RAII监督上下文必须收尾；若不能证明覆盖所有Drop路径，首包只能记录已完成调用并明示“取消/进程崩溃未捕获”，不可假称全Task计量。

**进程崩溃完整性不能由内存registry实现。** 要覆盖Started后崩溃必须用持久reservation/Started或独立持久request journal，并在request前commit；观察式最终measurement不能先写immutableUnknown再按同ID更新成Known。不能绕开phase1幂等契约。持久Started接线应是下一独立包。

SQLite写入由串行host writer完成，禁止持连接Mutex跨网络await。异步worker提交结果需被await，不能fire-and-forget后宣称计量已持久化。费用操作结束后持久失败必须显式错误/unknown并停止新的受控请求；输出已产生不代表账务成功。对于仅显式观察式模式，错误呈现策略需要明确候选级解释，不能擅自改变默认未启用任务。

## reservation 真正启用的条件

1. 捕获scope/current Running、fresh environment lease与Task gate贯穿；commit reserve→首次Granted start之后才调用网络，Replay绝不发第二次。取消不释放Started，Unknown/旧boot保持slot。
2. token输入需可信上界及输出/推理上界；字符启发估算不是证明。directimage、缓存/推理、多模态在无可信界限时拒绝strict模式，不能靠max_tokens假称完整。
3. 金额需可信provider/model/currency/版本/舍入快照及保守上界，unknown价拒绝strictcost；现Reservation Resources.cost只有金额，价格来源需在typed immutable元数据契约中补充，而非空口相信调用者。
4. provider内部兼容HTTP重发不经wrapper；logical operation不等于wire请求。严格wire次数/费用承诺必须下沉计量到每次post，或可信保守预留覆盖内部重发并明确范围。
5. 所有自动旁路与成员须显式enrollment/归账或unsupported。现observer.record_usage可绕reservation限制，未来受控来源必须限定接口并检查permit关联。

## 可运行受控fixture

- Mock complete/stream各调用一次：request透传，输出/chunk无重复，捕获Binding不受current环境/task变化污染，fallback新operation而相同请求重复调用各记一次。
- 真实SQLite wrapper：两Attempt/newRun仍同Task累计；provider返回无usage、有效真0、缺一字段、溢出及错误分别计量；未真实validity接线时断言全部unknown，防默认0误用。
- streaming多Done/冲突Done/error/消费者关闭、cancel在调用前/中/完成commit竞态，验证一次finalusage；取消未完成operation不消失。用可控mock barrier不调用付费服务。
- DB trigger/fault：record失败不能回报成功持久；每request账务提交前不准下一受控请求。旧完成响应使用原Run/Attempt，禁止新current归因。
- duration单调、provider/model敏感参数不写账、预算source有界；辅助summary同client可观测，独立delegate/postturn的明确unsupported测试。
- 下一持久permit包：进程模拟drop/reopen保留Started/Unknown、双start仅一个调用、reserve不足zero clientcall、internalwire loopback重发覆盖/边界单列。

实施建议：先包装器mock+真实DB只观察调用数/耗时/未知token，再独立补kernel usage validity，最后接持久Started与严格上界。每包单独候选，不让阶段一未知账务被误当完整token/费用控制。
