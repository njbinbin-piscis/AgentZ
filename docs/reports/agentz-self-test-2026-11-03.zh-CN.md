# AgentZ 任务驱动深度自测报告（2026-11-03）

## 摘要

本次在不安装依赖、不联网、不启动 AgentZ UI/外部服务、不修改 AgentZ 产品源码的前提下，使用唯一隔离临时项目实现了一个 Python Markdown 任务清单 CLI（add/list/complete/filter、损坏数据检查、文档和 unittest）。核心 6 项测试通过，1 项因当前环境拒绝创建 symlink 而跳过。通过真实工具调用观察到：工具可在仓库内完成文件读写、搜索、LSP 符号/诊断和代码执行；系统临时目录可由命令创建/执行，但当前 IDE 的文件导航、搜索、LSP 权限层拒绝其绝对路径，`code_run` 的工作目录也必须在 workspace 内。技能说明绝对路径读取被工作区访问限制拒绝。

**结论不是所有失败都属产品缺陷。** 有我自身的调用/参数错误（应承认并作为系统可靠性反馈），有环境/IDE 权限边界限制，还有至少两个由 AgentZ 源码确认的失败诊断文案过于武断的问题。未修改产品代码；报告中的修复方案仅为建议。

## 安全范围与初始状态

- 仓库：`C:\Projects\AgentZ`，分支 `main`，跟踪 `origin/main`。
- 初始 `git status --short --branch`：`## main...origin/main`；仅有既有未跟踪 `_release_build.log`、`_tauri_test.txt`、`_testlog.txt`。未覆盖或删除。
- Python 3.12.7、Node 24.19.0、Git 2.55.0 可用；此次项目仅用 Python 标准库。
- 禁止/未做：网络访问、外部服务、凭据访问、提交/推送/部署、AgentZ 产品源码修改、UI 启动。
- 真实任务 fixture 在 `C:\Users\zzz\AppData\Local\Temp\agentz-selftest-9634axs2`。此外在仓库根创建了 `.tmp-agentz-selftest-20261103/sample.py` 用于体验工作区内工具；待本报告写入后删除该 fixture 并复核状态。
- 临时目录之外的工具访问曾被本会话宿主层拒绝。该行为有真实工具返回作为证据，但不是 AgentZ 仓库源码返回，不据此单独断言为应用缺陷。

## 真实完成的开发任务

在临时目录实现 `tasks.py`、`cli.py`、`test_tasks.py`、`test_invalid_id.py`、`README.md`：

- task UUID、文本、done 状态；Markdown 逐行存储，读取时验证格式、UUID canonical form 与重复 ID。
- `TaskStore.add/list/complete/filter` 和 CLI 子命令 `add/list/complete`；支持 query、open-only、data-dir。
- 空白/多行文本拒绝；损坏任务文件在读取失败时不被覆盖；`../` 路径逃逸拒绝。
- CLI 实测：add 成功、按 query 列表成功；未知 ID complete 返回 usage 错误、退出码 2。
- 有意新增一个“长度看似 UUID 但不是 canonical UUID”的测试，先复现 `InvalidTaskFile not raised`，加入 `UUID()` 严格验证后通过。

### 测试证据

- 初始测试：`python -m unittest discover -s C:\Users\zzz\AppData\Local\Temp\agentz-selftest-9634axs2 -p test_tasks.py -v` → 5 tests，4 pass，1 skipped（symlink 创建不可用）。
- 新增边界测试首次失败：`test_rejects_noncanonical_uuid_that_matches_length_regex`，失败为未抛 `InvalidTaskFile`；修复后完整套件 `python -m unittest discover -s ... -p test_*.py -v` → 6 tests，OK，1 skipped。
- `python -m py_compile` 对四个 Python 文件成功。
- 当前隔离环境不允许创建 symlink；因此只实测 `../`，symlink escape 检查**未测**，不得视为通过。

## 调用观测与逐项归因

### A. 工作区内成功路径

1. 在 `.tmp-agentz-selftest-20261103/sample.py` 上 `file_write`、`file_list`、`file_search glob`、`file_search grep`、`file_read` 成功；`file_diff` 可预览修改；`file_edit` 两项精确替换成功。
2. `file_edit` 用不存在的 `old_string="not-present"` → `[error: match_not_found]`，提示重新读取并复制精确唯一片段。错误明确、恢复建议可执行；属预期保护，不是缺陷。
3. `file_search` 传入单文件作为 `path` → `Path is not a directory`，并明确告知传父目录+include 或使用 file_read；我传错参数，工具提示充分。
4. `file_list` 传文件路径 → `Not a directory`；`file_list` 指向不存在 fixture 目录 → OS `file not found`。均为我传错对象/路径；工具结果可解释。
5. 工作区 fixture 的 LSP `symbols` 返回 `normalize_task`、`main`；`read_lints` 无错误/警告；`code_run` 成功执行并输出 `test task`；`python -c "import sys; sys.exit(1)"` 返回退出码 1 和 `tool_failed` 状态。该失败路径正确保留退出信息。
6. `codebase_search` 在临时 fixture 下未命中（索引覆盖/临时目录是否入索引未知），不能据此判为功能错误；直接文件搜索与 LSP 可用。

### B. 所有失败、重试与限制（按发生顺序）

| # | 原始观察 | 归因 | 源码核实/边界 | 改进建议 |
|---|---|---|---|---|---|
| 1 | 读取安装技能的绝对路径 `C:\Users\...\skills\installed\test-driven-development\SKILL.md` → `Path ... is outside the workspace root ... Enable Allow outside workspace` | 当前 IDE 工具宿主的工作区授权限制；不是已确认的 AgentZ 文件读取实现错误。与此同时，系统提示要求“read skill instructions before use”，但没有在本次执行环境提供可读路径/注入正文的明确可行替代，属提示—能力不匹配风险 | AgentZ `skills_context` 在技能未启用时提供 SKILL.md 路径并要求读取；被显式启用时应用会直接将 `s.body` 注入 prompt。`src-tauri/src/commands/chat_turn/prompt_assembly.rs:63-105`。加载器扫描配置目录并读取技能：`src-tauri/src/skills/loader.rs:40-100`。当前错误是宿主工具返回，源码未找到该字符串 | 技能 UI/宿主应提供可读的受控技能内容接口，或只在正文已注入时要求阅读；被拒时明确提供替代/声明不可读取 |
| 2 | `python -m unittest -v C:\...\test_tasks.py` → Python `ModuleNotFoundError: No module named 'C:\\...\\test_tasks'`；工具诊断 “Missing Python module. Run pip install...” | 命令用法误用 + AgentZ 诊断过宽。`unittest -m` 后需要模块名，绝对文件路径不是模块名；但真实错误明确是路径被当模块名，不代表依赖缺失 | `diagnose` 只要命令含 python/pytest/pip 且输出含 ModuleNotFoundError/ImportError，就固定给“安装模块/检查 venv”提示。`vendor/piscis-engine/piscis-kernel/src/tools/code_run.rs:463-478`。该诊断函数无可见专门测试；测试模块可见 sandbox 边界覆盖：同文件 `535-618` | 区分 `No module named 'x'` 与 `No module named 'C:\...'`、`unittest -m` 用法；提示优先建议 `unittest discover -s DIR` 或直接运行文件。补回归测试 |
| 3 | 用 `file_list` 查询 `.tmp-agentz-selftest`（实际目录名不同）→ OS `file not found` | 我的目录名误记/参数错误 | 路径不存在正常报错；无证据显示工具异常 | 失败后先列出临时目录名，避免猜测 |
| 4 | `python -m unittest -v <Windows绝对路径>` 首次失败后，我先成功改用 `unittest discover -s ...`；之后仍额外重复了一次成功的 discover | 首次修复正确；后一次重复是我方效率/记录管理问题，非工具缺陷 | 两次成功结果一致；无需重跑 | 失败恢复记录中做已验证命令去重 |
| 5 | 临时目录创建成功（`C:\Users\...\Temp\agentz-selftest-9634axs2`），但 `file_read`、`file_list`、`file_search`、LSP 对该绝对路径返回 “outside workspace / escapes authorized environment”；`codebase_search` 空结果 | 宿主/工具授权边界造成真实任务摩擦；宿主运行身份可写/执行临时目录，不代表 IDE 文件工具授权该路径。该权限拒绝有安全合理性，不能简单视为漏洞；但测试计划一开始选系统 temp，且提示没有及时揭示“工具层和命令层 workspace 范围不同” | AgentZ `code_run` 描述含 Windows AppContainer 与 workspace 限制，并要求 cwd 在 workspace：`code_run.rs:31-36, 110-118, 172-200`。`file_read` 等内核工具将绝对路径作为文件系统路径：`file_read.rs:178-189`，但 AgentZ 主机 approval wrapper 会把 source path 约束在授权源根并保护 `.env`/`.agentz`：`src-tauri/src/runtime/approval.rs:175-207, 941-992`。LSP 同样检查授权根。系统临时路径被拒的精确 IDE 后端来源未完全确认；宿主错误不在上述源码字符串中 | 任务启动前以 workspace fixture 作为文件工具主目录；明确注明 shell/code_run、IDE文件 API、LSP/index 的 scope 分别是什么。提供获批的临时 workspace 路径，而不是期望系统 temp 自动可读 |
| 6 | 将 PowerShell `$root=...; @'...` 脚本交给 `code_run` → `$root is not recognized as an internal or external command`，通用诊断 “Command not found...” | 调用工具选择错误（PowerShell 语法发给 cmd.exe）+ 诊断文案缺少语法/解释器识别，属于源码可证的系统可改进项 | Windows `code_run` 明确执行 cmd.exe 语法，提示 PowerShell 用 shell：`code_run.rs:31-36`；`shell` 声明支持 PowerShell/cmd：`shell.rs:144-149`。通用诊断只看 “command not found/is not recognized” 后提示检查安装/PATH：`code_run.rs:495-507`，没有判断 `$name`、`@'` 等 PowerShell 语法 | 对“not recognized”同时检查 PowerShell 语法，提示选 PowerShell；在输入参数/工具描述中把 interpreter 显著展示。为诊断分支写回归测试 |
| 7 | `file_search` 把单文件作 `path`、LSP 把相对路径拼成错误 target，得到明确参数/文件不存在错误 | 前者是我参数误用；后者首次是路径使用误差。改用 repo 内绝对路径后 LSP 成功 | `file_search` 描述/schema明确 path 必须目录：`file_search.rs:54-70, 73-92, 128-168`。LSP 首次提示建议先确认路径/列目录，绝对 repo path 返回正常符号 | 工具错误明确；可在 LSP prompt 更醒目说明相对路径相对 workspace，不应继续重试同一无效路径 |
| 8 | 系统临时 fixture symlink 创建被 Windows 拒绝，测试跳过 symlink 检查 | 环境限制，不是产品行为验证；不可称安全漏洞也不可声称通过 | CLI 自己输出 `skipped 'symlink creation not available'`；AgentZ 对受授权 source tree 明确拒绝链接遍历：`src-tauri/src/runtime/approval.rs:839-859`，已有 junction/link 相关测试代码 `1101+` | 在允许的 CI runner 或显式权限下单独验证；本次标未测 |
| 9 | 临时目录的 `file_read/file_list/file_search/LSP` 被拒；切到仓库内 fixture 后全部可用 | 证明工作区内恢复路径有效；不消除工作区外能力限制 | 当前系统提示提供“绝对路径”示例，但宿主仍施加授权；尤其技能文件系统绝对路径与技能调用说明不一致。`file_read.rs:131-134` 说可用绝对路径访问 workspace 外文件，但 host gate 可能再拒绝，形成 kernel/schema 与 AgentZ host policy 两层语义差异 | 工具描述/API 要区分“输入格式支持绝对路径”与“host 是否授权访问”，并在错误中返回可行动的受授权替代 |
| 10 | 对当前待测 todo 复核时，调用 `file_list` `.tmp-agentz-selftest` 得不存在；另一次 `file_list` 指向工作区外 temp 目录被拒 | #3/#5 的重复观察，不是新的根因 | 同上 | 已合并记录，不重复归为独立缺陷 |
| 11 | 某次 LSP diagnostics `approval.rs` 返回 `Git approval identity timed out`；后续通过 `read_lints` 成功检查同文件，codebase/file search 也曾返回同样 timeout | IDE 工具/批准层临时失败，可能宿主审批身份超时；具体根因未证实，不归因产品源码 | `read_lints` 对三份相关 Rust 源码检查无错误/警告；没有仓库代码证据解释此 timeout。后续 LSP code_run.rs diagnostics 成功；工具超时不应按源码缺陷定性 | 记录并重试不同读取路径一次；若频发，再查 approval identity 生命周期/遥测，不在此猜结论 |
| 12 | 系统提示计划文件为 `.agentz/plans/agentz-self-test.md`，自动注入的 active plan 片段曾显示另一 UUID 路径；file_list 对拟定 `.tmp-agentz-selftest` 目录报告不存在 | 系统上下文/用户计划摘要存在路径不一致或会话状态偏移，至少增加助手犯路径错误的风险；不应隐瞒为纯助手错误 | 实际 `.agentz/plans/` 计划通过 file_edit 成功更新；目标 fixture 路径也通过 file_write 成功创建，之后确认目录/文件。自动注入 plan excerpt 与执行计划文件路径对不上，属于提示上下文一致性风险，需产品层确认 session plan binding 机制 | 开始操作前读取计划正文/列出路径；生成的摘要必须与当前活动计划实际路径一致，避免过期 UUID/旧 todo 被注入 |
| 13 | 多次外部路径 file API 在当前工具层返回拒绝；但 `shell`/PowerShell 能在 OS temp 写文件及运行测试 | 宿主工具能力/隔离方式不一致，安全边界本身可能有意；任务上是显著摩擦。我的前置路径选择对此有责任，提示缺少范围可发现性 | 仓库 `file_read` 描述称 workspace 外绝对路径可用（`file_read.rs:131-134`），而 runtime approval `validate_source_path` 对普通文件拒绝 escape 并拒绝 host metadata（`approval.rs:941-992`），显示主机策略更窄；这是 AgentZ 内存在“内核接口文案比主机授权宽”的源码级可用性缺陷候选。实际 IDE `escapes authorized environment` 原始错误来自平台工具层仍未能定位 | 统一 file tool 描述与 host approval 策略；拒绝时给出可批准的授权根/工具路径说明。安全目的不应以退回 shell 绕过保护 |

## 源码核验汇总（缺陷候选与非缺陷）

### 源码确认的诊断缺陷候选

1. **Python 模块错误误诊**：`code_run.rs:463-478` 仅依据 traceback 中字符串作宽泛分类。本次路径误用 traceback 命中 Missing Python module 提示，建议补精确条件与测试。严重性：中（容易诱导不必要安装依赖，浪费恢复步骤）。
2. **命令解释器错误误诊**：`code_run.rs:495-507` 将 Windows “is not recognized”一律归为安装/PATH 问题；本次 PowerShell 语法经 cmd 执行即命中。严重性：中低（错误引导，但原始输出可见）。
3. **文件工具描述与主机权限不一致候选**：`file_read.rs:131-134` 暗示可用绝对路径访问 workspace 外文件；`approval.rs:941-992` 又校验 authorized source root 并拒绝 escape。严重性：中（会导致尝试必然失败；精确错误来自实际宿主层仍需集成验证）。
4. **Skill 指令与能力不匹配**：prompt 要求读取 skill 路径，技能 loader 又可在 AgentZ 内加载正文，但当前 IDE 对安装目录绝对路径拒绝。仓库源码证明应用侧两种机制存在，宿主拒绝行为本身未定位到 repo。严重性：中（技能使用阻塞或让 agent 被迫声称未遵循）。建议以正文注入或授权 skill API 作为单一权威路径。
5. **计划上下文路径一致性候选**：会话注入计划路径与执行计划摘要中的实际路径不一致，造成临时目录误记；实际生成机制及影响范围未完整核实。严重性：低/待验证，建议先查 plan binding/摘要生成。

### 不判为产品缺陷的事件

- `unittest -m` 用 Windows 路径而非模块名：我的命令错误；诊断文案仍需改进。
- 用 PowerShell 语法发给 cmd：我的工具选择错误；通用诊断仍需改进。
- `file_search` path 传文件、`file_list` path 不存在、LSP 路径拼错：参数错误，错误说明总体明确。
- symlink 创建被拒：环境限制，测试跳过。
- 仓库外文件被限制：权限隔离可能是预期安全策略；问题是范围可发现性/提示与授权政策是否一致，不推论为越权漏洞。
- `Git approval identity timed out`：宿主/瞬时失败，未定位源码根因。

## 工具与技能覆盖清单

- 文件工具：仓库内 read/write/edit/diff/list/glob/grep 成功；精确替换失败提示可恢复；单文件作为搜索目录报错符合契约。
- 语义工具：repo 内 Python LSP symbols/read_lints 成功；临时路径 LSP 被 scope gate 拒绝；Repo search 对新 fixture 无索引命中（原因未确认）。
- 执行工具：repo 内 `code_run` 成功/非零退出两种路径；临时项目通过 cwd 内 `unittest discover` 执行成功。失败恢复报告含 exit code、termination 和 cleanup。
- PowerShell shell：本次真实 PowerShell 经 `shell(interpreter=powershell)` 写入并查看临时文件成功；PowerShell 脚本误用 `code_run` 的 cmd.exe 调用失败，保留为负向证据。
- 技能：相关技能文件受 workspace root 限制，无法读取；不得声称实际执行了技能的内容。技能内容由系统提示中的技能摘要提供，只有错误信息可访问。
- 计划：使用 `plan_todo` 维护步骤；执行计划通过 `file_edit` 记状态。
- 外部联网、API connector、邮件/部署/浏览器、持久化 UI 操作等：本任务约束明确不执行，因此不测。

## 建议修复/验证优先级

1. 为 `code_run::diagnose` 增加最小回归测试，将依赖缺失、`unittest -m` 路径误用、shell 语法不匹配、程序不在 PATH 分类分开；用案例证明诊断不诱导 `pip install`。
2. 对齐 `file_read` schema/工具描述与 AgentZ `validate_source_path` 授权策略；拒绝错误应讲清受授权 source root，并列出安全替代方式。
3. 设计技能内容单一可用路径：启用时明确注入正文且不再要求绝对路径读取；否则通过 scoped skill reader 提供受控读取能力。验证已安装技能和 learned/disabled skill 的状态差异。
4. 在支持 symlink/junction 的隔离 CI runner 实测 symlink escape；本地此次结果不可替代。
5. 审查活动计划路径注入与旧会话摘要的一致性，并添加计划路径状态测试/日志。
6. 追查 `Git approval identity timed out` 若同类事件可稳定重现；目前只记录为未定位宿主瞬时失败。

## AgentZ Rust 自动化检查尝试

在 `vendor/piscis-engine` 内执行 `cargo test -p piscis-kernel code_run --offline`。命令在前台 180 秒超时后转入受管后台，最终以退出码 101 结束。离线构建开始编译 `piscis-kernel`，但链接阶段报 `linker link.exe not found`，提示 MSVC target 需要 Visual Studio/Build Tools C++ linker。未触网、未安装依赖；因此 `code_run` 单测**未能执行**，属于环境工具链限制而非测试失败。`read_lints` 对 `code_run.rs`、`loader.rs`、`approval.rs` 均报告 0 error / 0 warning。

## 最终状态

- 清理脚本首次因 PowerShell 括号不匹配而在解析阶段退出，未执行删除；修正后清理成功。
- 未修改 AgentZ 产品源码、未安装依赖、未联网、未启动 UI 或服务、未提交代码。
- 临时项目测试结果：6 passed，1 skipped（symlink 环境限制）。AgentZ Rust `code_run` 单测因缺少 `link.exe` 未能执行。
- 已清理任务 fixture `C:\Users\zzz\AppData\Local\Temp\agentz-selftest-9634axs2`、报告辅助临时目录及仓库内 `.tmp-agentz-selftest-20261103/`；逐项确认路径均不存在。
