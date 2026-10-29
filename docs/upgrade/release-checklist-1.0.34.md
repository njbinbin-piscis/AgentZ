# AgentZ 1.0.34 发布检查清单

2026-10-29。基于 1.0.33：子代理超时与结论、子代理设置项、中止后工具气泡保留。

## 发布前检查

- [x] 版本号统一：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock`（仅 `agentz-desktop`）、`README.md`、`RELEASE.md`。
- [x] `CHANGELOG.md` 与 `docs/upgrade/release-notes-1.0.34.md` 已更新。
- [x] `tsc --noEmit`、`cargo check --tests` 通过；`subagent_final_report` 2 项、`keepLiveToolSteps` 3 项测试通过。
- [ ] `node scripts/upgrade-version-check.mjs` 通过。

## 安装后人工验收

- [ ] 委派子代理运行超过 150 秒不再报"执行超时（150秒）"。
- [ ] 子代理步数用完时返回结论报告，而不是"[sub-agent returned no findings]"。
- [ ] 设置 → Agent 页可配置子代理迭代次数与超时，保存后重新打开仍保留。
- [ ] 运行中点中止后立即发送新消息，上一轮的工具调用气泡仍然显示。
