# AZ-023 HTTP夹具冻结后八补丁重建

2026-10-08。本包仅向 `scripts/upgrade-kernel-candidate.mjs` 既有链末尾追加 `AZ-023-http-fixture-v0.8.68.patch`；不修改生产源、测试、Cargo、vendor 或台账，不运行 Cargo/真实模型/GUI，不发布。

初次八补丁重建基于旧HTTPpatch `e4cdaf77…` 成功，但随后主线程报告10585编译 E0283。收到冻结暂停后，该缓存产物未用于最终hash/逆向验收；parallel_plan修复类型注解并同步source/vendor/patch后才重建。最新patch SHA256：`0bfac48dd8d8c6fd50d3ead8bf849dfa63975abd6aa3b905eb91552e3b93d1a4`。此处记录重建结果，不替主线程追认测试通过。

实际 `node scripts/upgrade-kernel-candidate.mjs` 最新70692退出0：固定 commit `527e55496caf4437e4265e02869f0a8c764549f6` archive，八patch依次apply --check / apply / reverse --check通过；沿用先前仅归档命令和独立缓存Git仓的确定LF设置。输出 `.cache/kernel-candidate-86e6df97-fc03-4fce-97b1-21c160d413ff/manifest.json`；candidate.tar SHA256 `c36bce54271a2ce30f387dbb2cf910340abeb7aac45de3aad98afa93d844f93a`。

重建目标、源仓、产品vendor三方逐字节相等：

| 目标 | SHA256 |
| --- | --- |
| piscis-kernel/src/llm/error_class.rs | 2517c81c22e7bdf3a65590e83cb3bf9cb5d9f5e5a5021c15f000599cb8c43621 |
| piscis-kernel/src/agent/loop_.rs | 846ae9d3198c2c6bac80611a530998ee8872fabb80154e08a2921066ea795781 |
| piscis-kernel/tests/az023_retry_http.rs | 91d5ddf718857f11ca59c9fc149bfd3a18841b46e52f4551ea9d7ca9affd3e92 |

另在独立 `.cache/az023-http-chain-reverse-pdx8lmoz/source` 对八patch逆序真实 reverse --check / reverse，去除脚本明确集成的三个 package.workspace行后，与baseline.tar的124个源文件逐字节及文件集合完全相同；新HTTPtest文件已被reverse删除。验证34298退出0，`node --check` 和限定 `git diff --check` 同退出0。

该候选仅重建显式八patch链，仍不含所有后续vendor平台/沙箱/适配改动，不可整体替换当前产品vendor，也不意味着HTTPfixture编译或运行已通过。主线程的最新冻结源码编译/测试结果需单独记录；本包只有重建、三方bytes和逆向恢复证据。
