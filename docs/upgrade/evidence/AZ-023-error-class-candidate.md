# AZ-023 错误分类限定候选

2026-10-08。独占修改源仓 `C:/Projects/piscis-engine/piscis-kernel/src/llm/error_class.rs` 和 AgentZ 对应 vendor 文件；未修改 loop、其他源仓 dirty 文件、Cargo 或台账，未运行 Cargo、真实模型、GUI。主线程负责统一编译与运行测试。

新增公共 `ErrorClass::QuotaExceeded`，serde 与 `code()` 为 `quota_exceeded`；`is_retryable()` 仅允许 `UpstreamTransient`，fallback 仍仅允许 `ModelUnavailable/RateLimited`。明确鉴权失败、HTTP402/明确资金耗尽标记优先于 rate_limit、model_not_found、timeout/connection 和上下文错误文本。普通周期性请求 quota 不自动认定资金耗尽。HTTP500–599 是临时错误，其他已识别 HTTP 状态不因正文数字502或connection片段误判为临时；没有状态的网络错误兼容大小写。

仓内引用审查：源仓与 AgentZ 宿主搜索未发现除本模块 `code()` 外的穷举 ErrorClass match；loop 使用分类比较和字符串 code。已有 enum variants/codes 保留，新增 variant 是扩展的 wire 契约；未检索外部消费者，不能声称第三方 Rust exhaustive match 源兼容。parallel_plan 已获明确 QuotaExceeded/is_retryable 契约并独占 loop 策略接线，本证据不替其声明行为通过。

新增四组 Rust fixtures：混合401/403/429/500鉴权关键词；429余额耗尽/402/503硬额度并验证非retry/fallback及周期性quota区分；500/502/503/504/529/599与400/404正文transient误导；新增serde/code及旧code round-trip。既有11分类fixtures保留。**本轮仅编写，未实跑这些测试。** 主线程建议针对 `llm::error_class::tests` 实跑，再结合实际 loop fakeclient矩阵确认调用次数，不用分类fixtures替代真实策略验证。

目标 source/vendor 同字节 SHA256：`2517c81c22e7bdf3a65590e83cb3bf9cb5d9f5e5a5021c15f000599cb8c43621`。版本化补丁 `docs/upgrade/patches/AZ-023-error-class-v0.8.68.patch` SHA256：`4d4ecc3ca917391901d357f7c77922f572fcb25272dd2b6dd1a064a2f554cf22`，仅目标文件。

已实际运行 rustfmt（单文件）、source→vendor目标同步、SHA比较和补丁夹具；从固定commit `527e55496caf4437e4265e02869f0a8c764549f6` 的目标文件（LF bytes SHA `85b6e167a4ae63fa317dc0b68f9d7b97e97c39b1fd59547e594e5b04fe16efd5`）在独立临时Git仓执行 apply --check / apply / reverse --check / reverse，补丁后与新source逐字节相等、反向后与baseline逐字节相等。第一次夹具继承全局autocrlf，字节比较因LF→CRLF失败；第二次仅在隔离仓设置core.autocrlf=false后全过程exit0，记录于 `.cache/az023-class-byj8gjyb`。该格式修正没有修改产品 Git 配置。

原重建脚本 patch 链未更新，不能宣称脚本已自动包含本候选；新patch可由主线程整合并另验证完整链。flattened string分类仍是heuristic，明确永久marker可能来自正文；实际HTTP结构化错误、辅助模型/wire请求总预算、真实provider验收均未在本包完成。

收尾修正：删除顶层文档对已移除 `is_fallback_eligible_error` helper 的引用，改指现有 `agent::loop_` 统一策略；仅文档变化。重新生成目标patch并在独立 `.cache/az023-class-final-kbelj7ds` 重做apply/reverse及前后byte比对通过，测试仍由主线程运行。
