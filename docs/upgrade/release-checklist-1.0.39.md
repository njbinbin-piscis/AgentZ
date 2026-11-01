# AgentZ 1.0.39 发布检查清单

2026-11-01。基于 1.0.38：`file_search` 拒绝文件路径。

## 发布前检查

- [x] 版本号已同步：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`（仅 `agentz-desktop`）、`README.md`、`RELEASE.md`。
- [x] `CHANGELOG.md` 与 `docs/upgrade/release-notes-1.0.39.md` 已更新。
- [x] `file_search` 单测 `file_path_is_an_error_not_no_matches` 通过。
- [x] `node scripts/upgrade-version-check.mjs` 通过。

## 安装版验收

- [ ] 对一个文件调用 `file_search`，结果为 `Path is not a directory`，而不是 `No matches`。
