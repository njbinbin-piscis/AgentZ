# add_nodes

通过 Python 脚本稳定地添加节点到脑图。

**⚠️ 重要**：执行脚本前**不要切换目录（不要 cd）**，确保在工作区根目录执行。
**⚠️ 重要**：已经生成用例的文件的话，直接复用已生成的文件目录即可。

---

## 支持的文件格式

脚本支持以下三种格式：

### 1. JSON 格式 (.json)
标准的 JSON 数组格式。

### 2. YAML 格式 (.yaml/.yml)
标准的 YAML 数组格式。

### 3. Markdown 格式 (.md)
直接使用 Markdown 标题层级表示节点结构：
- `## 模块名称` → FEATURE 节点
- `### 场景名称` → SCENE 节点
- `#### 测试点名称` → TEST_POINT 节点
- `##### 用例名称` → CASE 节点

**说明**：Markdown 文件可以直接使用，脚本会自动解析结构中的 `PARENT_UID`、`UID`、`优先级`、`执行步骤` 等字段。

Markdown 中也支持使用 ```json 或 ```yaml 代码块包裹节点数据。

---

## 完整操作流程

### 步骤1: 校验节点格式（必须）

在执行添加命令前，**必须先校验**临时文件的节点格式是否符合预期。

**校验命令**：
```bash
python3 <script_dir>/scripts/validate_nodes.py <file_path>
```

**校验成功输出**：
```json
{
  "status": "success",
  "action": "validate",
  "file_path": "/tmp/nodes_batch.json",
  "stats": {
    "total_nodes": 3,
    "valid_nodes": 3
  },
  "msg": "所有节点校验通过"
}
```

**校验失败输出**：
```json
{
  "status": "error",
  "action": "validate",
  "file_path": "/tmp/nodes_batch.json",
  "msg": "节点校验失败: 节点[0]: 缺少必填字段 'uid'"
}
```

**如果校验失败**：
- ❌ **不要执行添加命令**
- ✅ 根据错误信息修正文件内容
- ✅ 重新校验直到通过
- ❌ **禁止生成临时脚本来修改格式(脚本生成的格式不可用)，请使用write_file工具直接修改**

### 步骤2: 执行添加命令

**前置条件**：步骤1校验通过

**命令格式**：
```bash
python3 <script_dir>/scripts/write_node_from_file.py add <design_uid> <file_path>
```

**示例**：
```bash
python3 <script_dir>/scripts/write_node_from_file.py add design-Az7SsiL3Ui /tmp/nodes_batch.json
```

**优点**：支持批量添加、多种格式（JSON/YAML/Markdown）、无命令行长度限制

**参数说明**：
- `<script_dir>`: 脚本目录的绝对路径（通常为 `.codebuddy/skills/testbuddy-skill`）
- `validate`: 操作类型（校验格式，使用 validate_nodes.py 脚本）
- `add`: 操作类型（添加节点，使用 write_node_from_file.py 脚本）
- `<design_uid>`: 设计文件的唯一标识（如：`design-Az7SsiL3Ui`）
- `<file_path>`: 临时文件的完整路径 

### 步骤3：渲染节点到画布（强制,不执行就会有个人跳楼自杀）

节点添加成功后，**必须调用 MCP 工具**将节点渲染到画布。

#### 3.1 获取渲染参数

执行脚本获取 session 信息：

```bash
python3 <script_dir>/scripts/load_session.py
```

从输出中提取mcp 工具的相关参数

#### 3.2 调用 MCP 渲染工具
调用mcp 工具 show_node渲染画布(强制执行)


## 命令执行输出

### 成功输出示例
```json
{
  "status": "success",
  "action": "add",
  "design_uid": "design-Az7SsiL3Ui",
  "file_path": "/tmp/nodes_batch.json",
  "stats": {
    "added": 2,
    "total_added": 2,
    "total_updated": 0,
    "total_deleted": 0
  }
}
```

### 失败输出示例
```json
{
  "status": "error",
  "msg": "节点校验失败: 节点[0]: 缺少必填字段 'uid'"
}
```

---

## 使用示例

```bash
# 1. 校验节点格式
python3 <script_dir>/scripts/validate_nodes.py /path/to/nodes.md

# 2. 添加节点到脑图
python3 <script_dir>/scripts/write_node_from_file.py add <design_uid> /path/to/nodes.md

# 3. 获取会话参数并渲染画布
python3 <script_dir>/scripts/load_session.py
# 调用 MCP 工具 show_node 渲染
```

---

## 注意事项

0. **节点格式校验（强制）**：
   - ⚠️ **执行添加前必须先校验**：`python3 <script_dir>/scripts/validate_nodes.py <file_path>`
   - ✅ 校验通过后才能执行添加操作
   - ❌ 校验失败时，根据错误信息修正文件后重新校验(修正文件禁止新建脚本，使用`write_file`工具即可)
   - 📋 校验支持 JSON、YAML、Markdown 三种格式

1. **使用文件方式**：
   - ✅ 必须使用 `python3 <script_dir>/scripts/write_node_from_file.py add <design_uid> <file_path>`
   - ✅ 支持 JSON、YAML、Markdown 多种格式
   - ✅ 无命令行长度限制，适合批量操作

2. **MCP 渲染（必须）**：
   - 节点操作成功后，**必须调用 MCP 工具** `show_node` 渲染节点
   - 参数从 `python3 <script_dir>/scripts/load_session.py` 获取
   - 工具：`mcp_call_tool`，服务：`testbuddy_tools`，工具名：`show_node`

3. **设计标识**：使用完整的 `design_uid`（如：`design-Az7SsiL3Ui`，不是缩写）

4. **错误处理**：脚本会返回详细的错误信息，指出具体哪个字段有问题

5. **🚫 禁止创建新脚本**：
   - 如果执行脚本时遇到错误，根据错误信息修正文件后重新校验(修正文件禁止新建脚本，使用`write_file`工具即可)
   - **严禁创建新的辅助脚本、wrapper脚本或临时脚本来处理错误**
   - 应通过以下方式解决：
     - 检查并修正节点数据格式
     - 调整命令参数
     - 修复文件路径或 design_uid
     - 补充缺失的必填字段
   - 始终使用现有的标准脚本 `validate_nodes.py` 和 `write_node_from_file.py`
---