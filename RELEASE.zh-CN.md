# AgentZ 发布流程

[English](RELEASE.md) | **简体中文**

> 本文是 [RELEASE.md](RELEASE.md) 的中文翻译。两者不一致时以英文版为准。

当前源码版本：**1.0.41**。记录的范围与检查见 [1.0.0 发布决策](docs/upgrade/release-decision-1.0.0.md)、[历史支持基线](docs/upgrade/evidence/AZ-005-support-matrix.md)、[当前升级进度](docs/upgrade/progress.md)、[1.0.41 发布说明](docs/upgrade/release-notes-1.0.41.md) 和 [1.0.41 检查清单](docs/upgrade/release-checklist-1.0.41.md)。

## 构建与分发边界

`.github/workflows/release.yml` 在推送 `v*` 标签或手动触发时运行。其中的 `tauri-action` 任务会创建或更新一个**已发布**的 GitHub Release（`releaseDraft: false`）并上传安装包。每个打包任务都依赖 `release-readiness`，后者在打包前依次运行 `node scripts/upgrade-version-check.mjs`、`node scripts/upgrade-plan.mjs validate` 和 `node scripts/upgrade-release-gate.mjs`。发布闸门检查源码版本、M0–M8 与 MB 闸门记录、已接受的 AZ-069 决策，以及当前 ref 是否为对应的 `refs/tags/v<version>` 标签。这套接线已通过本地工作流检查和脚本测试；尚未记录任何发布工作流演练或签名安装包验收。所有者已为 0.9.12 至 0.9.19 授权了按版本的闸门豁免，详见各版本的发布检查清单。触发该工作流属于分发行为，而不是本地验证步骤。

工作流构建 Linux x86_64 与 aarch64 的 `.deb` 和 AppImage、Windows x86_64 与 aarch64 的 NSIS `.exe` 安装包，以及 macOS aarch64 与 Universal（`--target universal-apple-darwin`）的 `.dmg`。每个打包任务会先运行 `npm --prefix extension-host run fetch-node`，把锁定版本的 Node.js 运行时下载到 `src-tauri/resources/node/`（作为应用资源打包，供扩展宿主和远程目标使用）。`tauri.conf.json` 设置了 `targets: "all"`，但 `src-tauri/tauri.windows.conf.json` 将 Windows 覆盖为 `targets: ["nsis"]`，因此不会构建 `.msi`；Windows 产物为 `bundle/nsis/*.exe`。某个打包目标或 CI 构建成功，并不能证明该平台上的安装、启动、升级、卸载、凭据访问或回滚可用。

当前候选版本没有记录已签名的 Windows 安装包、已公证的 macOS 包、干净机器上的安装/卸载，也没有原生 macOS/Linux 核心使用流程。应用内可信更新器及其恢复路径尚未通过验收。修改支持声明前，请先查看[升级进度](docs/upgrade/progress.md)以及 AZ-064/AZ-065/AZ-077/AZ-078 的证据。

## 准备候选版本

1. 更新 `package.json`、`package-lock.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`、README 中的源码版本（`README.md` 与 `README.zh-CN.md`）、本文件、变更日志，以及对应的发布说明/检查清单。保持 extension-host 的 package 与 lockfile 版本彼此一致。
2. 运行 `node scripts/upgrade-version-check.mjs` 以及相关构建/测试。它检查版本标签、必需文档，以及 README、RELEASE 和 CONTRIBUTING 文件中的本地链接；它不认证安装包。
3. 冻结源码与产物哈希。在 `docs/upgrade/` 中完成发布预检、签名/信任检查、原生安装与恢复流程，以及明确的闸门决策。
4. 只有在获得授权的发布决策之后，才推送版本标签或手动触发发布工作流。记录生成的资产和各平台回执。

## 签名与更新器前提

macOS 公证需要 Apple Developer 签名身份和公证凭据。Windows 签名需要合适的受信任证书或签名服务。工作流目前没有为该候选版本接受的签名身份。机密信息请保存在 CI 的受保护存储中；不要把私钥或密码放进本仓库。

更新器需要固定的受信任公钥、经过认证的更新清单与渠道策略、兼容性检查，以及下载/安装中断后的恢复。在启用自动更新之前，请通过 AZ-065 实现并验证这些能力。不要因为存在 Tauri 打包文件就推断更新器已就绪。

## 诊断与安全范围

诊断导出和机密存储各有其证据，见 AZ-031/AZ-061。渲染进程 CSP（`app.security.csp`，开发环境另有 `devCsp`）配置在 `src-tauri/tauri.conf.json` 中；任何修改都需要在桌面端覆盖 Monaco、本地桥接、资源和已配置端点的测试。当前行为以配置和源码为准，而不是历史发布说明。
