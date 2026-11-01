# 参与 AgentZ 贡献

[English](CONTRIBUTING.md) | **简体中文**

> 本文是 [CONTRIBUTING.md](CONTRIBUTING.md) 的中文翻译。两者不一致时以英文版为准。

感谢你关注 AgentZ。本文介绍如何报告问题、搭建构建环境，以及如何让改动被合并。

## 报告问题

- 先搜索[已有 issue](https://github.com/njbinbin-piscis/AgentZ/issues)。
- 写明 AgentZ 版本（安装包文件名，或你安装的 GitHub release）、操作系统、模型服务商，以及复现步骤。
- 日志在配置目录下的 `logs/`（见[配置与数据](README.zh-CN.md#配置与数据)）。附上任何内容之前，先删除 API key、token 和私有文件内容。
- **不要**在公开 issue 里报告安全漏洞，请看 [SECURITY.zh-CN.md](SECURITY.zh-CN.md)。

## 开发环境

前置依赖、Linux 系统包和主要命令见 README 的[开发](README.zh-CN.md#开发)一节。简单来说：

```bash
npm install
npm run tauri dev
```

内核从 `vendor/piscis-engine` 编译（通过根目录 `Cargo.toml` 的 `[patch]` 接入）。内核改动必须同时提交到上游 [piscis-engine](https://github.com/njbinbin-piscis/piscis-engine) 仓库，并更新 `Cargo.toml` 中钉住的 tag，保证 vendor 副本与上游一致。

## 提交 pull request 之前

运行与 CI（`.github/workflows/ci.yml`）相同的检查：

```bash
node scripts/upgrade-version-check.mjs
npm run lint
npm run typecheck
npm test
npm run test:e2e
npm run build
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

如果改了 `bundled/preinstall/` 下的预装技能或模板，还要运行 `npm run verify:preinstall` 和 `npm run lint:preinstall -- --strict`。

## Pull request 规范

- 一个 pull request 只做一件事，无关的修复分开提。
- 行为变更要补充或更新测试。
- 风格与周边代码保持一致。Rust 工作区把 warning 当作错误。
- 面向用户的文案走 i18n：同时在 `src/i18n/en.ts` 和 `src/i18n/zh.ts` 中添加键。
- 用户可见的变更，在 [CHANGELOG.md](CHANGELOG.md) 即将发布的版本下添加条目（[Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/) 格式）。
- 提交信息用祈使句，必要时加简短前缀，例如 `fix: ...`、`feat: ...`、`docs: ...`。
- 不要提交 API key、token 或其他密钥。

## 文档

英文文档为准。中文译文与英文文件放在一起，命名为 `<name>.zh-CN.md`，标题下方有语言切换链接。修改有译文的英文文档时，请在同一个 pull request 中更新译文，或在 pull request 中注明译文待更新。两者不一致时以英文版为准。

`docs/upgrade/` 下的发布记录（发布说明、检查清单、证据）属于历史档案，保留原语言。

## 发布

发布由维护者推送 `v*` tag 触发；版本同步清单见 [RELEASE.md](RELEASE.md) 和 README 的开发一节。

## 许可证

提交贡献即表示你同意你的贡献以本仓库的 [Apache License 2.0](LICENSE) 授权。
