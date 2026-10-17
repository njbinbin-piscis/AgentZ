# AZ-004 首次真实模型基线交付

2026-10-09，Windows x86_64，本地实际 vendor 内核（Cargo包版本0.8.63、产品固定v0.8.68及已审overlay）与OpenAI兼容客户端，调用官方https://api.deepseek.com / deepseek-flash。官方文档当天说明该别名对应V4.1-Flash；没有独立验证服务端权重或底层部署版本。用户授权提供模型凭据，凭据仅临时进程环境，不写仓库、任务文本、测试产物、配置或证据。official/models实际认证200并确认别名存在。

首次固定12项synthetic任务，每项重复3次，覆盖简单代码编辑、引用合成资料、文档交付。原计划此前只有“至少12项”要求，没有已落盘的12项或真实入口，故明确标记first-fixed-synthetic-v1。每样例实际AgentLoop调用模型及受限合成文件工具；成功必须验证落盘产物与读写记录，而非模型自报。J2资料是单一合成source.txt，J3续写是固定样本文本，不冒充真实桌面检索或持久续聊。

85757无凭据 `cargo test -p piscis-kernel --offline --locked --test upgrade_live_baseline --no-run` exit0。94060显式付费运行 `node scripts/upgrade-model-baseline.mjs --run --repeats 3 --output docs/upgrade/evidence/AZ-004-live-results-v1.json` exit0。实际36/36样例有记录，108 logical calls，45599 input tokens、8175 output tokens，unknown usage0，无provider/collector/超时错误。每样例实际结果、时间、工具读写、用量、成本未知及未验项见原JSON；共同环境记录在run层。

自动artifact oracle原评分33/36（91.67%），不改原数据。J2-04三个产物均明确观察timeout、原因unknown、并否定断言network failure；原规则禁出现关键词，误判否定句。独立代理复核全部36产物，实际执行9份简单JS功能检查和3份JSON检查；小样本语义检查36/36满足限定要求，单独证据AZ-004-live-results-review.md。这是限定synthetic语义结论，不能推广完整项目任务成功率。下一版本评估器需修正该假阴性并扩大任务难度，在现有后续评估工作中跟踪，不覆盖v1或浪费重跑原基线。

Kernel任务耗时p50=3271ms、p95=4468ms；固定顺序重复样本，不是独立随机样本，不是桌面冷启动或输入响应测量。自动运行内人工干预0；后验人工审查与harness实现工作不冒充0人力。36条成本实结算未知，保留null。按当天官方Flash峰时cache-miss输入0.3美元/百万token、输出1.2美元/百万token计算，已记录用量的保守价格估算为0.0234897美元；cache命中/非峰时可更低。该估算不等于账单、全部wire请求数量或全Run硬费用限制。

输入累计每样例16384 bytes、最多4 logical calls、每调用256 output token，禁fallback；auth/quota在下一付费调用前停止。逻辑调用与字节上限不等于provider内部兼容重发的全部wire计数。测试显式关闭thinking，新Flash别名兼容修复有独立源码审查、增量补丁apply/reverse字节核验以及3566定向1/1通过。

原始结果、冻结spec/source hash、全部合成产物、source snapshot与独立审查均留档。汇总脚本区分planned/attempted/unattempted，拒绝重复样本、矛盾成功和源码配置不匹配；verify脚本实际通过。Task AZ004限定“每样例记录结果环境时间成本未验项，缺失不填零”的基线交付完成；MB Gate、真实host/GUI/原生沙箱任务、真实会话恢复、跨平台和账单结算未据此验收。

官方配置/费率来源：https://api-docs.deepseek.com/zh-cn/、https://api-docs.deepseek.com/guides/thinking_mode/、https://api-docs.deepseek.com/quick_start/pricing/（2026-10-09核对）。
