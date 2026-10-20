# AgentZ 1.0.3 发布说明

发布日期：2026-10-21。版本号与 `CHANGELOG.md` 的 1.0.3 条目一致。

AgentZ 1.0.3 修复项目模板（`superpowers` 等全部 9 个内置模板）套用后未写入 `.agentz/rules/*.md` 的问题，去除模板选择对话框与模板描述中的无关外部品牌字样，并增强 Agent 编码工具层的自愈与跨平台兼容性。

## 修复与增强

- **项目模板闭环生效**：锚定 `.gitignore` 根目录规则，补齐全部 9 个内置项目模板的 `rules/*.md` 规则文件；修复 `project_apply_template` 与 `project_has_agentz`，支持对仅有空 `.agentz` 目录的文件夹重新应用模板，并在「设置 → 项目规则」中新增「套用项目模板…」入口。
- **去除无关品牌字样 & 校准 `using-superpowers` 技能**：移除项目模板对话框及 9 个模板描述中的 `CodeBuddy` 字样；修正 `using-superpowers/SKILL.md` 使其使用 AgentZ 原生的 `file_read` + `plan_todo` 工具链。
- **`file_edit` 自愈上下文提示**：`MatchNotFound` 时自动在报错提示中附带目标文件最接近的带行号代码片段；`MatchAmbiguous` 时直接返回各处匹配的 1-based 行号，省去额外 `file_read` 往返。
- **`file_search` 支持 `include_ignored`**：新增 `include_ignored: boolean` 参数，并在默认返回 0 结果且跳过排除目录时主动提示被跳过的目录名。
- **Windows `shell` 兼容 PowerShell 5.1 `&&` 链式语法**：自动将顶层未加引号的 `cmd1 && cmd2` 改写为 PowerShell 5.1 兼容的条件执行链。
- **`terminal_read` 支持 `wait_ms`**：允许在读取 PTY 终端输出前按需等待（`0..=15000ms`），减少空转轮询。
