# AgentZ 1.0.48 发布检查清单

2026-11-03。基于 1.0.47，加入 CodeZ 调试。

## 发布门禁

- [x] 版本号已同步：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`、README、RELEASE。
- [x] `CHANGELOG.md` 与 `docs/upgrade/release-notes-1.0.48.md` 已更新。
- [x] `tsc`、`src/extensions` 测试、`cargo test -p agentz-desktop --lib lifecycle_tests` 通过。

## 人工验收

- [ ] Python：`pip install debugpy` 后，Flask/FastAPI 模板启动，断点停住、变量展开、单步、表达式求值。
- [ ] Go：PATH 中有 `dlv`，主包模板启动并命中断点。
- [ ] Node.js：首次启动能下载 js-debug，断点命中后单步和求值正常。
