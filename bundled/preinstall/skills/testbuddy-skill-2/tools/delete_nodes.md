# delete_nodes

操作脑图时记录删除节点到update文件（无需自行创建Python脚本）。

**⚠️ 重要**：执行脚本前**不要切换目录（不要 cd）**，确保在工作区根目录执行。

---

## 完整操作流程

### 步骤1：读取脑图文件

读取脑图文件，定位要删除的节点：

```
脑图文件路径: .testbuddy/assets/{design_uid}.json
```

使用 `read_file` 工具读取完整的JSON文件。

### 步骤2：定位目标节点

通过UID在JSON中定位要删除的节点。递归遍历节点树，找到匹配的节点。

#### 节点类型枚举
- `STORY` - 需求
- `BUG` - 缺陷
- `FEATURE` - 功能模块
- `SCENE` - 测试场景
- `TEST_POINT` - 测试点
- `CASE` - 测试用例

### 步骤3：确认删除范围

**重要**：删除节点会同时删除其所有子节点（递归删除）。

在执行删除前，应该：
1. 确认要删除的节点
2. 列出该节点的所有子孙节点（如有）
3. 询问用户确认（如果子节点较多）

### 步骤4：执行删除命令

**命令格式**：
```bash
echo '["uid1", "uid2"]' | python3 <script_dir>/scripts/write_nodes.py delete <design_uid>
```

**示例**：
```bash
echo '["test_point-fvcfvuh9g2", "case-cf89EaXfQ2"]' | python3 <script_dir>/scripts/write_nodes.py delete design-Az7SsiL3Ui
```

**参数说明**：
- `<script_dir>`: 脚本目录的绝对路径（通常为 `.codebuddy/skills/testbuddy-skill`）
- `delete`: 操作类型（删除节点）
- `<design_uid>`: 设计文件的唯一标识（如：`design-Az7SsiL3Ui`）

**输入格式**：UID数组 `["uid1", "uid2"]`

**重要**：删除父节点时，需要将父节点及其所有子孙节点的UID都添加到数组中。

### 步骤5：渲染节点到画布（必须）

节点删除成功后，**必须调用 MCP 工具**将删除后的节点状态渲染到画布。

#### 5.1 获取渲染参数

执行脚本获取 session 信息：

```bash
python3 <script_dir>/scripts/load_session.py
```

**输出示例**：
```json
{
  "design_uid": "design-yeAkhG7cH5",
  "workspace_id": "100028791",
  "testbuddy_url": "https://testbuddy.woa.com"
}
```

从输出中提取 `design_uid` 和 `workspace_id`。

#### 5.2 调用 MCP 渲染工具

使用 `mcp_call_tool` 渲染节点：

```
工具名称: show_node
服务名称: testbuddy_tools
参数:
{
  "design_uid": "从 session.json 获取",
  "workspace_id": "从 session.json 获取"
}
```

---

## 命令执行输出

### 成功输出示例
```json
{
  "status": "success",
  "action": "delete",
  "design_uid": "design-Az7SsiL3Ui",
  "target_file": ".testbuddy/assets/design-Az7SsiL3Ui-update.json",
  "stats": {
    "deleted": 1,
    "total_added": 0,
    "total_updated": 0,
    "total_deleted": 1
  }
}
```

### 失败输出示例
```json
{
  "status": "error",
  "msg": "UID格式错误: 必须是字符串数组"
}
```

---

## 完整操作示例

### 示例1：删除单个测试点（无子节点）

**用户请求**：删除测试点 test_point-fvcfvuh9g2

**操作流程**：

```bash
# 步骤4: 执行删除命令
echo '["test_point-fvcfvuh9g2"]' | python3 <script_dir>/scripts/write_nodes.py delete design-Az7SsiL3Ui

# 步骤5.1: 获取渲染参数
python3 <script_dir>/scripts/load_session.py

# 步骤5.2: 调用 MCP 工具渲染
# 工具名: show_node, 服务名: testbuddy_tools
# 参数: {"design_uid": "...", "workspace_id": "..."}
```

---

### 示例2：删除有子节点的测试点

**用户请求**：删除测试点 test_point-WmaL4qejAu（包含子用例 case-cf89EaXfQ2）

**操作流程**：

```bash
# 步骤4: 执行删除命令（包含所有子孙节点UID）
echo '["test_point-WmaL4qejAu", "case-cf89EaXfQ2"]' | python3 <script_dir>/scripts/write_nodes.py delete design-Az7SsiL3Ui

# 步骤5.1: 获取渲染参数
python3 <script_dir>/scripts/load_session.py

# 步骤5.2: 调用 MCP 工具渲染
```

**重要**：删除父节点时，必须将父节点及其所有子孙节点的UID都添加到数组中。

---

### 示例3：批量删除多个同级节点

**用户请求**：删除 custom-cmjAXojn7Q 下的所有TEST_POINT类型节点

**操作流程**：

```bash
# 步骤4: 执行删除命令
echo '["test_point-WmaL4qejAu", "case-cf89EaXfQ2", "test_point-fvcfvuh9g2"]' | python3 <script_dir>/scripts/write_nodes.py delete design-Az7SsiL3Ui

# 步骤5.1: 获取渲染参数
python3 <script_dir>/scripts/load_session.py

# 步骤5.2: 调用 MCP 工具渲染
```

---

### 示例4：删除单个测试用例

**用户请求**：删除测试用例 case-TLe94mwFoQ

**操作流程**：

```bash
# 步骤4: 执行删除命令
echo '["case-TLe94mwFoQ"]' | python3 <script_dir>/scripts/write_nodes.py delete design-Az7SsiL3Ui

# 步骤5.1: 获取渲染参数
python3 <script_dir>/scripts/load_session.py

# 步骤5.2: 调用 MCP 工具渲染
```

---

## 常见删除场景

### 1. 清理测试数据
删除测试用的临时节点

### 2. 移除重复节点
删除重复或错误创建的节点

### 3. 重构测试结构
删除旧的测试点，准备创建新的结构

### 4. 删除失效用例
删除不再适用的测试用例

### 5. 批量清理
删除某个模块下的所有节点

---

## 注意事项

1. **输入格式**：
   - 删除操作只支持从标准输入读取 UID 数组
   - 使用 `echo '["uid1", "uid2"]' | python3 <script_dir>/scripts/write_nodes.py delete <design_uid>`

2. **MCP 渲染（必须）**：
   - 节点操作成功后，**必须调用 MCP 工具** `show_node` 渲染节点
   - 参数从 `python3 <script_dir>/scripts/load_session.py` 获取
   - 工具：`mcp_call_tool`，服务：`testbuddy_tools`，工具名：`show_node`

3. **递归删除**：删除节点时必须包含所有子孙节点的UID

4. **不可恢复**：删除操作不可撤销，执行前务必确认

5. **用户确认**：删除有子节点的节点时，建议先告知用户

6. **设计标识**：使用完整的 `design_uid`（如：`design-Az7SsiL3Ui`，不是缩写）

7. **错误处理**：脚本会返回详细的错误信息，指出具体问题

8. **🚫 禁止创建新脚本**：
   - 如果执行脚本时遇到错误，**必须直接修改原有的节点数据或命令参数**
   - **严禁创建新的辅助脚本、wrapper脚本或临时脚本来处理错误**
   - 应通过以下方式解决：
     - 检查并修正 UID 数组格式
     - 调整命令参数
     - 修复文件路径或 design_uid
     - 确认节点是否存在
   - 始终使用现有的标准脚本 `write_nodes.py`
