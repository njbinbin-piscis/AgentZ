# get_descendants

直接操作脑图JSON文件查询目标节点的所有后代节点（无需Python脚本）。

## 🚨 重要说明

**本工具通过AI直接读取JSON文件进行查询，操作过程中不要创建任何临时脚本或代码文件。**

## 使用方式

Agent应该直接使用 `read_file` 工具读取脑图JSON文件，然后在AI内部进行递归查找。

## 操作步骤

### 1. 读取脑图文件

```
脑图文件路径: .testbuddy/assets/{design_uid}.json
```

使用 `read_file` 工具读取完整的JSON文件。

### 2. 找到目标节点

首先根据提供的 `target_uid` 找到目标节点。

### 3. 递归查找所有后代节点

从目标节点的 `Children` 数组开始，递归遍历所有子孙节点。

### 4. 应用过滤条件（可选）

根据用户提供的过滤条件对后代节点进行过滤：

#### 支持的过滤条件

```
- query: 关键词（模糊匹配节点名称或描述，不区分大小写）
- kind: 节点类型（精确匹配）
  * STORY - 需求
  * BUG - 缺陷
  * FEATURE - 功能模块
  * SCENE - 测试场景
  * TEST_POINT - 测试点
  * CASE - 测试用例
- priority: 优先级（仅CASE类型，精确匹配）
  * P0, P1, P2, P3
- max_level: 最大层级限制（相对于目标节点的层级）
  * 例如：1表示只查找直接子节点，2表示查找子节点和孙节点
```

### 5. 匹配逻辑

```
对于目标节点的每个后代节点：
1. 跳过根节点（Kind == "DESIGN"）
2. 如果提供了query，检查节点Name或Description是否包含关键词（不区分大小写）
3. 如果提供了kind，检查节点Kind是否等于指定值
4. 如果提供了priority（且节点Kind为CASE），检查Spec.Instance.Priority是否等于指定值
5. 如果提供了max_level，检查节点相对于目标节点的层级是否在限制内
6. 所有条件都匹配时，将节点加入结果集
```

### 6. 返回结果格式

以清晰的文本格式呈现查询结果：

```
目标节点: [节点类型] 节点名称 (UID: xxx-xxxxxxxxxx)
找到 X 个后代节点：

1. [节点类型] 节点名称
   - UID: xxx-xxxxxxxxxx
   - 相对层级: X
   - 路径: target-xxx/child-xxx/...
   - 顺序: X
   - [仅CASE] 优先级: P0/P1/P2/P3
   - 描述: ...

2. ...
```

**节点类型显示格式建议**:
- `STORY` → [需求]
- `BUG` → [缺陷]
- `FEATURE` → [模块]
- `SCENE` → [场景]
- `TEST_POINT` → [测试点]
- `CASE` → [用例]

## 完整操作示例

### 示例1: 查询某个节点的所有后代节点

**用户请求**: "查询 custom-cmjAXojn7Q 的所有后代节点"

**操作步骤**:
1. 使用 `read_file` 读取 `.testbuddy/assets/jeriezhang-test.json`
2. 在AI内部找到 UID 为 "custom-cmjAXojn7Q" 的目标节点
3. 递归遍历目标节点的所有后代节点
4. 输出所有后代节点的列表

### 示例2: 查询某个节点的所有测试点后代

**用户请求**: "查询 custom-cmjAXojn7Q 的所有测试点后代"

**操作步骤**:
1. 读取脑图文件
2. 找到目标节点
3. 递归遍历所有后代节点
4. 应用过滤条件：
   - kind: "TEST_POINT"
5. 匹配逻辑：
   - 检查每个后代节点的 `Meta.Kind` == "TEST_POINT"
6. 输出结果

### 示例3: 查询某个节点的直接子节点

**用户请求**: "查询 custom-cmjAXojn7Q 的直接子节点"

**操作步骤**:
1. 读取脑图文件
2. 找到目标节点
3. 递归遍历所有后代节点
4. 应用过滤条件：
   - max_level: 1
5. 匹配逻辑：
   - 只包含相对于目标节点层级为1的节点（直接子节点）
6. 输出结果

### 示例4: 查询某个节点下所有P0优先级的测试用例

**用户请求**: "查询 custom-cmjAXojn7Q 下所有P0优先级的测试用例"

**操作步骤**:
1. 读取脑图文件
2. 找到目标节点
3. 递归遍历所有后代节点
4. 应用过滤条件：
   - kind: "CASE"
   - priority: "P0"
5. 匹配逻辑：
   - 检查 `Meta.Kind` == "CASE"
   - 且 `Spec.Instance.Priority` == "P0"
6. 输出结果

### 示例5: 组合条件查询

**用户请求**: "查询 custom-cmjAXojn7Q 下包含'登录'关键词的、P0优先级的测试用例"

**操作步骤**:
1. 读取脑图文件
2. 找到目标节点
3. 递归遍历所有后代节点
4. 应用过滤条件：
   - query: "登录"
   - kind: "CASE"
   - priority: "P0"
5. 匹配逻辑（所有条件都必须满足）：
   - `Meta.Name` 或 `Meta.Description` 包含"登录"
   - 且 `Meta.Kind` == "CASE"
   - 且 `Spec.Instance.Priority` == "P0"
6. 输出结果

## 节点JSON结构参考

```json
{
  "Meta": {
    "Uid": "节点UID",
    "ParentUid": "父节点UID",
    "Kind": "节点类型",
    "Name": "节点名称",
    "Description": "节点描述",
    "Path": "节点路径",
    "Level": "节点层级（相对于根节点）",
    "Order": "节点顺序",
  },
  "Spec": {
    "Instance": {
      "Priority": "优先级(仅CASE)",
      "PreConditions": "前置条件",
      "Steps": []
    }
  },
  "State": {
    "ExecState": "执行状态",
    "ReviewState": "审核状态"
  },
  "Children": []
}
```

## 常见查询场景

### 1. 查询所有后代节点
```
target_uid: "custom-cmjAXojn7Q"
→ 找到该节点下的所有子孙节点
```

### 2. 按类型查询后代
```
target_uid: "custom-cmjAXojn7Q"
kind: "TEST_POINT"
→ 找到该节点下所有测试点类型的后代节点
```

### 3. 按层级限制查询
```
target_uid: "custom-cmjAXojn7Q"
max_level: 2
→ 找到该节点下相对层级为2以内（子节点和孙节点）的所有节点
```

### 4. 查询特定关键词的后代
```
target_uid: "custom-cmjAXojn7Q"
query: "登录"
→ 找到该节点下所有名称或描述中包含"登录"的后代节点
```

### 5. 查询高优先级用例后代
```
target_uid: "custom-cmjAXojn7Q"
kind: "CASE"
priority: "P0"
→ 找到该节点下所有P0优先级的测试用例
```

### 6. 组合条件查询
```
target_uid: "custom-cmjAXojn7Q"
kind: "CASE"
query: "支付"
max_level: 3
→ 找到该节点下相对层级为3以内、名称或描述包含"支付"的所有测试用例
```

## 相对层级计算

相对层级 = 后代节点Level - 目标节点Level

例如：
- 目标节点 Level = 3
- 后代节点 Level = 5
- 相对层级 = 5 - 3 = 2（表示是目标节点的孙节点）

## 注意事项

1. **不要创建脚本**: 直接在AI内部进行递归查找和过滤
2. **必须提供target_uid**: 必须指定目标节点UID才能查询其后代节点
3. **递归遍历**: 需要递归遍历目标节点的整个子树
4. **大小写不敏感**: query关键词搜索时不区分大小写
5. **精确匹配**: kind、priority等使用精确匹配
6. **模糊匹配**: query使用包含匹配（substring）
7. **所有条件AND**: 多个过滤条件同时提供时，使用AND逻辑（都必须满足）
8. **先找目标节点**: 在查询后代之前，必须先定位到目标节点

## 输出建议

查询结果应该包含：
- 目标节点的基本信息（UID、名称、类型）
- 匹配的后代节点总数
- 每个后代节点的关键信息（UID、名称、类型、相对层级等）
- 对于CASE类型节点，额外显示优先级信息
- 如果匹配节点很多，可以考虑分页显示或只显示最相关的前N个
