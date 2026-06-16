# RAG Search Tool

## 工具说明

提供两种知识检索策略，**同时执行**以获取更全面的检索结果。两种策略互相补充，从不同来源检索相关知识。

**⚠️ 重要**：执行脚本前**不要切换目录（不要 cd）**，确保在工作区根目录执行。

---

## 检索策略（同时执行）

### 策略 1: MCP 知识检索

通过 MCP 工具 `retrieve_knowledge` 检索当前会话相关的知识内容。

**MCP 工具参数说明：**
- `token`: 用户 token，从 `.testbuddy/env/session.json` 获取
- `query`: 检索查询字符串，描述要查找的内容（**每次只能传一个查询关键词**）
- `knowledge_uids`: 知识库 UID 列表，指定要检索的知识库
- `top_k`: 返回的 top_k 个结果（可选）
- `score_threshold`: 检索结果的最小分数阈值（可选）

**⚠️ 重要**：`query` 参数每次只能传入一个查询关键词。如果需要使用多个关键词进行检索（如"用户登录"、"密码验证"、"权限管理"），需要**分批调用** MCP 工具，每个关键词调用一次。

**执行步骤：**

1. **获取会话信息**
   
   执行脚本获取 session.json 内容：
   
   ```bash
   python <script_dir>/scripts/load_session.py
   ```
   
   脚本会读取 `.testbuddy/env/session.json` 文件，从中提取：
   - `token`: 用户认证 token
   - `knowledge_uids`: 知识库 UID 列表
   - 其他会话相关参数

2. **调用 MCP 工具检索知识**
   
   使用提取的参数调用 MCP 工具：
   - 首先使用 `mcp_get_tool_description` 获取 `retrieve_knowledge` 工具描述
   - 然后使用 `mcp_call_tool` 调用 `retrieve_knowledge` 工具
   - 参数构造示例：
     ```json
     {
       "serverName": "testbuddy_tools",
       "toolName": "retrieve_knowledge",
       "arguments": "{\"token\": \"从session.json提取\", \"query\": \"用户搜索关键词\", \"knowledge_uids\": [\"从session.json提取的UID列表\"], \"top_k\": 10, \"score_threshold\": 0.5}"
     }
     ```

3. **返回结果处理**
   - 返回检索到的会话相关知识内容
   - 支持语义检索，根据查询问题找到相关的业务文档或历史测试用例
   - 包含文档内容、元数据、相关度评分等

### 策略 2: RAG_search 工具检索附加文件

**触发条件：** 当上下文中包含 `<attached_files>` 区域时执行此策略

**执行步骤：**

1. **调用系统内置的 RAG_search 工具**
   - 使用 `RAG_search` 函数
   - **作用范围**：只检索 `<attached_files>` 区域中的文件内容
   - 参数：
     - `queryString`: 搜索关键词
     - `explanation`: 搜索说明（可选）

2. **返回结果处理**
   - 返回检索到的附加文件中的相关内容
   - 包含：
     - **chunk**: 检索到的文档内容
     - **metadata**: 文档元数据（来源、类型、文件名等）
     - **score**: 相关度评分

---

## 参数说明

### 用户输入参数
- `queryString`: 搜索关键词或查询语句，例如：'登录用例'、'支付功能测试'、'性能测试场景'等

### MCP 工具参数（从 session.json 自动提取）
- `token`: 用户 token（必填）
- `query`: 检索查询字符串（必填，**每次只能传一个关键词**）
- `knowledge_uids`: 知识库 UID 列表（必填）
- `top_k`: 返回的 top_k 个结果（可选，默认可设为 10）
- `score_threshold`: 检索结果的最小分数阈值（可选，默认可设为 0.5）

**⚠️ 注意**：如需使用多个关键词检索（如"登录功能"、"密码验证"、"权限管理"），需要分批调用 MCP 工具，每个关键词调用一次。

### 脚本路径
- `<script_dir>`: 脚本目录的绝对路径（通常为 `.codebuddy/skills/testbuddy-skill`）

### 数据文件
- `.testbuddy/env/session.json`: 存储 token 和 knowledge_uids 等会话信息

**注意**：两种策略使用相同的查询关键词逻辑

---

## 执行逻辑

**两种策略同时执行，同等优先级：**

1. **始终执行策略 1**：通过 `load_session.py` 从 `.testbuddy/env/session.json` 获取 `token` 和 `knowledge_uids`，调用 MCP 工具检索会话相关知识
   - **重要**：如需使用多个关键词，需要**分批调用** MCP 工具，每个关键词调用一次
2. **同时检查策略 2**：如果上下文包含 `<attached_files>` 区域，同时执行 `RAG_search` 检索附加文件内容
3. **结果合并**：将两种策略的检索结果合并展示，提供更全面的知识覆盖

```
开始检索
    ↓
并行执行两个策略：
    ├─ 策略 1: 执行 load_session.py
    │           ↓
    │       读取 .testbuddy/env/session.json
    │           ↓
    │       提取 token 和 knowledge_uids
    │           ↓
    │       调用 MCP retrieve_knowledge
    │       (参数: token, query, knowledge_uids, top_k, score_threshold)
    │           ↓
    │       返回会话相关知识 (结果 A)
    │
    └─ 策略 2: 检查上下文是否包含 <attached_files>
                ↓
            包含？
                ├─ 是 → 调用 RAG_search 检索附加文件
                │         ↓
                │      返回附加文件中的相关内容 (结果 B)
                └─ 否 → 跳过此策略
    ↓
合并结果 A + B，返回综合检索结果
```

---

## 使用示例

### 完整检索流程示例（两个策略同时执行）

**用户查询：** "登录相关用例"

**并行执行：**

1. **策略 1 执行（如需多个关键词，分批调用）：**

```bash
# 步骤 1: 获取会话信息
python <script_dir>/scripts/load_session.py

# 输出示例（从 .testbuddy/env/session.json 读取）：
# {
#   "token": "user_token_string",
#   "knowledge_uids": ["knowledge_uid_1", "knowledge_uid_2"],
#   "design_uid": "design-Az7SsiL3Ui",
#   ...
# }

# 步骤 2: 分批调用 MCP 工具检索（每个关键词调用一次）
# 第一次调用 - 关键词1: "登录相关用例"
mcp_call_tool(
  serverName="testbuddy_tools",
  toolName="retrieve_knowledge",
  arguments='{
    "token": "user_token_string",
    "query": "登录相关用例",
    "knowledge_uids": ["knowledge_uid_1", "knowledge_uid_2"],
    "top_k": 10,
    "score_threshold": 0.5
  }'
)

# 第二次调用 - 关键词2: "密码验证"（如需要）
mcp_call_tool(
  serverName="testbuddy_tools",
  toolName="retrieve_knowledge",
  arguments='{
    "token": "user_token_string",
    "query": "密码验证",
    "knowledge_uids": ["knowledge_uid_1", "knowledge_uid_2"],
    "top_k": 10,
    "score_threshold": 0.5
  }'
)
# 返回: 会话上下文中的相关知识（业务文档或历史测试用例）
```

2. **策略 2 执行（如果有 attached_files）：**

```python
RAG_search(
  queryString="登录相关用例",
  explanation="检索附加文件中的登录相关内容"
)
# 返回: 附加文件中的相关内容
```

3. **结果合并展示：**
   - 来自会话上下文的知识（策略 1，可能包含多次调用的结果）
   - 来自附加文件的内容（策略 2）
   - 按相关度综合排序展示

---

## 工具特性

1. **并行双重检索**：两种策略同时执行，互相补充
2. **智能检索**：支持关键词、短语、自然语言查询
3. **会话上下文**：自动获取当前会话的相关参数（策略 1）
4. **附加文件检索**：可检索 `<attached_files>` 中的文件内容（策略 2）
5. **相关度排序**：返回结果按相关度评分排序
6. **元数据丰富**：包含文档来源、类型等元信息
7. **结果聚合**：合并两种来源的检索结果
8. **分批查询支持**：MCP 工具支持多关键词分批调用

---

## 使用建议

1. **关键词选择**：使用具体、相关的关键词以获得更精确的结果
2. **分批查询策略**：
   - 如需使用多个关键词（如"用户登录"、"密码验证"、"权限管理"），对 MCP 工具进行分批调用
   - 每个关键词单独调用一次 `retrieve_knowledge`
   - 将多次调用的结果合并处理
3. **脚本执行**：确保在工作区根目录执行 `load_session.py` 脚本，**不要 cd 切换目录**
4. **上下文准备**：如需使用策略 2，确保上下文包含 `<attached_files>` 区域
5. **结果处理**：综合处理两种策略的检索结果，提供完整的知识视图
6. **并行执行**：两个策略同时进行，提高检索效率

---

## 适用场景

该工具适用于以下场景：
- 查找特定功能的测试用例（从会话和知识库）
- 检索测试策略和方法论
- 获取测试场景示例
- 查询最佳实践和规范
- 搜索历史测试文档
- 基于会话上下文的知识检索
- 跨来源的综合知识查询

---

## 注意事项

1. **脚本执行路径**：
   - ⚠️ 执行 `load_session.py` 前**不要切换目录（不要 cd）**
   - ✅ 确保在工作区根目录执行
   - `<script_dir>` 是脚本所在路径的前缀（不要写死路径）

2. **session.json 文件**：
   - 位置：`.testbuddy/env/session.json`
   - 必须包含：`token`（用户认证）、`knowledge_uids`（知识库 UID 列表）
   - `load_session.py` 脚本会自动读取此文件

3. **策略 1** 依赖 session.json 文件和 MCP 工具
   - 参数说明：
     - `token`: 必填，用户认证 token
     - `query`: 必填，检索查询字符串（**每次只能传一个关键词**）
     - `knowledge_uids`: 必填，知识库 UID 列表
     - `top_k`: 可选，返回结果数量（建议设为 10）
     - `score_threshold`: 可选，最小分数阈值（建议设为 0.5）
   - **多关键词处理**：如需使用多个关键词，必须分批调用 MCP 工具，每个关键词调用一次
   - 如果 session.json 不存在或参数缺失，仅返回策略 2 的结果

4. **策略 2** 依赖上下文中是否存在 `<attached_files>` 区域
   - **作用范围**：只检索 `<attached_files>` 中的文件内容
   - **不需要** `knowledgeBaseNames` 参数
   - 如果不存在 `<attached_files>`，仅返回策略 1 的结果

5. **两种策略相互独立**：
   - 任何一个成功都会返回有效结果
   - 建议同时满足两种策略的条件，以获得最全面的检索结果

6. **MCP 工具调用**：
   - 使用 `mcp_get_tool_description` 获取工具描述
   - 使用 `mcp_call_tool` 执行实际检索
   - 参数从 `load_session.py` 输出中动态提取（不要硬编码）

---
