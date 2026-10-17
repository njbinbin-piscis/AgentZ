# AZ-043 实现审查记录（r2）

审查者：independent-reviewer-135fa3b0（独立于 owner Codex）
范围：调试会话生命周期（preparing）。本记录不是正式 Gate 验收。

清单：`docs/upgrade/evidence/AZ-043-implementation-manifest-r2.json`（11 个源文件，SHA256 与审查时一致）。

## 结论

PASS（非阻塞）。

## 已修复的阻塞项（前一轮 BLOCK）

1. 孤儿 start：`stopIfProjectChanged` 在项目变化时递增 epoch；`start()` 在 `await cleanup` 之后检查 epoch，变化则放弃，不创建会话。
2. 陈旧 projectDir：控制器新增同步的 `currentProject`，`start()` 优先使用它，不再依赖防抖后的 `extensionService.projectDir`。
3. `ExtensionHostProvider.tsx` 中停止调用移到 `useLayoutEffect`，在提交阶段同步执行。

## 验证

- `npm run typecheck` 通过。
- `npx vitest run src/extensions/debug`：22 项通过（dapClient 9，debugController 13）。
- 本轮复核命令：`npx vitest run src/extensions/debug src/services/tauri/lsp.test.ts src/services/tauri/lspSession.test.ts src/workspaces/codez`，18 个文件，135 项通过。

## 非阻塞项

- `debugController.test.ts`：“同步停止”测试中调用是 await 的，若将来在 `stop()` 前加 await，该测试不会失败；start 与 cleanup 并发的情况尚无测试。
- 残余窗口：`switchProject` 先 `setProjectDir`，提交前旧会话仍可接收 DAP 事件并向旧适配器发送 `stackTrace` 等请求。建议在 `setProjectDir` 之前同步调用 `stopIfProjectChanged`。
- `mainThreads.ts:595`：旧扩展宿主在布局效果之后、宿主重启之前发出的 `startDebugging` 会被打上新项目标签，而其 cwd 与程序仍指向旧项目。可通过拒绝 cwd/程序不在 `currentProject` 之内的配置来关闭。

以上非阻塞项不影响本次实现审查结论，计入后续 AZ-043 工作。
