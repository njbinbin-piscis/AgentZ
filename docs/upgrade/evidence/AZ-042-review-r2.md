# AZ-042 实现审查记录（r2）

审查者：independent-reviewer-135fa3b0（独立于 owner Codex）
范围：语言服务崩溃恢复的实现（preparing）。本记录不是正式 Gate 验收，任务不计完成。

清单：`docs/upgrade/evidence/AZ-042-implementation-manifest-r2.json`（8 个源文件，SHA256 与审查时一致）。

## 结论

PASS（非阻塞）。

## 已核对

- 重连：握手失败时关闭 socket 并清空 `connectPromise`，重试使用新 socket（`lsp.ts` failConnection；测试 "retries a failed handshake"）。
- 文档重同步：`reconnect` 只重放已打开文档的当前未保存文本并重置版本（测试 "reopens current unsaved documents"）。
- 陈旧诊断：socket 关闭时清空并通知监听者 `[]`；旧 socket 的回调被忽略；push 与 pull 结果按版本校验（三条新增测试）。

## 非阻塞项

- `lsp.ts:334`：`sendDidChange` 每次编辑都清空诊断。`CodeEditor.tsx:609` 只在打开时请求 pull 诊断，纯 pull 服务器在首次编辑后会失去标记；push 服务器会重新发布，影响有限。
- `CodeEditor.tsx` `.catch`（约 611 行）：任何获取失败（包括“服务未安装”）都显示“语言服务不可用”，此前是静默降级。
- 证据文档 `AZ-042-lsp-recovery-candidate.md` 中“24 passed”与当前 27 项（17 + 10）不一致，且未写明具体命令，无法复现。

## 未验证

- vendored Rust 真实服务异常测试（ignored）与 Clippy 结论本轮未重跑。
- Monaco 窗口与健康横幅布局。
- 真实 TS/Rust 服务异常旅程的桌面验收，仍是 AZ-042 Gate 的剩余项。
