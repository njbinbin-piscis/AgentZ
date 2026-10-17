# AZ-023 主循环有界重试候选

2026-10-08，仅目标源仓与 vendor 的 `piscis-kernel/src/agent/loop_.rs`、独立 patch 及本证据；未修改 Cargo、台账或其他已有 dirty，未运行 Cargo、模型或工具。此变更晚于本轮 0.9.7 安装器构建，不包含在该安装器内。

主逻辑同模型尝试数 clamp 1..5；包含 primary 的去重候选最多4个。每 agent iteration 的主逻辑调用计数最多12，置于 corrective 外部，由 overflow 重建与唯一 tool_args_invalid 纠偏共同消费；overflow 恢复标志也不再因纠偏重置。退避为1/2/4/8秒且安全封顶，极端u32值不移位溢出。取消前检查、调用等待与退避等待原200ms轮询竞争保持。AuthFailed/QuotaExceeded优先终止，包括正文含timeout/context overflow的混合错误；fallback与同模型重试统一消费独立分类模块，不再另用raw transient contains。没有自动重跑工具。

预算只针对 `llm_call_unified` 主逻辑调用，不涵盖视觉、摘要、client内部wire重发、整个Run费用或工具数；自定义compaction仍可能等待辅助请求，不能称总网络/墙钟预算完成。默认3次尝试在硬cap内保持；超大配置与多于4候选现在受到明确固定边界。

新增实际loop夹具：永久混合401/余额429各1调用；50 fallback配置最多4调用；调用内取消后不再请求；极端attempt配置真实5次耗尽；真实退避中取消不启动第二请求；先3fallback、4transient、1corrective，再3fallback与1transient，跨纠偏实际主调用最多12且不产生ToolStart。另候选去重与极端backoff单测。没有Tokio test-util feature，按主线程同意使用真实有界定时（耗尽和纠偏各约15秒，timeout30秒），不改生产delay或Cargo feature，不伪称fakeclock。以上新增夹具本代理尚未运行，统一主线程验证。

固定基线commit `527e55496caf4437e4265e02869f0a8c764549f6` 单目标生成 [AZ-023-loop-v0.8.68.patch](../patches/AZ-023-loop-v0.8.68.patch)。在独立临时目录从git show提取该commit目标，执行apply --check/apply、reverse --check/reverse并校验字节。显式 `core.autocrlf=false`，目标源仓/vendor文件归一LF并逐字节一致；没有整体同步或读取其他dirty文件。

- 基线单文件LF SHA256：`daf4e104c71119a236d626139914b587a0d4ce81d4b07ade5fced0eb7a5b0f7e`
- 当前source/vendor SHA256：`846ae9d3198c2c6bac80611a530998ee8872fabb80154e08a2921066ea795781`
- patch SHA256：`5b1307b6eb8409aed56647e40947aa71fc9b65be9636a75e61b994198c710902`
- 隔离apply/reverse最终实跑exit0，恢复基线摘要相等。初次继承Windows autocrlf使apply文本相同而bytes不同，未追认为通过；明确false重跑后通过。

patch仅目标文件，不自动更新kernel重建脚本patch链；主线程需统一审阅分类模块接口、候选链与实际验证。未宣称桌面、provider恢复、schema rollback或路线图Gate验收通过。
