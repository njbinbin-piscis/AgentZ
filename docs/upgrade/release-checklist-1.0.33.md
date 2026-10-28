# AgentZ 1.0.33 发布检查清单

2026-10-28。基于 1.0.32：零输入预算修复、按模型的上下文窗口、设置面板重组。

## 发布前检查

- [x] 版本号统一：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`（仅 `agentz-desktop`）、`README.md`、`RELEASE.md`。
- [x] `CHANGELOG.md` 与 `docs/upgrade/release-notes-1.0.33.md` 已更新。
- [x] `tsc --noEmit`、ESLint、`cargo check --tests` 通过；内核 budget/llm 测试 73 项通过。
- [ ] `node scripts/upgrade-version-check.mjs` 通过。

## 安装后人工验收

- [ ] 模型"最大输出 Token"设为 256000 时，日志中 `total_budget` 不为 0，不再每步执行 `proactive compaction tier=full`。
- [ ] 模型条目可单独设置上下文窗口，保存后重新打开仍保留。
- [ ] 设置面板默认打开"通用"页；"Agent"页包含策略、迭代次数、权限、委派和 MCP。
