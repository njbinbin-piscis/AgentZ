# AgentZ 1.0.49 发布检查清单

2026-11-03。基于 1.0.48：远程打开文件、浏览器端口转发、远程调试、工具说明。

## 发布门禁

- [x] 版本号已同步：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`、README、RELEASE。
- [x] `CHANGELOG.md` 与 `docs/upgrade/release-notes-1.0.49.md` 已更新。
- [x] `cargo check -p agentz-desktop`、`tsc`，以及路径、端口和调试配置相关测试通过。

## 人工验收

- [ ] 重连 Linux 远程，打开 `/home/...` 下的一个源文件，编辑器能显示内容。
- [ ] 远程起一个只听 `127.0.0.1` 的服务，用内置浏览器打开对应的 localhost 地址，页面来自远程。
- [ ] 对原生 Windows 远程的连接被拒绝，并提示改用 WSL 或 Linux。
- [ ] 计划完成后直接再提问，Build 卡片关闭且不开始执行。
