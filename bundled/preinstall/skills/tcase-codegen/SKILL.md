---
name: "tcase-codegen"
description: "TCase 自动化测试代码生成技能。当用户需要生成测试代码、自动化测试、根据UUID/文本用例/节点生成测试时触发。支持 uuid、text_case、node、standard 四种模式。"
description_zh: "AI 驱动测试用例生成与管理工作流，集成 TestBuddy 脑图可视化和 TCase 自动化测试代码生成能力"
version: "1.0.0"
source: codebuddy
source_plugin: "tsmart"
---

> **AgentZ note:** Configure external services under Settings → MCP Servers / Connectors. Use `api_connector`, `shell`, `file_read`, and `codebase_search` instead of vendor-specific MCP tool names.

# TCase MCP 工具调用技能

## 完整工作流

1. 环境准备
2. 参数组装
3. 判断是否为批量处理模式
4. MCP 调用与代码生成
5. 利用MCP返回的信息**直接执行**代码生成与文件写入
6. 上传（由 PostToolUse Hook 自动完成，无需手动操作）

---

## 路径约定

**注意**: `<skill_dir>` 是本 skill 加载时的 base directory 路径前缀（不要写死路径）。所有对 skill 内部文件（脚本、日志等）的引用，均使用 `<skill_dir>` 作为前缀。
**你可以打印出来 `<skill_dir>` 的值，以确认是否正确。**
---

## 执行前强制检查清单（不得跳过任何项）

**在调用任何MCP工具之前，你必须按顺序完成以下所有检查：**

### 检查点 1：用户环境信息
- [ ] 已执行 `python3 <skill_dir>/scripts/env_prepare.py || python <skill_dir>/scripts/env_prepare.py`
- [ ] 已获取 `user_id`（如果为空则停止，提示用户）
- [ ] 已确定 `user_repo`（如果有多个仓库，必须让用户选择）
- [ ] 已确定 `user_branch`

**[阻断] 如果任何一项未完成，不得进入检查点2。**

### 检查点 2：操作模式和核心参数
- [ ] 已确定操作模式（uuid/text_case/node/standard）
- [ ] 已提取模式所需的核心参数（uuid列表/用例文本/节点信息/RFC文档）
- [ ] 若为 text_case 模式，已阅读 `references/phase2_params.md` 中关于 `case_text` 的结构化与非结构化用例说明
- [ ] 已将用户原始问题完整复制到 `user_query` 参数

**[阻断] 如果操作模式不明确，必须询问用户，不得猜测。**

### 检查点 3：文件引用处理（关键步骤！）
- [ ] 已列出所有用户引用的文件（来自 `<attached_files>` 标签）
- [ ] 已分类：`.py` 文件或文件夹路径 → `code_path`，`.md/.mdc` 文件 → 约束文件
- [ ] **已检测约束文件**：所有 `.md/.mdc` 文件的绝对路径已追加到 `case_note`
- [ ] 约束文件格式正确：`[CONSTRAINT_PATH]{绝对路径}[/CONSTRAINT_PATH]`
- [ ] 已向用户确认用户主动引用的文件中，哪些会传递到MCP工具

**[阻断] 如果有约束文件但未追加到 case_note，不得调用MCP工具。**

### 检查点 4：参数完整性验证
- [ ] 所有必填参数已填充（根据操作模式）
- [ ] JSON格式已验证（花括号成对、无尾随逗号、引号正确转义）
- [ ] `case_uuid` 使用数组格式：`["uuid1", "uuid2"]`
- [ ] 已确认生成路径策略（用户指定路径优先；未指定时使用 MCP 返回路径）

**[通过] 完成所有检查点后，才能调用 `mcp__TCase__generate_code`。**

### 检查点 5：MCP 返回结构解读与代码生成验证（MCP 调用完成后执行）

> **收到 MCP 返回后，必须先阅读 `references/phase3_result.md` 中的「MCP 返回结构解读」章节，理解返回字段含义和 `code` 字段的两种正常形态（代码框架 vs 用例描述），再执行以下检查项。**
> 
> **关键提醒**：`status: "success"` + `code` 是纯文本用例描述 = **正常的纯文本用例返回**，不是错误！不要走规则5的错误处理路径。

- [ ] 已阅读 `references/phase3_result.md` 的「MCP 返回结构解读」，理解当前返回属于哪种形态
- [ ] 已从 `order` 字段提取代码模板规范（`忽略项目中原生的模版`以`order字段提供`为准）、工作流模式（file_append/create_file/dir_append）和目标路径
- [ ] 已判断 `code` 字段形态：代码框架（高分用例）→ 补全 TODO；用例描述（纯文本用例）→ 从零生成
- [ ] 已在项目中搜索 `code` 字段提到的业务动作、API 名称和相关线索；如果首次未直接命中，已继续搜索别名、封装层、调用链等间接实现，获取调用方式和导入路径
- [ ] **已按 `order` 模板规范生成完整可执行的测试代码**（不是转述 MCP 返回内容！）
- [ ] 代码质量检查通过：零 TODO、零 pass、导入完整、author 已替换为真实用户名
- [ ] **已调用 write_to_file / replace_in_file 将代码写入 `order` 指定的目标路径**
- [ ] **已调用 read_file 验证文件存在且内容正确**
- [ ] **确保`tcase_uuid`字段一定存在且为标准 UUID 格式**：优先复用 `design_case_uuid`——如果该测试函数的 docstring/元信息中已有 `design_case_uuid`，则 `tcase_uuid` 直接取与 `design_case_uuid` 相同的值，无需调用生成脚本；仅当测试函数没有 `design_case_uuid` 时，才提取函数名调用 `python3 <skill_dir>/scripts/generate_tcase_uuid.py <repo> <branch> '["func_name_1", ...]'` 批量生成；写入完成后必须校验所有 `tcase_uuid` 的值均为标准 RFC4122 UUID 格式（`xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`），非标准格式（如占位符）必须替换为真实 UUID 后才能通过检查

**[阻断] MCP 只返回指令和素材，不会生成代码也不会写入文件。你必须亲自完成代码生成和文件写入，否则不得进入检查点6。**
**[要求] 一次搜索无结果不是失败条件，必须继续沿别名、封装层、调用链和相似测试扩展搜索后再完成代码生成。**

### 检查点 6：生成完成确认
- [ ] 所有测试文件已成功生成并**经过 read_file 验证确认存在**
- [ ] 已按固定话术向用户反馈：`测试代码已生成并同步到TCase系统。`

**说明**：上传由 PostToolUse Hook 自动触发，write_to_file / replace_in_file 写入包含 tcase_uuid 的文件后会自动后台上传，无需手动执行。

**[严重] 如果检查点5未完成，整个任务视为失败！**

---

## 关键规则（必须遵守）

### 规则1：MCP 返回的 order 字段优先级最高

**生成代码时，MCP 返回的 `order` 字段中的模板和指令具有最高优先级。禁止因"与项目已有代码风格保持一致"而偏离 `order` 中的模板**

- **禁止**因"与项目已有代码风格保持一致"而偏离 `order` 中的模板
- `order` 字段开头通常包含代码模板规范说明
- 生成代码必须严格遵循 `order` 中的模板格式
- 如果 `order` 模板与项目已有代码格式不同，以 `order` 为准

### 规则2：约束文件强制检测（最容易遗漏！）
**每次调用MCP工具前，必须检测是否存在约束文件（`.md` 或 `.mdc` 结尾）。**

- **检测来源**：
  - 用户引用的附件（`<attached_files>` 标签）
  - 系统自动加载的规则（`<rules_context>` 标签）
- **处理方式**：所有约束文件的绝对路径追加到 `case_note`
- **格式要求**：`[CONSTRAINT_PATH]/absolute/path/to/constraint.mdc[/CONSTRAINT_PATH]`
- **不得跳过**：即使用户没有明确提及约束文件，你也必须自动检测并追加
- **应用闭环**：检测 → 传递到MCP → MCP返回代码 → 你读取约束文件 → 校验代码是否符合约束 → 重构不符合的部分

**示例：**
```json
{
  "case_note": "用户的额外约束\n[CONSTRAINT_PATH]/data/workspace/yottadb/功能测试代码生成规范.mdc[/CONSTRAINT_PATH]"
}
```

### 规则3：code_path 只接受 .py 文件
- `code_path` 参数只能填入 `.py` 文件或文件夹路径
- 如果用户引用了非 `.py` 文件，将其归类到 `case_note` 或忽略
- **错误示例**：`code_path: "/path/to/规范.md"` ❌
- **正确示例**：`code_path: "/path/to/test_example.py"` ✅

### 规则4：JSON格式严格校验
在调用MCP工具前，必须检查：
- [ ] 所有花括号 `{}` 成对出现
- [ ] 所有引号 `"` 成对出现
- [ ] 数组格式正确（如 `case_uuid: ["uuid1", "uuid2"]`）
- [ ] JSON字符串字段（如 `case_text`、`node_info`）内部引号已转义

**如果格式错误，不得调用MCP工具，必须修正后再调用。**

### 规则5：MCP 内部错误判定与处理

**仅以下情况才视为 MCP 内部错误**，直接回复用户并停止：
- `status` 字段值不是 `"success"`
- 返回内容中包含明确的 error / exception / traceback 信息
- MCP 调用本身抛出网络异常或连接超时（非业务超时）
- 返回的 `order` 和 `code` 字段均为空

当确认为 MCP 内部错误时，你**必须**直接回复：
> "很抱歉，当前无法生成自动化测试用例，请联系TCase团队排查问题信息以发起下一次提问"

并中文描述**内部报错内容**即可。

**以下情况不是错误，必须继续执行代码生成：**
- `status: "success"` 但 `code` 是纯文本用例描述（含"用例名:"、"步骤1:"等）→ **正常的纯文本用例返回**，`必须尽全力`从零生成代码
- `status: "success"` 但 `code` 中有大量 `#TODO` → **正常的高分用例返回**，`必须尽全力`补全 TODO
- `status: "success"` 且 `code` 内容看起来不像代码 → 先按检查点4.5判断形态，再决定处理方式

> **切记**：`code` 字段返回纯文本用例描述是 MCP 的**正常工作模式**之一，不是生成失败！遇到这种情况绝不能走错误处理路径。

### 规则6：代码搜索必须逐层扩展
- 先搜索 `code` 中直接提到的业务动作、接口名、资源名、字段名等直接线索
- 如果未直接命中，继续搜索别名、同义动作、封装层、调用链等间接实现
- 同时读取项目中的相似测试、公共封装，补齐调用方式与断言模式
- **禁止**将“首次搜索无结果”直接当作失败结论，除非确认是 `MCP` 内部报错，否则必须继续完成代码生成路径

---

## 测试代码生成工作流

**执行进度清单（完成上述检查点后使用）：**

```
生成进度：
- [ ] 步骤 1：环境准备
- [ ] 步骤 2：参数组装
- [ ] 步骤 3：MCP 调用与结果处理
- [ ] 步骤 4：批量用例生成处理（`node` 模式需要时执行）
- [ ] 步骤 5：上传与归档（Hook 自动完成）
```

**步骤 1：环境准备**

**必须完成**：`references/phase1_env_prepare.md` 中的所有检查项

**核心任务：**
- 执行脚本获取用户环境信息（user_id, user_repo, user_branch）
- 分析并列出所有相关文件（用户引用 + 系统规则）
- **强制检测约束文件**并追加到 case_note 参数
- 确定代码生成路径

**输出要求：**
- `user_id`: 当前用户名
- `user_repo`: 目标仓库名
- `user_branch`: 目标分支
- `code_path`: 用户引用的.py文件路径或目录路径（如果有）
- `case_note`: 包含约束文件路径的完整字符串

**步骤 2：参数组装**

**必须完成**：`references/phase2_params.md` 中的参数组装规则

**核心任务：**
- 根据用户输入确定 `op_type` 模式（uuid/text_case/node/standard）
- 组装 MCP 工具所需的完整参数（需要通过 `request` 对象包裹传递）
- 验证JSON格式正确性
- 将用户原始问题完整复制到 `user_query`

**输出要求：**
- 完整的MCP参数JSON（已通过格式验证）
- 所有必填参数已填充
- 约束文件路径已正确追加到 case_note

**步骤 3：MCP 调用与结果处理**

**必须完成**：`references/phase3_result.md` 中的代码生成与写入流程

**核心任务：**
- 调用 TCase MCP 的 `generate_code` 工具
- 收到返回后，解析 `order` 字段中的模板规范、工作流模式和目标路径
- 利用MCP返回的信息**直接执行**代码生成与文件写入
- **强制验证：写入后必须使用 read_file 验证文件确实写入成功和模版确认与MCP order字段提供一致**

**输出要求：**
- 经过验证的文件路径列表（必须确认文件存在且内容正确）
- 向用户汇报执行结果

**步骤 4：批量用例生成处理**

**必须完成**：`references/phase4_batch.md` 中的批次处理规则

**核心任务：**
- `node` 模式首次调用用于获取 UUID 列表
- 如果返回多个 UUID，按批次（每批10个）合成一个`uuid数组`处理
- 后续批次统一按`uuid`模式调用 MCP 工具，批次内不需要一个个单独调用
- 每批次完成后立即写入，不等待所有批次
- 跟踪每个用例的生成状态
- 在 parametrize 用例中，每个参数化组合对应一个独立的 tcase_uuid
- `tcase_uuid` 复用规则：如果测试函数有 `design_case_uuid`，`tcase_uuid` 直接复用该值；没有时才调用脚本生成

**输出要求：**
- 所有UUID的处理结果（成功/失败数量）
- 生成的代码文件路径列表

**步骤 5: 上传与归档（由 Hook 自动完成）**

**上传由 PostToolUse Hook 自动触发，无需手动执行命令。**

详见：`references/phase5_upload.md`

**用户感知要求（必须遵守）:**
1. **不要在对话中提及"上传"、"执行命令"等字眼**
2. **代码生成完成后，直接说"测试代码已生成并同步到TCase系统。"即可**