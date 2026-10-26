# AgentZ 1.0.25 发布检查单

2026-10-26。相对 1.0.24:修复 BotZ 成员缺少 pool 工具与 `Cargo.lock` 错误,并补齐 BotZ 协同缺口。

## 自动检查

- [x] 版本号同步到 `package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`(仅 `agentz-desktop`)、`README.md`、`RELEASE.md`。
- [x] `CHANGELOG.md` 与 `docs/upgrade/release-notes-1.0.25.md` 已更新。
- [x] `npx tsc --noEmit` 通过。
- [x] `npx vitest run src/workspaces/botz` 通过。
- [x] `cargo test --lib` 中新增的预算、父任务、PR 链接解析测试通过。

## 需要人工验证

- [ ] 两个成员的办公室:协调者派发两个互不依赖的 todo,确认并行启动。
- [ ] 成员用 `pool_chat` 发交付物并 `@!` 另一成员,对方被唤醒,`complete_todo` 生效。
- [ ] 把某个 Bot 的每日预算设得很小,确认它在超额后不再启动新回合。
- [ ] 带 `Parent:` 的子任务全部结束后,父任务负责人被唤醒。
- [ ] 评审面板 "Open PR" 在已登录 `gh` 的仓库中返回 PR 链接。
