# AZ-024 宿主/内核预算接线基线

日期：2026-10-08。只读代码审计；未运行提供商请求、模型、GUI、Cargo 或费用验收。本文不是 AZ-024 完成证据。计划要求全部 attempt/成员累计 token、费用、时长及并发；下述 local 单任务候选只是可先实现的子集。

## 核心结论

可以先在普通本地任务的权威 Binding 上接持久预算，并用 LlmClient 装饰器覆盖主请求、fallback、纠偏以及共享该 client 的上下文摘要。现有 AgentHooks 不是 LLM 请求前置授权接口，现有 session token/Done 统计不是 Task 费用账本。若提供严格费用上限，未知价格、缺失 usage、取消/通信失败后的未知计费不得按零结算；无法可靠预留的入口应明确 unsupported 或失败关闭。

## 实际请求与计量矩阵

以下 kernel 路径均相对 vendor/piscis-engine/piscis-kernel/src；宿主路径相对 src-tauri/src。

| 入口 | 实际客户端/定位 | 现有计量 | 候选接线边界 |
|---|---|---|---|
| 普通主 turn、fallback、格式纠偏 | commands/chat_turn.rs:2311 构建，2482/2502 Harness；agent/loop_.rs:3326 llm_call_unified | 成功 response 累计 token；失败返回 Err 无可靠 usage | 同一个捕获 Binding 的 client 可覆盖每次逻辑请求；不能只按一次 turn 预留 |
| 主动/overflow 摘要及结构化摘要合并 | agent/loop_.rs:1730、1878 complete；共享 self.client | 成功摘要返回 token 并累计；摘要失败可能被忽略 | 同 wrapper 可覆盖，但需要显式请求 purpose/operation ID，不能按提示词猜身份 |
| vision delegate | agent/loop_.rs:2514、2599；3221 调用先于主请求预算 | 独立 vision client；成功 token 未纳入主累计 | 普通 chat 当前配置 delegate=None；其他启用者须独立接线。直接图片仍进入主 request，启用严格预留前需证明图片输入上界 |
| delegate / call_fish 子代理 | commands/chat_turn.rs:1213–1255 注册；1367 helper，1430 独立 flash client，1465 合成 subagent session | 自己的 loop/session 统计，无显式 root Task enrollment | 初期有预算任务禁用或显式 unsupported；只读工具权限不代表不产生模型费用 |
| Koi / headless 成员 | runtime/koi.rs；headless/mod.rs:274、311 独立 client/loop | 独立成员会话统计 | 需宿主 root/member enrollment 与 Run 归属；不得从 pool/session 名推导 root Task |
| 自动任务标题 | commands/chat_turn.rs:2880 附近成功后 spawn maybe_llm_rename_session_title；同文件225/241独立请求 | 独立调用，不在主 Done 内 | 禁用或传入原 Task 的显式预算操作；不能主 Run 完成后遗忘费用 |
| 自动记忆/技能复盘 | commands/chat_turn.rs:2911 成功后 spawn；commands/post_turn.rs:25 独立 mem client，skill_review.rs:143 独立 client | 无权威 Binding/预算参数 | 必须禁用或显式归入原 Task；post-turn 签名仅 session/provider/model，无法证明 root/attempt 归属 |
| 编辑、curator、记忆整合等其他直接 LLM 调用 | commands/edit.rs:67/175；curator.rs:345；memory_consolidate.rs:90 | 各自客户端 | 若任务执行间接触发则须归账；独立全局用户操作应单独定义预算 scope，不能冒称已覆盖全部产品调用 |
| 提供商内部 HTTP 兼容重发 | llm/openai.rs:269 post_chat | 一次 LlmClient 调用内部可重发 HTTP；逻辑层无法观察每次 wire attempt | 费用/请求数的严格承诺需下沉到 wire 层，或在预留中显式覆盖所有内部尝试；AZ-023 cap 是逻辑主请求边界 |

AZ-023 的每 iteration 主请求上限12、transient尝试5、fallback候选4和8秒backoff不等于整个 Task 资源预算；摘要、vision、成员及提供商内部 wire 请求仍是独立覆盖问题。

## 计量与价格缺口

- llm/mod.rs:106 LlmRequest 无 Task/Run/Attempt、operation ID 或价格字段；120 LlmResponse 只有 u32 input/output，无 usage 是否存在标记、缓存/图片/推理收费细分。136 LlmClient 只有 stream/complete。
- agent/hooks.rs:80 等只有 before_tool/after_tool/context lifecycle；ContextHookEvent 不包含准确 request/model/usage。不能将工具审批当作请求预留。
- llm/mod.rs:220 estimate_request_input_tokens 是字符启发估算，不是已证明的计费输入上界。max_tokens 也不足以单独保证所有提供商推理/多模态收费上界。
- llm/openai.rs:1035–1036、1135–1136 将缺失 usage 默认0并把 u64 转 u32。当前零值无法区分真实零与未知；严格预算需要 validity 和溢出检查，不能依赖该值释放全部预留。
- agent/loop_.rs:1981 streaming 聚合器在错误时返回 Err；取消、超时、断流可能没有 Done。服务器仍可能已执行/收费，需保留 Unknown reservation 并明确解释性停止，不能以错误即免费。
- agent/loop_.rs:2842/2938 初始化当前 turn/累计计数，3642 更新 session totals；store/db.rs:1948 是 session加法 UPDATE，无 stable usage ID 去重/预留或预算 CAS。仅用于 UI/context 参考，不能从 lastDone、reset、会话 totals 重建全 Task 严格预算。
- 对宿主源码、kernel/core 和 settings 的 price/pricing/cost_per/per_million/input_cost/output_cost 搜索没有发现当前权威版本化价格字段。动态 provider/base_url/model/fallback 别名必须由可信宿主固定币种/价格版本/适用模型快照；未知价格须显示未知。未查询实时商业报价，也不推荐默认费率。

## 权威身份与跨 attempt

runtime/approval.rs:579 resolve_binding 做 fresh environment/direct workspace 检查，606 task_id 来自宿主 session；crates/agentz-task-state/src/approval.rs:10 Binding 保有 project_id/task_id/environment_id/run_id/attempt_id。commands/chat_turn.rs:2100–2130 生成或接 queued Run/Attempt，设置真实 attempt 并 begin_bound/start_bound；2469 校验 ApprovalHooks binding 与早期 Binding 一致。

候选预算总 key 应为 canonical project + 已登记 root Task。Run/Attempt/环境保留为单次 reservation 的权威元数据，不得放入总 key 造成重试、续跑或切环境重置。当前普通 Task 与 session 的关联必须通过上述宿主 Binding/Run 读取，不能相信 renderer任意session、清计划、lastDone 或合成 subagent 名。新目标是否产生新 root Task 需另有明确产品契约。

复用项目 scope lookup、fresh environment lease、Run/currentAttempt 校验；价格/限制/member enrollment 均由宿主配置，renderer不能随意改总额或把成员归另一 Task。并发 slot 应在同一持久事务预留，跨崩溃未结算记录不能自动TTL释放。Task时长如何计量（墙钟含暂停还是执行累加）尚需明确，不能直接用每turn timeout宣称覆盖。

## 建议最小实施顺序与验证

1. 先实现独立预算 namespace：绑定权威 Run/currentAttempt 的 Immediate transaction reserve/settle、stable operation/usage去重，跨attempt总额不重置，未知预留保持Unknown。与 parallel_plan 的设计一致，不改变 Run schema version。
2. 普通本地、单代理、纯文本候选先接捕获 Binding 的预算 client；主/摘要每个逻辑请求使用不同稳定operation ID。unknown usage/price/上界显式拒绝严格费用模式，允许纯token模式时也必须说明估算与真实usage区别。
3. 启用预算候选时对子代理、Koi、remote、多模态以及自动标题/记忆/技能复盘明确禁用或 unsupported，直到显式归账；不应悄悄遗漏自动 side effect。
4. 受控 fixture 验证两attempt累计、并发最后额度只一请求获准、fallback/摘要耗尽停止、重复settlement不重复扣费、缺usage/断流/cancel/崩溃不释放未知、价格变更仍使用捕获快照、member错归属失败关闭。wire重发与真实provider费用对账另列验收。

此审计没有发现现有代码已经具备全 Task 严格费用预算。它提供可消费接口和缺口；成员/全部费用/完整桌面场景仍未验收。
