# AgentZ 1.0.23 发布检查清单

2026-10-26，从 1.0.22 升级到 1.0.23，范围为重复工具调用的网关兼容与防循环加强。

## 已完成

- [x] 版本号：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`、`README.md`、`RELEASE.md` 已更新为 `1.0.23`。
- [x] `CHANGELOG.md` 与 `docs/upgrade/release-notes-1.0.23.md` 已写入本版内容。
- [x] `cargo clippy --workspace --all-targets -- -D warnings` 通过。
- [x] 内核 `cargo test --lib`：434 个通过；新增折叠、无进展拦截、提醒升级、历史重建四项测试。
- [x] `node scripts/upgrade-version-check.mjs` 见发布记录。

## 人工验收（发布后）

- [ ] 使用千问模型，让 agent 反复执行同一命令，请求不再返回 400。
- [ ] `code_run` 反复得到相同结果时，第 4 次出现提醒，第 8 次被拦截并要求汇报。
- [ ] 新发一条消息后，防循环计数延续而不是清零。
