# AgentZ 1.0.26 发布检查单

2026-10-26。相对 1.0.25:放开 BotZ 成员权限,搜索不再因受保护条目整体失败。

## 自动检查

- [x] 版本号同步到 `package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`(仅 `agentz-desktop`)、`README.md`、`RELEASE.md`。
- [x] `CHANGELOG.md` 与 `docs/upgrade/release-notes-1.0.26.md` 已更新。
- [x] `cargo clippy --all-targets -- -D warnings` 通过。
- [x] `cargo test --lib approval` 通过(含搜索范围的 junction 替换测试)。
- [x] `node scripts/upgrade-version-check.mjs` 通过。

## 需要人工验证

- [ ] 成员能用 `pool_chat` 发消息、在自己的 worktree 写文件并 `complete_todo`。
- [ ] 在含 `.agentz` 和 `.env` 的项目根执行 `**/*.md` 搜索能返回结果,且结果不含 `.env` 内容。
