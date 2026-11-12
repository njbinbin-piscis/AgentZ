# AgentZ 工具摩擦问题清单（Piscis 自用·常设）

> 每次任务结束追加：现象 / 根因(file:line) / 修复方案 / 状态。
> 工具实现主体：`vendor/piscis-engine/piscis-kernel/src/`（shell/file/web/plan）、
> `vendor/piscis-engine/piscis-ide-tools/src/`（lsp）；`src-tauri` 负责审批与 devenv。
> 修复执行跟踪：`.agentz/plans/koi-fix-and-tool-friction.md` Step 7。

## 2026-11-12 会话（工具体检 / 远程同步 / Koi 审计 / 计划制订）

### F1. shell 超时参数名不匹配 → 长命令必 120s 超时【高优先】
- 现象: 传 timeout_secs: 300/600 仍报"工具 'shell' 执行超时（120秒）"（npm i -g、rustup、大文件下载均中招）。
- 根因: shell 只读 input["timeout"]（tools/shell.rs:429-432，schema :180-182），不认 timeout_secs；默认 120s、上限 300s（:21-23）。loop 级 tool_timeout_for 认 timeout_secs（agent/loop_.rs:67-76）但工具内部先超时。
- 修复: 参数解析兼容 timeout_secs/timeout_ms 别名；MAX_TIMEOUT_SECS 提到 600；错误文案带实际生效值。
- 状态: ✅ 已修复（2026-11-12）：shell.rs 解析 timeout→timeout_secs→timeout_ms（ms 向上取整），Windows MAX_TIMEOUT_SECS 300→600，schema 描述同步。

### F2. PowerShell 原生命令 stderr → 误报 tool_failed【高优先】
- 现象: git fetch 成功但 stderr 有进度输出 → NativeCommandError → tool_failed / exit -1。
- 根因: 失败判定只看退出码（tools/shell.rs:591-624、tool_recovery.rs:156-164）；PS 5.1 在 ErrorActionPreference=Stop 下原生命令写 stderr 以 1 退出。
- 修复: PS 包装脚本显式 ErrorActionPreference=Continue 并 exit $LASTEXITCODE；或优先 pwsh 7。
- 状态: ✅ 已修复（2026-11-12）：PS 包装前缀加 `$ErrorActionPreference='Continue'`，尾部 `exit $LASTEXITCODE` 保留真实退出码。

### F3. .agentz 路径守卫无豁免 → 自己的 spill 日志读不了【中优先·自相矛盾】
- 现象: 存 .agentz/dsh-docs/ 后 file_read 被拒；工具提示"Full output saved to .agentz/logs/tool-output/…用 file_read 读"，但该路径同样被守卫拒绝。
- 根因: runtime/approval.rs:1008-1060 黑名单 .git|.agentz|.codez|.piscis|.secret_key|.env，唯一豁免 .agentz/plans/*.md（:1042-1045）；spill 落盘 tools/output.rs:239-281。
- 修复: 豁免 .agentz/logs/tool-output/（官方推荐读取路径）；凭据类保持拒绝。
- 状态: ✅ 已修复（2026-11-12）：approval.rs validate_source_path 增加 `.agentz/logs/tool-output/` 前缀豁免 + 单测 tool_output_spill_is_readable_but_other_agentz_logs_are_not。

### F4. web_fetch 长行 1000 字符硬省略 → base64/JSON 不可用【中优先】
- 现象: GitHub API base64 单行被 middle-elide 不可解码；GitHub HTML 长行同样被截。
- 根因: tools/output.rs:108-137 MAX_LINE_CHARS=1000 常量，smart_truncate 无条件执行（:144），无参数可关。
- 修复: web_fetch 增加 raw/max_line_chars 参数（默认不变）；或对 base64/JSON 自动放宽。
- 状态: ✅ 已修复（2026-11-12）：output.rs 新增 cap_long_lines_with / smart_truncate_with_line_cap；web_fetch 增加 raw 参数（true 时不做行内省略，总预算仍生效）。

### F5. LSP 报 rust-analyzer 未安装（文件存在）【高优先·环境+文案】
- 现象: ~/.cargo/bin/rust-analyzer.exe 存在但 LSP/devenv 报 missing。
- 根因: 该 exe 是 rustup 代理；lsp/manager.rs:250-258 要求 runs_ok("--version")（:550-559），组件缺失时代理非零退出被过滤。rustup component add 本次因 F1 超时未装成。
- 修复: 环境装组件；文案区分"代理存在但组件缺失"，直接给 rustup component add 指令。
- 状态: ✅ 已修复（2026-11-12）：环境侧 rustup component add 后 rust-analyzer 1.96.0 可用，LSP hover 恢复；代码侧 manager.rs 错误文案区分代理存在但组件缺失。

### F6. devenv 探测不到 npm 全局 typescript-language-server【中优先】
- 现象: npm i -g 成功、npx 可运行，devenv 仍 missing。
- 根因: commands/devenv.rs:1003-1043 静态目录扫描 + run_version 需成功退出；.cmd shim 依赖 node 在 PATH；不查 npm root -g / 项目 node_modules（LspManager 却查，manager.rs:214-227）。
- 修复: 增加 npm root -g 动态解析与项目本地 .bin；shim 失败报告原因。
- 状态: ✅ 已修复（2026-11-12）：devenv.rs probe 增加项目 node_modules/.bin + npm root -g 动态全局目录（OnceLock 缓存）；找到但运行失败时 reason 注明路径与可能原因。

### F7. plan_write 校验字段试错 3 次【低优先·规则已掌握】
- 根因: agent/plan_doc.rs:159-187 硬性要求 **状态**/**描述**/**预期产物**/**验收证据** 四字段；schema 描述未列全。
- 修复: 错误信息聚合报告全部缺失字段；schema 描述列全。
- 状态: ✅ 已修复（2026-11-12）：plan_doc.rs validate_plan_content 聚合所有 Step 的全部缺失字段一次性报告 + 单测。

### F8. file_edit 混合行尾匹配失败【低优先】
- 根因: tools/file_write.rs:538-552 已有 EOL 归一化，但 file_is_crlf 按全文件含任意 CRLF 判定（:542），混合行尾文件的 LF 区段被错误转 CRLF。
- 修复: 按片段所在区段行尾适配，或 LF/CRLF 双向尝试。
- 状态: ✅ 已修复（2026-11-12）：file_write.rs EOL 适配改为双向尝试（主样式不匹配则试另一样式），new_string 按实际匹配区段样式转换 + 混合行尾单测。

### F9. 危险命令无确认通道时直接拒绝【关闭·符合设计】
- 根因: policy/gate.rs:39-52 DESTRUCTIVE_PATTERNS + agent/loop_.rs:2639-2656：headless/子代理无 confirmation_responses 直接拒绝；前台有 60s 确认弹窗（:2658-2696）。
- 结论: 安全设计正确。本次用备份分支+cherry-pick 可逆方案替代。

### F10. 输出展示截断与 spill【信息·随 F3 解决】
- stdout 24KB / stderr 12KB 展示预算（shell.rs:581-582），超出落盘 .agentz/logs/tool-output/（保留 30 份，output.rs:275）。读取问题随 F3 豁免解决。

### F11. file_diff 预览语义是整文件替换【高优先·易误用】
- 现象: 对已有文件传"追加片段"作 new_content，预览显示删除全部原内容（879 行 known-issues.md 差点被覆盖）。
- 根因: file_diff 预览模式按"new_content 即完整新文件"生成 diff，无追加语义；工具描述未明示。
- 修复: 工具描述明确"new_content 必须是完整文件内容"；或增加 append 模式。
- 状态: ✅ 已修复（2026-11-12）：file_diff.rs description 明示 new_content 为完整文件替换语义。

### F12. plan_write 成功文案诱导违反 Plan 流程【高优先·已证实】
- 现象: plan_write 成功后 agent 用文字询问"是否切换 Agent 模式"，而未调用 plan_mode_ui(action=plan_ready) 展示 Build 面板。
- 根因: tools/plan_write.rs:113-116 成功返回硬编码"Plan 模式完成后请总结 trade-off 并询问用户是否切换到 Agent 模式执行"——与 harness 规定（plan_write 后立即 plan_ready、禁止文字询问切换模式）直接矛盾，工具输出诱导 agent 违规。
- 修复: 文案改为引导立即调用 plan_mode_ui(action=plan_ready)；删除切换模式询问。
- 备注: 原 F12"工具面缺失 file_write/file_edit/shell"为误诊——Plan 模式只读工具面是设计使然，已撤销。
- 状态: ✅ 已修复（2026-11-12）：plan_write.rs 成功文案改为"请立即调用 plan_mode_ui(action=plan_ready) 展示 Build 按钮…不要用文字询问是否切换模式"。

## 修复优先级
1. F1+F2（消除最多失败重试）→ 2. F3（自相矛盾）→ 3. F5（恢复 Rust LSP）→ 4. F11+F12（防误覆盖/流程诱导）→ 5. F4/F6/F7/F8。

### F13. Chromium 进程死后浏览器永久卡死【高优先·已修复】
- 现象: browser 工具/面板报 "browser did not open a page in time"（15s）或 "Request timed out"，重启 IDE 前永远无法恢复。
- 根因: robotz-browser（git 依赖 v0.1.14）`is_running()` 只查 `browser.is_some()`，`launch()` 在 Some 时直接返回 Ok。Chromium 进程死亡（崩溃/外部 kill/系统休眠）后句柄仍在，所有 CDP 调用挂起超时，且永远不会重新拉起。日志证据：04:29 启动成功后，后续失败均无新启动日志。
- 修复: AgentZ 包装层（src-tauri/src/browser/mod.rs）bind() 与 with_active_page_timeout() 增加"失败→mgr.close() 丢弃死实例→重试一次"；错误信息附带首次与重试错误。
- 状态: ✅ 已修复（2026-11-12），待重新构建 IDE 生效。
