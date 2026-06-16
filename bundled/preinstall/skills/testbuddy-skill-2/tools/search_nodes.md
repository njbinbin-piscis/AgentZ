# search_nodes

直接操作脑图JSON文件搜索节点（无需Python脚本）。

## 🚨 重要说明

**本工具通过AI直接读取JSON文件进行搜索，操作过程中不要创建任何临时脚本或代码文件。**

## 使用方式

Agent应该直接使用 `read_file` 工具读取脑图JSON文件，然后在AI内部进行匹配和过滤。

## 操作步骤

### 1. 读取脑图文件

```
脑图文件路径: .testbuddy/assets/{design_uid}.json
```

使用 `read_file` 工具读取完整的JSON文件。

### 2. 在AI内部递归遍历节点树

从根节点的 `Children` 数组开始，递归遍历所有节点。

### 3. 应用过滤条件

根据用户提供的过滤条件匹配节点：

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
- parent_uid: 父节点UID（精确匹配）
- priority: 优先级（仅CASE类型，精确匹配）
  * P0, P1, P2, P3
```

### 4. 匹配逻辑

```
对于每个节点：
1. 跳过根节点（Kind == "DESIGN"）
2. 如果提供了query，检查节点Name或Description是否包含关键词（不区分大小写）
3. 如果提供了kind，检查节点Kind是否等于指定值
4. 如果提供了parent_uid，检查节点ParentUid是否等于指定值
5. 如果提供了priority（且节点Kind为CASE），检查Spec.Instance.Priority是否等于指定值
6. 所有条件都匹配时，将节点加入结果集
```

### 5. 返回结果格式

以清晰的文本格式呈现搜索结果：

```
找到 X 个匹配节点：

1. [节点类型] 节点名称
   - UID: xxx-xxxxxxxxxx
   - 父节点UID: xxx-xxxxxxxxxx
   - 层级: X
   - 路径: design-xxx/parent-xxx/...
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

### 示例1: 搜索登录相关的测试点

**用户请求**: "搜索所有包含'登录'的测试点"

**操作步骤**:
1. 使用 `read_file` 读取 `.testbuddy/assets/jeriezhang-test.json`
2. 在AI内部遍历所有节点
3. 应用过滤条件：
   - query: "登录"
   - kind: "TEST_POINT"
4. 匹配逻辑：
   - 检查每个节点的 `Meta.Name` 或 `Meta.Description` 是否包含"登录"（不区分大小写）
   - 且 `Meta.Kind` == "TEST_POINT"
5. 输出匹配的节点列表

### 示例2: 查找指定父节点下的所有子节点

**用户请求**: "查找 custom-cmjAXojn7Q 下的所有子节点"

**操作步骤**:
1. 读取脑图文件
2. 遍历所有节点
3. 应用过滤条件：
   - parent_uid: "custom-cmjAXojn7Q"
4. 匹配逻辑：
   - 检查 `Meta.ParentUid` == "custom-cmjAXojn7Q"
5. 输出结果

### 示例3: 搜索P0优先级的测试用例

**用户请求**: "搜索所有P0优先级的测试用例"

**操作步骤**:
1. 读取脑图文件
2. 遍历所有节点
3. 应用过滤条件：
   - kind: "CASE"
   - priority: "P0"
4. 匹配逻辑：
   - 检查 `Meta.Kind` == "CASE"
   - 且 `Spec.Instance.Priority` == "P0"
5. 输出结果

### 示例4: 组合条件搜索

**用户请求**: "搜索登录相关的、P0优先级的、未执行的测试用例"

**操作步骤**:
1. 读取脑图文件
2. 遍历所有节点
3. 应用过滤条件：
   - query: "登录"
   - kind: "CASE"
   - priority: "P0"
   - exec_state: "NONE"
4. 匹配逻辑（所有条件都必须满足）：
   - `Meta.Name` 或 `Meta.Description` 包含"登录"
   - 且 `Meta.Kind` == "CASE"
   - 且 `Spec.Instance.Priority` == "P0"
   - 且 `State.ExecState` == "NONE"
5. 输出结果

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
    "Level": "节点层级",
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

## 常见搜索场景

### 1. 按关键词搜索
```
query: "登录"
→ 找到所有名称或描述中包含"登录"的节点
```

### 2. 按类型搜索
```
kind: "TEST_POINT"
→ 找到所有测试点节点
```

### 3. 查找子节点
```
parent_uid: "custom-cmjAXojn7Q"
→ 找到指定父节点下的所有直接子节点
```

### 4. 搜索未执行的用例
```
kind: "CASE"
→ 找到所有测试用例
```

### 5. 搜索待审核的节点
```
（暂不支持，因为简化的Node结构中不包含执行和审核状态）
```

### 6. 搜索高优先级用例
```
kind: "CASE"
priority: "P0"
→ 找到所有P0优先级的测试用例
```

### 7. 搜索需求或缺陷
```
kind: "STORY"
→ 找到所有需求节点

kind: "BUG"
→ 找到所有缺陷节点
```

## 注意事项

1. **不要创建脚本**: 直接在AI内部进行匹配和过滤
2. **大小写不敏感**: query关键词搜索时不区分大小写
3. **精确匹配**: kind、priority、exec_state、review_state等使用精确匹配
4. **模糊匹配**: query使用包含匹配（substring）
5. **跳过根节点**: 根节点Kind为"DESIGN"，搜索时应跳过
6. **递归遍历**: 需要递归遍历整个节点树
7. **所有条件AND**: 多个过滤条件同时提供时，使用AND逻辑（都必须满足）

## 输出建议

搜索结果应该包含：
- 匹配节点的总数
- 每个节点的关键信息（UID、名称、类型、层级等）
- 如果匹配节点很多，可以考虑分页显示或只显示最相关的前N个
- 对于CASE类型节点，额外显示优先级信息
