# Phase 2: MCP工具参数输入原则

## 重要：MCP工具调用格式

**参数需要通过 `request` 对象包裹传递，所有业务参数放在 `request` 内部！**

### 复制粘贴硬规则（否则会 JSON 解析失败）
- 只能粘贴**纯 JSON**：不能包含 `//` 注释、不能有尾随逗号、引号/花括号必须成对
- 参数需要放在 `request` 对象内部传递

## 参数提示

- 对于`user_query`，你只需要完整复制用户的问题进来即可，不需要总结
- 对于`code_path`，你只需要完整复制用户`引用的代码路径`，`code_path`传递的是路径，不是代码内容
- 对于`node_info`，必须是JSON格式字符串，包含nodeUid、projectId、designId三个字段
- 对于`case_text`，分两种情况：
  - **用户输入是结构化用例**（有明确的`测试步骤和预期结果`字段，二者`缺一不可`）：组装成JSON格式字符串，包含name、test_step、expect_result等字段
  - **用户输入不是结构化用例**（无论长短，只要没有明确区分步骤和预期结果）：**禁止自行编造步骤和预期结果，禁止将描述硬改成JSON格式**，直接把用户原文传入`case_text`即可，MCP会识别并正确处理
- 如果是文本用例的情况，一个步骤下如果有`多个小步骤`，请你`不要拆分`它们，你仍然认为这是一个步骤交给MCP工具
- 如果是文本用例的情况，一个预期结果下如果有`多个小预期结果`，请你`不要拆分`它们，你仍然认为这是一个预期结果交给MCP工具
- 请保证它们是`一一对应`的
- 不管是uuid还是文本用例的情况，除uuid以及文本用例外的用户问题中的`额外约束`请写入`case_note`参数

## 参数说明

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `op_type` | string | ✅ | 模式：uuid / text_case / node / standard |
| `user_repo` | string | ✅ | Git 仓库名 |
| `user_branch` | string | 否 | 分支名（默认 master） |
| `user_query` | string | ✅ | 用户原始问题（完整复制） |
| `case_uuid` | string[] | uuid模式必填 | UUID 数组，支持单个或多个 |
| `case_text` | string | text_case模式必填 | 结构化用例传JSON格式，非结构化描述直接传用户原文 |
| `node_info` | string | node模式必填 | JSON 格式节点信息 |
| `user_id` | string | 否 | 用户名（默认 default_user） |
| `timeout` | int | 否 | 超时时间（默认 300 秒） |
| `code_path` | string | 否 | 用户引用的 `.py` 文件路径或目录路径 |
| `case_note` | string | 否 | 额外约束条件 |
| `document` | string | standard模式必填 | RFC 文档名（如 rfc7540） |
| `section` | string | 否 | RFC 章节 |
| `chapter` | string | 否 | RFC 章节 |

## Request 完整示例（request 包裹式参数）

### Mode 1a: UUID processing (单个UUID)
```json
{
  "request": {
    "op_type": "uuid",
    "case_uuid": ["1a890156-30f7-4e55-80a2-777172a2806f"],
    "user_id": "",
    "timeout": 600,
    "user_repo": "",
    "user_branch": "",
    "user_query": "",
    "code_path": "",
    "case_note": ""
  }
}
```

### Mode 1b: UUID processing (多个UUID批量处理)
```json
{
  "request": {
    "op_type": "uuid",
    "case_uuid": ["1a890156-30f7-4e55-80a2-777172a2806f", "a1b2c3d4-e5f6-7890-abcd-ef1234567890"],
    "user_id": "",
    "timeout": 600,
    "user_repo": "",
    "user_branch": "",
    "user_query": "",
    "code_path": "",
    "case_note": ""
  }
}
```

**注意**：Mode 1b 支持批量处理多个UUID，根据 Phase 4 批次处理规则，每批最多10个UUID。

### Mode 2: Text processing
```json
{
  "request": {
    "op_type": "text_case",
    "case_text": "{\"name\": \"\", \"test_step\": [\"\"], \"expect_result\": [\"\"], \"tapd_url\": \"\", \"create_person\": \"\", \"update_person\": \"\", \"test_scene\": \"\", \"note\": \" \", \"version\": \" \", \"is_passed\": \"\", \"design_case_uuid\": \"\", \"priority\": \"\", \"preConditions\": \"\"}",
    "user_id": "",
    "timeout": 600,
    "user_repo": "",
    "user_branch": "",
    "user_query": "",
    "code_path": "",
    "case_note": ""
  }
}
```

### Mode 3: case node UID processing
```json
{
  "request": {
    "op_type": "node",
    "node_info": "{\"nodeUid\":\"\",\"projectId\":\"\",\"designId\":\"\"}",
    "user_id": "",
    "timeout": 600,
    "user_repo": "",
    "user_branch": "",
    "user_query": "",
    "code_path": "",
    "case_note": ""
  }
}
```

### Mode 4: standard processing (基于RFC标准生成)
```json
{
  "request": {
    "op_type": "standard",
    "document": "rfc7540",
    "section": "",
    "chapter": "",
    "user_id": "",
    "timeout": 600,
    "user_repo": "",
    "user_branch": "",
    "user_query": ""
  }
}
```

## MCP工具使用注意事项

- **最重要**：所有业务参数必须放在 `request` 对象内部传递
- 发起请求之前，一定要检查请求body的`json格式`是否正确！特别是花括号有没有成对出现
- `case_uuid` 是数组格式，即使只有一个UUID也要用数组：`["uuid"]`
- `node_info` 和 `case_text` 是JSON字符串格式，需要转义引号
- 如果用户引用了多个`.py`文件，请优先传入用户明确指定的目标文件路径；如果这些文件位于同一目录，也可以直接传入该目录路径
- 当你给MCP输入参数失败时，你需要仔细阅读`MCP工具参数`输入原则以输入正确的参数格式来调用
- 如果用户指定了文件夹生成路径，请优先在用户`指定的文件夹路径`生成代码；如果用户未指定，则使用TCase返回的路径
- 当MCP工具内部执行失败时，你不需要做任何重试或为用户生成代码，你`必须`直接回复`很抱歉，当前无法生成自动化测试用例，请联系TCase团队排查问题信息以发起下一次提问`并中文描述`内部报错内容`即可。
