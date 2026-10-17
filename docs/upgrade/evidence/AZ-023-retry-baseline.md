# AZ-023 重试起点只读审计

2026-10-08。本轮只读源代码、工作区状态与指纹，未调用模型、运行编译/测试、修改产品源码或台账。以下次数是代码推导，不是实跑结果。

## 规则与源归属

使用 `rg --files --hidden -g AGENTS.md -g '!**/.git/**' C:/Projects/piscis-engine` 搜索，并检查 `C:/AGENTS.md`、`C:/Projects/AGENTS.md`、源仓根目录；未发现适用 AGENTS.md。源仓已有用户/其他工作修改：`piscis-kernel/Cargo.toml`、`agent/file_journal.rs`、`security/secrets.rs`、`store/settings.rs`，后续不得整体覆盖该仓。

源仓与 AgentZ 固定 vendor 的两处目标源码当前逐字节相同：

| 文件（piscis-kernel/src/ 下） | 两仓共同 SHA256 |
| --- | --- |
| agent/loop_.rs | ef9b843939135371b05fad3a3d1e245f9b06d7c8598d0326dc0c4c4e5453d6eb |
| llm/error_class.rs | 633a543bb63895c12b16e3f972554cb97f2c34fb3a92f05c913c2cd8aec3f3c4 |

AgentZ root `Cargo.toml:29` 的 path patches 实际消费 `vendor/piscis-engine`。`vendor/piscis-engine/AGENTZ-BASELINE.md` 记录基线 commit `527e55496caf4437e4265e02869f0a8c764549f6` / v0.8.68。`scripts/upgrade-kernel-candidate.mjs:11` 从不可变 commit archive 重建，仅应用显式 AZ029/079/081/082/030 patch 链；不读取 dirty source，也不重建所有后续 vendor 改动。

因此后续最小实现应明确维护源仓目标改动和对应 vendor 目标文件/版本化补丁，验证目标文件相等及候选 patch apply/reverse，保留其他 vendor 差异。不可直接把源仓整个目录覆盖 vendor，也不可声称现重建脚本包含 AZ023。若更新脚本 patch 清单，需独立验证重建基线及产品实际依赖；本轮没有执行重建或同步。

## 当前请求次数矩阵

令 `F = 1 + fallback_models.len()`（当前不去重），`T = same_model_transient_retries.max(1)`（默认 3，实际是总尝试次数，不是“初次之外的重试数”）。下表针对一次普通 agent iteration 的主模型逻辑调用，不把工具调用次数、视觉委托、摘要调用混为一谈。

| 输入/分支 | 当前分类及行为 | 逻辑主调用上限/退出 |
| --- | --- | --- |
| 普通 HTTP 429 / rate_limit | RateLimited；立即换下一 fallback，不做同模型退避 | 每候选 1 次，总至多 F 次 |
| model_not_found / 特定 404 | ModelUnavailable；换 fallback | 至多 F 次 |
| 502/503/529、lowercase timeout/connection/overloaded、部分 EOF/decode | 同模型重试，指数 1/2/... 秒；耗尽直接终止，不再 fallback | 单候选至多 T 次，默认 3 次/等待 1+2 秒；先发生 fallback 时前序模型额外各 1 次 |
| 普通 500/504/其他 5xx（正文不含上述片段） | Unknown；不重试 | 当前候选 1 次即终止 |
| 普通 401/403 / invalid API key | AuthFailed；若正文不命中 transient 片段则立即终止 | 当前候选 1 次；但混合正文可能错误重试，见下 |
| 429 insufficient_quota / 余额耗尽 | 没有 quota 分类，429 先归 RateLimited | 当前会遍历 fallback，至多 F 次，并非永久错误立即停止 |
| tool_args_invalid | 单次 outer corrective，重新从 primary 开始模型链 | correction 最多 1 次，但整个模型/退避/overflow 预算重新建立 |
| 上下文溢出且 compact.changed | 每个 corrective sweep 最多 1 次 overflow 恢复，然后同候选重建请求 | 增加摘要请求和重启主候选的尝试窗口；无统一总请求计数 |
| 已取消 | 每次主调用前检查；在 LLM await 与退避中 200ms 轮询竞争 | 观察取消后不再主调用，结束 iteration；不是严格即时取消保证 |

依据：`agent/loop_.rs:3161` corrective flag、`:3179` fallback 链、`:3278` T、`:3280` 调用前取消、`:3291` LLM select、`:3337` 分类日志、`:3351` overflow、`:3456` fallback、`:3468` 另行 raw transient 判断、`:3485` 退避及取消、`:3562` corrective。`llm/error_class.rs:121` rate_limit/429 在 auth 之前，`:125` auth，`:144` transient。`agent/harness/config.rs:477` 只 `.max(1)`，`:582` 默认 3。

在有限 F/T 下控制流有结构边界，但 F/T 输入没有统一硬上限，也没有整个 iteration 的主请求预算。粗略保守逻辑上界可写为两次 sweep 各 `T*F + T`（额外 T 用于一次 overflow 重启）；这不是当前实现显式计数器，也不覆盖辅助模型调用和客户端内部重发。

OpenAI 客户端 `llm/openai.rs:269–303` 的 `post_chat` 可在 400 reasoning replay 错误时关闭 thinking 并重新 POST 一次。一个逻辑调用因此可能有两次 wire 请求；正确性纠偏、fallback、摘要/视觉调用不能仅靠主循环次数宣称整个 Run 网络请求/费用有界。费用与跨工具资源总预算仍归 AZ024。

## 最小可行动缺口

1. **永久错误没有压过 raw transient 判断。** 分类用于日志/fallback，但重试决策另用 case-sensitive `msg.contains`。例如 `OpenAI API error 401 Unauthorized: connection rejected` 先分 AuthFailed，却因 `connection` 重试至 T；429 insufficient_quota 先走 fallback。应让鉴权/余额/取消优先终止，并让一个统一分类决定重试/换模型/纠偏，补混合错误矩阵。不能只增加日志枚举而保留旁路判断。
2. **没有硬总尝试/退避边界。** T 只下限 1，fallback 允许重复候选；`1 << attempt` 对大 T 有 shift 溢出及极大等待风险。最小策略需 T/fallback/主请求数的固定上限、saturating/capped 退避，并让 corrective/overflow 共用主请求预算，保留有限一次纠偏而不重置全预算。主逻辑预算与 wire/辅助调用预算分别记录，不能冒称全 Run 费用预算完成。
3. **状态分类与策略不一致。** classifier lowercase 而 loop raw case-sensitive；普通 500/504 没有按 5xx transient 处理。应以真实状态/稳定分类为优先，flattened 字符串兼容为保守兜底，验证 429、全部目标 5xx、auth/quota、未知永久错误、耗尽与取消等待；context compact await 的取消和辅助请求耗时另列限制，不声称已被主调用 select 覆盖。

建议下一 1–1.5 天原子候选首先限定主 agent 请求链，fake client/受控本地 HTTP fixture 记录真实调用次数、候选顺序、纠偏、取消前后、混合永久错误；不调用真实 provider、不重执行工具、不修改 AZ018 的当前稳定源码范围。固定源仓及 vendor 的目标接口修改后分别验证；台账仍由主线程依 ADR006 消费 AZ017 当前限定复核进入 preparing，正式 Gate 不变。
