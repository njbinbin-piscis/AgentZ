# AZ-064 签名、安装器准备可行性复核

2026-10-07，Codex 子代理只读检查，不运行打包、签名、安装、CI、下载或发布；仅创建本证据，不改台账。

## 依赖与真实验收边界

AZ-064 pending，任务依赖 AZ-005 已 done，属于 M8；M8 的前置 Gate 尚未通过。因此可由主线程按 ADR-005 进入 preparing 候选实现，不可进入正式 review/done。验收仍要求声明支持平台的实际签名身份/完整性、干净机器安装/启动/卸载记录；静态检查和模拟测试均不能替代。

## 当前源码事实

- `src-tauri/tauri.conf.json` 固定产品/版本/identifier，bundle.targets=all，包含 extension-host JS、Node 目录、预装扩展等资源。Windows overlay 明确 targets=[nsis]。配置未指定 Windows 签名身份/签名命令或 macOS signingIdentity/entitlements；不能据此推断证书可用。
- 本地安装的 Tauri CLI `config.schema.json` 含 certificateThumbprint、timestampUrl、signCommand、signingIdentity、hardenedRuntime、entitlements 等配置字段，可用本地 schema 校验候选配置，无需猜测外部版本。
- `.github/workflows/release.yml` 有 Linux x64/arm64、Windows x64/arm64、macOS arm64/universal 构建定义，tag v* 和 workflow_dispatch 触发，contents:write，tauri-action releaseDraft:false/prerelease:false。这是直接公开发行配置，未运行；不能将 AZ-064 候选准备连接到该触发器或推 tag。Linux/macOS artifact 缺失仅 warn，Windows为 error。
- `extension-host/scripts/fetch-node.mjs` 固定 Node 22.11.0，仅按运行该脚本机器的 process.platform/process.arch 下载本地主机 runtime，存在文件直接复用；未见下载摘要校验。macOS universal job 仍只调用 host fetch-node，因此源码不能保证同时含 darwin-x64/darwin-arm64 runtime。此为候选静态缺口，不宣称已跑 universal 包或实际 Intel 运行失败。
- 当前本机资源目录可见 win32-x64，以及 Linux x64/arm64 远程 archive。目录存在不证明内容完整性、目标架构或签名。
- AZ-005 支持矩阵明确跨平台/桌面/签名更新未获本轮运行证据，不能从既有 CI 定义提升为已验证支持。

## 无证书、无新增平台资源可实施的候选包

1. **离线发行预检与版本化 manifest。** 新增专用脚本和 schema/测试，输入明确平台架构、渠道、产物目录、提交身份；校验 package/Cargo/Tauri版本一致、identifier、目标 bundle 与必需资源、预装 manifest 和架构 runtime存在，计算 SHA-256，并记录每项 verified/missing/unverified。严格禁止把文件存在或 unsigned 包当签名已验收。夹具覆盖缺资源、错误架构、版本不一致、坏文件以及完整 unsigned candidate。只读检查默认不下载、不安装、不写 secrets、不运行发布。
2. **候选构建与发行触发隔离。** 准备 artifact-only 的候选 workflow/配置，contents:read、无 tag 发布/无 tauri-action release 元数据，必需 artifact 缺失 error；主线程审阅具体 diff 后决定如何替换现有公开触发。静态测试应拒绝候选工作流中的 releaseDraft:false、prerelease:false、推送/上传公开 Release 等路径。工作流源码可先本地验证，但不 dispatch CI，不假称 GitHub runner 已通过。
3. **签名证据契约与本地校验入口。** 保存平台签名检查的字段/错误分类/预期身份占位契约和 mock 工具输出解析测试：产物路径与摘要、平台/架构、身份、有效期/时间戳、验证工具原始结果、签名及安装验收是否实际执行。缺证书/身份/工具/平台时明确 unsupported或missing，不生成伪签名记录。产品签名身份、证书采购/密钥服务、Apple账户及平台机器由用户或后续资源提供；此包不替其决策，不导出凭据，不扩大签名权限。

优先 1，再 2；3 可做证据模型，但没有实际身份资源前不能证明可信身份。每包仅候选准备，统一由主线程审阅和台账追踪，不能靠拆包把 AZ-064 标 done。Node 下载完整性与 universal runtime矩阵可作为包1对应的资源前置修复，必须使用审阅后的可信摘要来源；本轮未联网取得摘要，不能凭空填写。

## 建议下步与待补资源

AZ-045 收敛后主线程可将 AZ-064 置 preparing 并先实施离线 manifest/预检，不阻塞于证书采购，也不进行不可撤回发行。真实 Windows/macOS/Linux 安装录像、macOS 公证、实际签名身份与干净机器资源仍待补；资源缺失只阻塞最终验收，不需要现在停止所有可逆代码准备。

2026-10-07 主实现转为AZ064 preparing，3个Codex子代理文件独占并行：离线manifest/二进制架构预检、artifact-only workflow静态门禁、签名/安装证据契约。主线程集成及台账，M8 Gate未通过不转review/done。无需现在请求证书或发行批准，先完成可审阅候选；不启动CI/签名/安装/发布，不把原release.yml视为候选安全入口。

原release.yml仍直接公开发行，保持未触发。fetch-node无摘要校验属于已发现供应链缺口，候选不能宣称其下载完整性；universal runtime未具备资源证明时不宣称支持。具体manifest结果与真实平台验收分开。

主线程独立复跑三包：offline manifest10组临时夹具、candidate workflow静态守卫、signing26项合成证据夹具均exit0；没有签名/安装/打包或CI。实际当前仓库Windows x64离线预检启动会话70683，结果待确认；未以临时fixture通过替代真实资源齐备。代理正在将post-build产物manifest接入候选workflow并保持PowerShell退出码失败传播，后续静态守卫再验。

2026-10-07用户新授权收敛为尽快提交发布相对稳定版本，阻塞可跳过并记录下一轮。当前执行目标变为Windows x64候选，保留全部roadmap未验收状态，不以GUI/跨平台/模型未跑变造done。并行运行前端稳定性回归、extension-host已有依赖构建、捆绑Node官方摘要对比；主线程npm run build及发行阻塞清单，尚未签名/发布/对外提交。

会话70683当前仓库预检已完成：exit1，必需资源与版本检查当前通过但NSIS产物目录缺失；候选workflow完整性前置同样exit1，缺approved resource-integrity.json。不以源码输入检查代替安装包验收。主线程开始生成前端构建以形成可审阅候选，真实打包/证书条件单列下一轮。

候选构建进展：扩展宿主npm run build实跑exit0（代理3739），host.js及map资源生成、node --check exit0，见RC-extension-host.md；主线程前端npm run build会话61303仍在vite transform。整合后candidate workflow静态verify主线程exit0，台账validate通过。尚未生成安装包/运行桌面/签名，不假称发行通过。

会话61303 npm run build最终exit0，前端4134 modules/生产dist生成（vite8m27s，有chunk大小/动态导入提示非错误）。并行全量Vitest12文件102项通过、扩展宿主build/语法通过；捆绑Win x64 Node SHA256与官方22.11.0 HTTPS清单一致，但未PGP/Authenticode验签。

已开始本地Windows NSIS候选打包，使用已通过的frontend/exthost资源，临时config只覆盖beforeBuildCommand为空避免重复构建；无签名/发布动作。完整打包日志保存RC-windows-build.log，exit文件结束时保存，不把运行中构建当成功。版本暂0.9.6且dirty身份，候选不是新版本正式发行；后续版本身份/提交发布必须以实包及证据审阅。

NSIS本地候选构建已完成：RC-windows-build.exit=0；release优化编译84m44s，Tauri生成target/release/bundle/nsis/AgentZ_0.9.6_x64-setup.exe，98527524 bytes。Tauri下载NSIS工具及插件时日志含hash验证；仅本地打包，无外部发布。当前安装器版本仍0.9.6，用于冻结源码候选，不冒称新的正式版本已发行。

正在执行产物离线manifest与SHA256/Authenticode状态读取，会话7662；结果落盘RC-windows-manifest.json/RC-windows-artifact.json。未安装/启动GUI/卸载，无完整用户旅程或签名已通过声明。子代理后续计划曾因使用额度失败，未伪造交付，主线程继续本地候选核查。

会话7662完成exit0：实际产物离线输入preflight_exit0；安装器SHA256=99a7f939de2c1ec7aabb31fdb355c524ae65b2781e5d2519308f54e97fde776c；Authenticode状态NotSigned。manifest release_ready=false，installed=false、published=false。这是可交付本地未签名候选，不是正式发行或安装验收通过。

新版本准备：本地已有v0.9.6 tag，本轮候选不能作为新正式版本重复发布。按用户新版本目标，已委派只改AgentZ自身version到0.9.7（保留第三方依赖和已有dirty），不commit/tag/push/Release。其余代理准备发行说明/下一轮原子计划与只读源码指纹，不自动提交117条混合修改。当前0.9.6包作为已验证工程基线保留，新version需重新构建，不能沿用旧包摘要。

六处AgentZ元数据已更新0.9.7且第三方/既有dirty文本保持；本地无v0.9.7 tag，未commit/push/release。0.9.7源码指纹在打包前捕获RC-0.9.7-source-snapshot.json，开启独立NSIS构建日志RC-0.9.7-windows-build.log/exit（旧0.9.6证据不覆盖）。仅版本元数据变化，保留之前已通过frontend/exthost生成资源，宿主及安装器重建。

当前268个allowlist源码dirty条目含122个必要crate/vendor未跟踪，仅HEAD不能复现，指纹不是归档/提交。需发布前建立审阅过源码基线，不能自动把全部用户修改提交。下一轮计划和依赖分离草案已交付，未先豁免验收Gate。

0.9.7 NSIS36609仍编译宿主，未重复启动。冻结产品源码期间并行推进发版后准备：ADR006台账候选代码依赖分离只改scripts/隔离夹具，AZ017限定接口只读复核，0.9.7提交发布清单只读整理。正式plan未放宽、未预先开下游产品实现；不打断在途候选构建，也不把未签名/未安装候选冒称发布完成。
