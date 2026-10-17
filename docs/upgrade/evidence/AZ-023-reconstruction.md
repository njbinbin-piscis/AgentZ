# AZ-023 固定基线补丁链重建

2026-10-08。仅修改 `scripts/upgrade-kernel-candidate.mjs`，将 `AZ-023-error-class-v0.8.68.patch`、`AZ-023-loop-v0.8.68.patch` 追加到既有五个补丁之后；没有删除旧补丁、覆盖 vendor、修改产品/源仓代码或台账，没有编译、调用模型或发布。

首次重建失败在旧 AZ029 的 Cargo/secrets/settings apply-check，尚未运行 AZ023。已确认 Git archive 继承源仓 global core.autocrlf，将固定 blob 的 LF 转为归档 CRLF；原脚本又依赖 apply 的 global 策略容忍该差异。修正仅为 archive 命令临时 `-c core.autocrlf=false` 与新建候选独立Git仓 local `core.autocrlf=false`，确保归档/补丁 LF 字节确定。不改源仓、AgentZ或全局Git配置，不改旧patch语义。

实际 `node scripts/upgrade-kernel-candidate.mjs` 最新退出0（98700），从固定 commit `527e55496caf4437e4265e02869f0a8c764549f6` archive，七个补丁逐个 apply --check / apply / reverse --check 成功，输出：

- `.cache/kernel-candidate-319be15a-74a0-40bc-a215-5a3d34fae5b0/manifest.json`
- 同目录 `candidate.tar` SHA256 `88bf801b541f1ffabb2f4ed544988140a947a52bbe1bb2b532ac6639c1fc9f20`
- 分类patch SHA256 `4d4ecc3ca917391901d357f7c77922f572fcb25272dd2b6dd1a064a2f554cf22`
- loop patch SHA256 `5b1307b6eb8409aed56647e40947aa71fc9b65be9636a75e61b994198c710902`

重建目标、产品 vendor、源仓逐字节三方比较成功：

| 文件（piscis-kernel/src/） | 三方共同SHA256 |
| --- | --- |
| llm/error_class.rs | 2517c81c22e7bdf3a65590e83cb3bf9cb5d9f5e5a5021c15f000599cb8c43621 |
| agent/loop_.rs | 846ae9d3198c2c6bac80611a530998ee8872fabb80154e08a2921066ea795781 |

另在独立 `.cache/az023-chain-reverse-dpjrkwb6/source` 复制上述候选（排除Git metadata），逆序对七个补丁执行真实 reverse --check / reverse；仅去除脚本明确添加的三处 package.workspace集成行后，与原 baseline.tar 全部124文件逐字节相等，文件集合也完全相同，无额外或丢失源文件。`node --check` 与限定 `git diff --check` 均退出0。

这只证明当前显式补丁链可重建，且AZ023两个目标匹配；**不是整个当前 vendor 的字节重建**。既有AZ034、平台秘密适配和其他后续改动不在该脚本链内；候选archive不能直接替换整个产品vendor。未运行重建候选的Cargo/GUI/真实模型/跨平台验收，也未声称这些检查通过。主线程已报告实际产品43 loop/15 class等验证，仍需其对当前最终源与本脚本限定复核后整合。
