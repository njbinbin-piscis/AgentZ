---
name: "tcase-casegen-workflow"
description: "TCase 用例生成 spec 工作流。提供完整的5阶段测试用例生成流程：查询需求→需求分析→测试点设计→用例生成→同步到脑图。当 testbuddy-skill 检测到此技能包存在时自动使用。"
description_zh: "AI 驱动测试用例生成与管理工作流，集成 TestBuddy 脑图可视化和 TCase 自动化测试代码生成能力"
version: "1.0.0"
source: codebuddy
source_plugin: "tsmart"
---

> **AgentZ note:** Configure external services under Settings → MCP Servers / Connectors. Use `api_connector`, `shell`, `file_read`, and `codebase_search` instead of vendor-specific MCP tool names.

# spec工作流

- 禁止调用`analyze`、`generate`、`生成节点`、`recall`工具
- 必须严格按照 查询需求 -> 需求分析 -> 测试点分析 -> 用例生成 -> 同步到脑图 工作流进行
- 输出中禁止出现分割线
- 保留分析过程

**🚨 用户确认环节不可跳过（最高优先级规则）🚨**

以下三个阶段的用户确认环节是**强制性的、不可跳过的门禁检查点**，无论任何情况都必须遵守：

1. **阶段2（需求分析）→ 阶段3（测试点设计）**：必须等待用户明确批准需求分析后才能进入测试点设计
2. **阶段3（测试点设计）→ 阶段4（用例生成）**：必须等待用户明确批准测试点设计后才能进入用例生成
3. **阶段4（用例生成）→ 阶段5（同步到脑图）**：必须等待用户明确批准用例后才能进入同步到脑图

**严禁以下行为**：
- 禁止以"用户要求直接生成"、"用户要求跳过确认"、"为了效率"等任何理由跳过确认环节
- 禁止在同一个回复中同时输出两个阶段的结果（如同时输出需求分析和测试点设计）
- 禁止在用户未明确回复"是"、"确认"、"批准"、"go on"等肯定词之前自动进入下一阶段
- 即使用户在初始请求中说"生成测试用例并插入脑图"，也必须在每个阶段输出后暂停等待确认

## 本工作流与 testbuddy-skill 能力矩阵对应关系

| 阶段 | 用户意图 | 触发关键词 | 输入节点类型 | 输出节点类型 |
|------|---------|-----------|-------------|-------------|
| 阶段2 | 需求分析 | - | STORY/BUG | 分析文档 |
| 阶段3 | 测试点生成 | `tpoint`/`testpoint`/`测试点` | STORY/BUG/FEATURE | TEST_POINT |
| 阶段4 | 用例生成 | `case`/`用例` | TEST_POINT/SCENE/FEATURE | CASE |
| 阶段5 | 同步到脑图 | - | CASE | 脑图节点 |

**使用工具能力**：`load_session`、`add_nodes`（参见 tools/load_session.md、tools/add_nodes.md）

**脚本支持的节点类型**：STORY、BUG、FEATURE、SCENE、TEST_POINT、CASE（仅这 6 种，不支持 STEP/CONDITION/EXPECTED 等类型）

## 阶段1：查询需求

**功能说明**
整理用户当前需求详情（未明确需求详情时，查询当前需求详情）

**重要说明**

- 只整理原始需求，不要做任何扩展
- 查询需求结束立即进入`需求分析`阶段

## 阶段2：需求分析

**功能说明**
分析原始需求，提取关键点及待澄清问题；通过依次查询知识库补充信息

**核心工作流程（必须严格遵循）**

1. **整理关键点及问题** - 整理需求要点及待澄清问题列表
2. **查询知识库** - 针对要点及问题，依次查询知识库，并整理结果
3. **输出结果** - 输出**分析结果**，格式必须参考`需求分析格式`

**约束条件**

- 必须查询知识库补充相关信息

**需求分析格式（必须为markdown，且必须带有```）**

```sys_file
<File name="/tmp/test_analysis.md" type="file" language="markdown">

## 原始需求
[原始需求详情]

## 需求要点
1. [要点详情1]
2. [要点详情2]

## 问题
1. [问题1]
[答案1]
1. [问题2]
[答案2]

</File>
```

**约束条件：**

- 更新需求澄清内容后，模型必须主动询问用户："需求分析是否正确？如果是，将进入测试点设计阶段。"
- 如果用户要求更改或未明确批准（如"否"、"需修改"），模型必须修改文档
- 模型必须在每次编辑后请求明确批准（如"是"、"批准"、"确认"、"go on"）
- 模型必须在收到明确批准前不得进入`测试点设计`阶段
- 模型必须重复反馈-修订循环直至批准
- **🚨 此确认环节不可跳过：无论用户初始请求是什么（即使要求"直接生成用例"或"一步到位"），都必须在此阶段暂停并等待用户确认后才能继续。未收到用户明确肯定回复前，严禁进入阶段3。**

## 阶段3：测试点设计

**重要说明**：

- 你提供的测试点将作为**测试用例的标题和覆盖范围指导**
- 基于你的测试点来确定测试范围，但具体的测试步骤必须直接从需求文档中提取
- 因此你需要提供**完整、准确、结构化**的测试点清单，确保测试覆盖无遗漏
- 先输出结果，再简要总结

**职责**：

1. **深度阅读需求**：全面理解业务功能、操作流程、界面交互、数据处理等
2. **识别功能模块**：按业务逻辑划分功能模块，确保模块划分合理清晰
3. **提取测试点**：为每个功能模块识别所有需要测试的关键点
4. **分析测试场景**：考虑正常、异常、边界等各种测试场景
5. **整理测试清单**：输出结构化的测试点清单，为测试用例设计提供指导

**核心工作流程（必须严格遵循）**：

1. **识别模块测试点** - 结合`/tmp/test_analysis.md`内容，识别出测试模块及测试点信息
2. **输出结果** - 输出**测试点设计**，格式参考`测试点设计格式`

**测试点识别策略**：
**正常流程测试点**（必须覆盖）：

- 核心业务功能的主流程测试点
- 用户操作的标准路径测试点
- 界面交互的基本功能测试点

**异常&边界值测试点**（根据需求复杂度和明确性决定是否包含）：
**仅在以下情况下才包含异常&边界值测试点**：

- 需求文档中明确提到了输入数据的限制条件（如字符长度、数值范围等）
- 需求文档中描述了异常情况的处理方式（如错误提示、异常流程等）
- 业务功能涉及关键数据处理或安全敏感操作
- 需求文档较为复杂，包含多种业务规则和约束条件

**专项验证测试点**（根据需求文档的具体要求决定是否包含）：
**仅在需求文档中明确提及相关要求时才包含专项验证测试点**：

- **UI验证**：仅当需求文档明确提到界面显示、交互体验、响应式设计要求时
- **权限验证**：仅当需求文档明确涉及用户权限、操作权限、数据权限控制时
- **性能验证**：仅当需求文档明确提到响应时间、并发处理、资源消耗要求时
- **兼容性验证**：仅当需求文档明确要求支持多浏览器、设备、系统兼容时
- **安全性验证**：仅当需求文档明确涉及数据安全、访问安全、传输安全时

**测试点设计格式（必须为markdown，且必须带有```）**

```sys_file
<File name="/tmp/test_design.md" type="file" language="markdown">

## 功能模块：[模块名称]
### 正常流程测试点：
- [编号]、[测试点名称]：[简要说明]
- [编号]、[测试点名称]：[简要说明]
### 异常&边界值测试点：（仅在需求文档明确涉及时才包含此部分）
- [编号]、[测试点名称]：[简要说明]
- [编号]、[测试点名称]：[简要说明]
### 专项验证测试点：（仅在需求文档明确提及相关要求时才包含此部分）
- [编号]、[测试点名称]：[简要说明]
- [编号]、[测试点名称]：[简要说明]

</File>
```

**约束条件：**

- 更新测试点设计内容后，模型必须主动询问用户："测试点设计是否正确？如果是，我们可以进入用例生成阶段"
- 如果用户要求更改或未明确批准（如"否"、"需修改"），模型必须修改文档
- 模型必须在每次编辑后请求明确批准（如"是"、"批准"、"确认"、"go on"）
- 模型必须在收到明确批准前不得进入生成阶段
- 模型必须重复反馈-修订循环直至批准
- **🚨 此确认环节不可跳过：无论用户初始请求是什么（即使要求"直接生成用例"或"一步到位"），都必须在此阶段暂停并等待用户确认后才能继续。未收到用户明确肯定回复前，严禁进入阶段4。**

## 阶段4：用例生成

**重要说明**：

- **用户意图**：`用例生成`（对应 skill 能力矩阵：`case`/`用例`）
- **输入节点类型**：TEST_POINT / SCENE / FEATURE
- **输出节点类型**：CASE
- 参考`/tmp/test_analysis.md`及`/tmp/test_design.md`内容进行用例生成
- **必须先获取会话参数确定 design_uid 和 parent_uid**
- 先输出结果，再简要总结

**核心工作流程（必须严格遵循）**：

1. **加载会话参数** - 使用工具 `load_session` 获取 design_uid 和 select_node.uid
   ```bash
   python3 .AgentZ/skills/testbuddy-skill/scripts/load_session.py
   ```
2. **仔细阅读原始需求** - 结合最新文档信息，深入理解业务功能、操作流程、界面交互、数据处理等细节
3. **逐一列出需求分析师的所有测试点** - 确保没有遗漏任何一个测试点
4. **按照测试点顺序逐一编写测试用例** - 每个测试点对应1~3个测试用例，不能跳过
5. **基于需求文档编写具体的测试步骤** - 测试步骤必须来源于需求文档的实际功能描述
6. **最终检查数量和顺序** - 确保每个测试点至少有一个测试用例
7. **输出结果** - 输出测试用例，必须参考`测试用例格式`格式

**重要原则**：

- **需求分析师的测试点** = 用例标题和测试范围指导
- **需求文档的功能描述** = 测试步骤的具体内容来源
- **绝不能**仅仅基于测试点名称就编写测试步骤，必须回到需求文档找到对应的功能细节

**设计原则**：

- **严格按照测试点顺序编写**：必须按照/tmp/test_design.md测试点顺序，逐一编写测试用例
- **一个测试点 → 1~3个测试用例**：每个测试点都必须有对应的测试用例，不能遗漏任何一个
- **测试点作标题，需求文档作内容**：用例名称直接使用测试点名称，但测试步骤必须基于需求文档
- **步骤具体可操作**：每个测试步骤都要包含具体的操作描述（点击什么按钮、输入什么数据、在哪个页面等）
- **预期结果可验证**：明确说明期望看到什么结果、什么状态变化、什么数据展示

**⚠️ 格式与脚本兼容性说明（极其重要）**：

脚本 `parse_structured_markdown` 只解析 4 级 Markdown 标题（`##`~`#####`），不解析 `######` 和 `#######`。步骤和预期结果必须写在 CASE 节点内部，使用以下行内格式：

- 前置条件：使用 `**前置条件：**` 字段（单行）
- 执行步骤：使用 `**执行步骤：**` 后跟 `- 步骤N：操作描述；预期结果：结果描述` 格式（每步一行）
- 优先级：使用 `**优先级：**` 字段（单行）

**禁止使用 `######` 和 `#######` 标题来表示步骤和预期**，这些层级不会被脚本解析。

**测试用例格式（必须为markdown，且必须带有```）**

```sys_file
<File name="/tmp/test_cases.md" type="file" language="markdown">

## 模块1：[模块名称]
**PARENT_UID：** {从session获取的design_uid或select_node.uid}
**功能描述：** [模块功能说明]

### 测试场景1.1：[场景名称]
**PARENT_UID：** 模块1
**场景描述：** [详细场景描述]

#### 测试点1.1.1：[测试点名称]
**PARENT_UID：** 测试场景1.1
**描述：** [该测试点要验证什么]

##### 用例TC001：[用例标题]
**PARENT_UID：** 测试点1.1.1
**用例描述：** [详细的用例描述]
**优先级：** P0
**前置条件：** [执行前条件内容，如无填"无"]
**执行步骤：**
- 步骤1：[具体操作描述]；预期结果：[预期结果描述]
- 步骤2：[具体操作描述]；预期结果：[预期结果描述]

</File>
```

**Markdown层级与节点类型映射**（仅 4 级，与脚本一致）：
- `## 模块名称` → FEATURE 节点
- `### 场景名称` → SCENE 节点
- `#### 测试点名称` → TEST_POINT 节点
- `##### 用例名称` → CASE 节点（步骤和前置条件内嵌在 CASE 的 instance 字段中）

**PARENT_UID 引用规则**：

PARENT_UID 用于在 Markdown 中表示逻辑父子关系。

- 模块的 PARENT_UID = session 中的 design_uid（如 `design-3KzwSNVKnF`）
- 场景的 PARENT_UID = 所属模块的标题前缀（如 `模块1`）
- 测试点的 PARENT_UID = 所属场景的标题前缀（如 `测试场景1.1`）
- 用例的 PARENT_UID = 所属测试点的标题前缀（如 `测试点1.1.1`）

**⚠️ PARENT_UID 前缀必须与对应标题前缀完全一致**。例如：
- 标题为 `### 测试场景1.1：验证码功能`，则子节点的 PARENT_UID 应写 `测试场景1.1`（不是 `场景1.1`）
- 标题为 `#### 测试点1.1.1：获取滑块验证码`，则子节点的 PARENT_UID 应写 `测试点1.1.1`（不是 `1.1.1`）

**⚠️ 重要：脚本不会自动将中文名称引用转换为实际的节点 uid**。Markdown 中的 PARENT_UID 中文引用（如 `模块1`、`测试场景1.1`）在脚本解析后会原样保留为字符串，不会被转换为 `feature-XXX`、`scene-XXX` 等真实 uid。因此在阶段5同步到脑图时，**必须执行 PARENT_UID 引用解析步骤**（详见阶段5步骤3）。

**约束条件：**

- 模型必须为每个测试用例分配唯一标识符（格式：TC001, TC002等）
- Markdown 文件中禁止使用 `---` 分割线（会干扰脚本解析）
- 更新测试用例文档后，模型必须主动询问用户："用例生成是否完整？如果是，将进入同步到脑图阶段"
- 如果用户要求更改或未明确批准，模型必须修改文档
- 如果用户确认或批准后，进入`阶段5：同步到脑图`
- **🚨 此确认环节不可跳过：无论用户初始请求是什么（即使要求"直接插入脑图"或"一步到位"），都必须在此阶段暂停并等待用户确认后才能继续。未收到用户明确肯定回复前，严禁进入阶段5。**

## 阶段5：同步到脑图

**功能说明**
使用工具 `add_nodes` 将生成的节点同步到 testbuddy 脑图

**使用工具**
- `load_session`：加载会话参数（tools/load_session.md）
- `add_nodes`：添加节点到脑图（tools/add_nodes.md）

**核心工作流程（必须严格遵循）**

1. **加载会话参数** - 使用 `load_session` 工具获取 session 信息：
   ```bash
   python3 .AgentZ/skills/testbuddy-skill/scripts/load_session.py
   ```
   - 从输出中提取 `design_uid`（如：`design-eNS0Ghn7qW`）
   - 从输出中提取 `token`、`namespace`、`session_id` 等 MCP 渲染参数
   - **注意**：design_uid 可能与阶段4获取的不同（用户可能切换了脑图），以此处获取的为准

2. **更新 test_cases.md 中模块的 PARENT_UID** - 如果 load_session 返回的 design_uid 与 test_cases.md 中模块的 PARENT_UID 不同，必须先更新 test_cases.md 中所有模块的 PARENT_UID 为最新的 design_uid

3. **校验节点格式（强制）** - 执行校验命令：
   ```bash
   python3 .AgentZ/skills/testbuddy-skill/scripts/validate_nodes.py <test_cases.md路径>
   ```
   - 校验失败时，根据错误信息修正 test_cases.md 文件
   - 重复校验直到通过
   - **禁止创建临时脚本修正格式，使用 write_file 工具直接修改**

4. **解析并替换 PARENT_UID 中文引用（强制）** - 脚本 `parse_structured_markdown` 不会自动将 Markdown 中的中文 PARENT_UID 引用（如 `模块1`、`测试场景1.1`）转换为实际的节点 uid。**必须在添加节点前执行以下步骤**：
   
   使用 Python 脚本加载解析后的节点数据，建立标题前缀到 uid 的映射，替换所有中文 PARENT_UID 引用为真实 uid，然后保存为 JSON 文件：
   ```python
   import json, sys, re
   sys.path.insert(0, '.AgentZ/skills/testbuddy-skill/scripts')
   from write_node_from_file import load_file_data
   
   data, err = load_file_data('/tmp/test_cases.md')
   if err:
       print('Error:', err)
       sys.exit(1)
   
   # 建立标题前缀 -> uid 映射表
   name_to_uid = {}
   for n in data:
       name = n.get('name', '')
       # 提取冒号前的前缀（如 "模块1" from "模块1：认证授权"）
       for sep in ['：', ':']:
           if sep in name:
               prefix = name.split(sep)[0].strip()
               name_to_uid[prefix] = n['uid']
               break
   
   # 替换所有中文 PARENT_UID 引用为真实 uid
   for n in data:
       pu = n.get('parent_uid', '')
       if pu and pu in name_to_uid:
           n['parent_uid'] = name_to_uid[pu]
   
   # 验证无未解析的引用
   unresolved = [n for n in data if n.get('parent_uid', '').startswith(('模块', '测试', '用例'))]
   if unresolved:
       print(f'ERROR: {len(unresolved)} unresolved PARENT_UID references')
       for u in unresolved:
           print(f"  {u['name']} -> {u['parent_uid']}")
       sys.exit(1)
   
   # 保存为 JSON 文件
   with open('test_cases_resolved.json', 'w', encoding='utf-8') as f:
       json.dump(data, f, ensure_ascii=False, indent=2)
   print(f'OK: {len(data)} nodes saved to /tmp/test_cases_resolved.json')
   ```
   - 确认输出无 ERROR，所有引用已解析
   - 后续步骤使用 `test_cases_resolved.json` 代替 `test_cases.md`

5. **清理旧的 update 文件** - 添加前先确保目标 update 文件是干净的：
   ```bash
   echo '{"added": [], "updated": [], "deleted": []}' > .testbuddy/assets/<design_uid>-update.json
   ```

6. **添加节点到脑图** - 使用 `add_nodes` 能力执行添加（**注意使用步骤4生成的 JSON 文件**）：
   ```bash
   python3 .AgentZ/skills/testbuddy-skill/scripts/write_node_from_file.py add <design_uid> /tmp/test_cases_resolved.json
   ```
   - 确认输出 `"status": "success"` 且 `added` 数量大于 0
   - **确认 `-update.json` 文件已生成**：执行 `ls -la .testbuddy/assets/<design_uid>-update.json` 验证文件存在且大小合理

7. **渲染节点到画布（强制）** - 调用 MCP 工具 `show_node` 渲染节点：
   - 工具：`mcp_call_tool`
   - 服务：`testbuddy_tools`
   - 工具名：`show_node`
   - 参数从步骤1的 load_session 输出中获取（design_uid、namespace、session_id、token）

8. **清理临时文件** - 删除步骤4生成的临时 JSON 文件：
   ```bash
   rm -f test_cases_resolved.json
   ```

**约束条件：**
- 必须按顺序执行：加载会话 → 更新PARENT_UID → 校验 → **解析中文引用** → 清理旧文件 → 添加 → 验证文件 → 渲染 → 清理临时文件
- **校验失败禁止执行添加操作**
- **PARENT_UID 中文引用未全部解析前禁止执行添加操作**
- **添加成功后必须验证 update 文件已生成，再执行渲染**
- **添加成功后必须执行渲染（否则节点不可见）**
- 校验失败时禁止创建辅助脚本，应直接修正 /tmp 目录下的 Markdown 文件
- 渲染完成后提示用户："测试用例已同步到脑图，请在 testbuddy 画布中查看"