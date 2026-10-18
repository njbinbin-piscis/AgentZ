# AgentZ 0.9.15 发布清单

2026-10-18。目标：统一 CodeZ 与 WorkZ 的项目会话续写体验，并在 CodeZ 提供「选择文件夹 + 历史项目」入口（与 WorkZ 共用最近项目清单）。

## 已完成

- [x] 版本号一致：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`（agentz-desktop）、README、RELEASE 均为 0.9.15。
- [x] CHANGELOG 增加 0.9.15 条目。
- [x] WorkZ 点击 `codez` 来源的项目会话直接在当前视图打开并续写，不再强制跳转至 CodeZ。
- [x] 顶栏文件夹按钮及 CodeZ 空状态欢迎页接入全局最近项目清单（`workz.json`），支持「选择文件夹…」与「最近打开的项目」切换及移除。
- [x] `npx tsc --noEmit -p .` 通过。
- [x] `npx vitest run src/i18n/i18n.test.ts` 通过。
- [x] `node scripts/upgrade-version-check.mjs` 通过。

## 发布决策

- 2026-10-18，用户确认设计并要求发布新版本。验收门禁仍未通过，沿用 0.9.12–0.9.14 的做法：不接入 `release-readiness`，通过 tag `v0.9.15` 直接构建安装包。
- 该决定只覆盖 0.9.15，不改变 `RELEASE.md` 对后续版本的门禁要求。

## 未完成（已知）

- [ ] 用户验收：在安装包上验证 WorkZ 直接续写 CodeZ 会话，以及 CodeZ 顶栏/欢迎页切换历史项目。
- [ ] 发布门禁、安装包签名（AZ-064）、macOS 与 Linux 真机验证。

## 发布方式

- 通过 tag `v0.9.15` 触发 `release.yml`。
