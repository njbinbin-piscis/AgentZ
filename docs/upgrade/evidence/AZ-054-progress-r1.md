# AZ-054 进展证据 r1：Windows 桌面操作最小闭环（未收口）

ledger 状态：`pending`（未分配 owner）。`update AZ-054 --status doing` 被拒绝，原因是未满足依赖与门禁：AZ-034、AZ-053 未完成，M2/M3/M4 未通过。

本文件记录的是工作区中已完成且已验证的代码层修复（未提交），不构成验收，也不代表 `done`。在依赖解除或用户明确批准绕过门禁之前，ledger 不会推进该任务。

## 1. 范围与代码位置

仓库：`C:/Projects/RobotZ`（未提交工作区，未 git commit/push）。

- `crates/robotz-automation/src/uia.rs`
- `crates/robotz-automation/Cargo.toml`（新增 windows feature `Win32_System_SystemInformation`，用于 `GetTickCount`）

## 2. 对照 rebaseline 修复要求

| 要求 | 状态 | 说明 |
|---|---|---|
| 1) 移除 `type_text` 的全局 `root2.send_keys` 回退 | 已完成 | 找不到目标元素时直接拒绝，不向前台窗口发送任何按键 |
| 1) 同上，`send_keys_action` | 已完成 | 必须指定 `name` / `class_name` / `automation_id`，否则拒绝；不再发送到焦点窗口 |
| 2) 每次输入前校验前台窗口句柄与目标一致，不一致则中止 | 已完成（type / send_keys / 带 `window_title` 的 hotkey） | 返回 `focus lost: ... stopped without sending input` |
| 3) 拒绝向受保护窗口输入 | 部分完成 | 已拒绝前台为 `Credential Dialog Xaml Host` / `CredUIBrokerWindow` 的情况，并拒绝密码框（`IsPassword`，未知时按密码框处理）。安全桌面（UAC）无法从普通进程读取，依赖 ADR-008 不覆盖的原生验证，仍为残余风险 |
| 3) 应用与动作的显式白名单写入工具 schema | 未完成 | 属于产品决策，本轮未实现 |
| 4) 每次动作前检查 `GetLastInputInfo`，用户已交互则停止 | 已完成（基于时间的启发式） | 代理成功注入后记录 `GetTickCount`；之后用户输入（晚于记录时间 + 100ms 回显宽限）即拒绝。首次注入前无基线，不做接管判断 |
| 5) 受控应用测试：焦点丢失中途停止、回退路径拒绝、接管停止 | 部分完成 | 已有 14 个单元测试（纯决策函数 + 假元素）。真实受控 GUI 测试未做（需要原生 GUI 运行，属于 ADR-008 范围外的 GUI 验收，列为残余项） |

## 3. 关键实现说明

- 输入前统一经过 `input_guard::check`：前台窗口 → 受保护窗口类 → 用户接管。任一失败即返回错误，且不调用键盘注入。
- 只有成功的注入才会推进代理输入时间（`dispatch` 中对 `INJECTING_ACTIONS` 且 `!is_error` 才调用 `mark_agent_input`）。被拒绝的动作不会掩盖造成拒绝的用户输入。
- 接管检测使用 `GetLastInputInfo().dwTime` 与 `GetTickCount` 的差值，使用 `wrapping_sub` 处理 32 位 tick 回绕。
- `GetLastInputInfo` 失败时拒绝输入（fail closed）。

## 4. 验证结果

- `cargo test -p robotz-automation`：14 passed, 0 failed（`uia::type_text_tests` 全部用例）。
- `cargo clippy -p robotz-automation --all-targets -- -D warnings`：exit 0。

运行环境：Windows 10 (19045)，本地 RobotZ 工作区。

## 5. 残余风险与未覆盖范围

1. 鼠标注入（click / double_click / right_click / drag_drop / scroll 等）未做前台窗口校验，只在动作完成后记录代理输入时间。点击误入其他窗口仍可能发生。
2. `send_hotkey` 未带 `window_title` 时仍是全局按键，只做受保护窗口与接管检查，没有目标校验。
3. `crates/robotz-automation/src/desktop_automation.rs`（Linux xdotool / macOS 路径）的 `type_text` 与 hotkey 未改动，也未在本机编译或测试。
4. 接管检测是启发式：用户在代理注入后 100ms 宽限窗口内的输入会被归因为回显；首次注入前的用户输入不检测。
5. 白名单（应用与动作声明）未实现。
6. 受控应用 GUI 测试未运行；本证据不声明其通过。

## 6. 文件指纹（工作区当前字节）

- `crates/robotz-automation/src/uia.rs` SHA256 `f7a2f13e9565bb165c0a0d0b4c3052caf70ecfe114b63cbadf52b0a1887857dd`
- `crates/robotz-automation/Cargo.toml` SHA256 `c3a3e2ff663f0866be1e4ea9d1102128dac064881ba566dc1e8a63e2d503ed05`

## 7. 下一步

1. 将鼠标注入动作纳入前台窗口校验（或在每个注入点调用统一守卫）。
2. 在 schema 中声明应用/动作白名单。
3. 编写受控 Win32 测试窗口的集成测试（焦点丢失、接管、回退路径），在 GUI 环境中运行。
4. 完成后由独立复核人做 implementation review，再推进到 `review` / `done`。
