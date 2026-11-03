# AgentZ 1.0.50 发布检查清单

2026-11-04。基于 1.0.49：修复非 Windows 发布编译，并把内核引用改为 v0.8.73。

## 发布门禁

- [x] 版本号已同步。
- [x] 内核 git 引用为 `v0.8.72` 的三处已改为 `v0.8.73`，上游标签已推送。
- [x] `CHANGELOG.md` 与 `docs/upgrade/release-notes-1.0.50.md` 已更新。
- [x] `cargo check -p agentz-desktop` 在 Windows 上通过。Linux 上的死代码错误已用 `cfg(any(windows, test))` 去掉。

## CI

- [ ] `v1.0.50` 的 Release 工作流中，Linux x86_64、Linux aarch64、macOS aarch64、macOS Universal 都成功，发布页上不只是 Windows 安装包。
