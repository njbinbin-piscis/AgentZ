---
name: "testbuddy-skill"
description: "文本测试用例生成技能包。任何有关于文本测试用例生成、文本测试用例框架生成、脑图用例生成、召回、需求分析等文本测试用例生成的相关问题，都可以使用此技能。"
description_zh: "AI 驱动测试用例生成与管理工作流，集成 TestBuddy 脑图可视化和 TCase 自动化测试代码生成能力"
version: "1.0.0"
source: codebuddy
source_plugin: "tsmart"
---

> **AgentZ note:** Configure external services under Settings → MCP Servers / Connectors. Use `api_connector`, `shell`, `file_read`, and `codebase_search` instead of vendor-specific MCP tool names.

# Design Agent Skill 测试设计技能包

## 核心定位

本 skill 是一个**轻量级的意图识别和工作流调度器**，负责：
1. 识别用户意图
2. 加载必要的会话参数
3. 调用对应的工作流

**工作流负责**：s
- 自己的执行清单规划
- 具体的执行逻辑
- 任务状态管理

## 执行流程

### 步骤 1：识别用户意图

根据用户请求的关键词，识别用户想要执行的操作：

| 用户意图 | 触发关键词 | 意图关键词 |
|---------|-----------|-----------|
| 生成测试框架 | `生成测试框架`、`生成框架`、`创建测试框架` | `框架生成` |
| 生成测试模块 | `生成测试模块`、`生成功能模块`、`识别测试模块` | `模块生成` |
| 生成测试场景 | `生成测试场景`、`生成场景`、`创建测试场景` | `场景生成` |
| 生成测试点 | `生成测试点`、`创建测试点`、`根据模块生成测试点` | `测试点生成` |
| 生成测试用例 | `生成测试用例`、`生成用例`、`根据测试点生成用例` | `用例生成` |

### 步骤 2：加载会话参数

使用 `tools/load_session.md` 工具读取上下文信息。

### 步骤 3：选择并调用工作流

1. **优先检测 tcase-casegen-workflow 技能包**：
   - 检查文件 `.AgentZ/skills/tcase-casegen-workflow/SKILL.md` 是否存在
   - 如果存在，直接使用该技能包中的 spec 工作流（无需调用 select_workflow）
   - 如果不存在，继续执行原有的 select_workflow 流程

2. **使用 select_workflow 工具**（仅当 tcase-casegen-workflow 不存在时）：
   - 调用 `tools/select_workflow.md` 工具
   - 传入步骤 1 识别的意图关键词（如"用例生成"、"框架生成"）

3. **调用工作流**：
   - 根据检测结果或 `select_workflow` 返回的路径类型选择读取方式：
     - 如果检测到 tcase-casegen-workflow 技能包：使用 `use_skill` 工具调用该技能包
     - 如果路径以 `.AgentZ/rules/` 开头（自定义工作流）：使用 `read_rules` 读取
     - 如果路径以 `workflows/` 开头（默认工作流）：使用 `read_file` 读取
   - 交由工作流执行具体逻辑
   - 禁止不执行检测或select_workflow流程直接读取文件/工作流


## 工作流能力矩阵（供参考）

| 生成目标 | 触发关键词 | 输入节点类型 | 输出节点类型 |
|---------|-----------|-------------|-------------|
| 测试框架 | `framework`/`框架` | STORY/BUG | FEATURE/SCENE/TEST_POINT |
| 测试模块 | `feature`/`模块` | STORY/BUG | FEATURE |
| 测试场景 | `scene`/`场景` | FEATURE/STORY/BUG | SCENE |
| 测试点 | `tpoint`/`testpoint`/`测试点` | FEATURE/SCENE/STORY/BUG | TEST_POINT |
| 测试用例 | `case`/`用例` | STORY/BUG/FEATURE/TEST_POINT/SCENE | CASE |

## 工具能力矩阵（供参考）

| 工具 | 功能 | 文档路径 |
|-----|------|----------|
| select_workflow | 智能选择工作流 | `tools/select_workflow.md` |
| select_rule | 智能选择规则 | `tools/select_rule.md` |
| load_session | 加载会话参数 | `tools/load_session.md` |
| add_nodes | 添加节点到脑图 | `tools/add_nodes.md` |
| update_nodes | 修改脑图节点 | `tools/update_nodes.md` |
| delete_nodes | 删除脑图节点 | `tools/delete_nodes.md` |
| search_nodes | 查询脑图节点 | `tools/search_nodes.md` |
| rag_search | 知识库检索 | `tools/rag_search.md` |

---