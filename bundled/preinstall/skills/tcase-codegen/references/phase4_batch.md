# Phase 4: 批次处理规则

## Node模式多UUID处理规则（本阶段仅适用于 `node` 模式）

### 批次处理配置
- **批次大小**: `BATCH_SIZE = 10`
- **批次间延迟**: `BATCH_DELAY = 1` 秒

### 处理模式
- 当用户选择`node`模式时，首次调用用于获取UUID列表，不要中断会话；如果返回多个UUID，则按照配置的批次大小进行`批量串行`处理

### 处理流程
1. 首次调用node模式MCP工具，获取UUID列表，并显示打印所有uuid
2. 如果返回的是UUID数组，提示用户`检测到多个测试用例UUID，将按批次（每批{BATCH_SIZE}个）生成测试代码...`
3. 将UUID列表按照`BATCH_SIZE`进行分组，即case_uuid列表中填入多个uuid组成的`uuid数组`
4. 对每个批次内的`uuid数组`使用`uuid`模式调用MCP工具
5. 每一个批次生成完成后就开始写入，不需要最后再一起写入
6. 批次间串行处理，每个批次完成后等待`BATCH_DELAY`秒再处理下一批次
7. 每次调用时保持相同的`user_repo`、`user_branch`、`user_query`、`code_path`参数
8. 将所有生成的代码按顺序分别处理；如需合并，以目标路径规则和MCP返回结果为准

### 注意事项
- 批次内以`uuid数组`形式调用MCP工具，批次间串行等待
- 每一个批次生成完成后就开始写入，不需要最后再一起写入
- 如果某个批次中的UUID调用失败，继续处理该批次的其他UUID和后续批次
- 最终向用户报告所有UUID的处理结果（成功/失败数量）
- 保持代码生成的一致性，所有用例应该生成到同一个目标位置
- 可通过修改文件顶部的`BATCH_SIZE`和`BATCH_DELAY`配置来调整处理策略

## Abnormal模式处理规则

### 批次处理配置
- **批次大小**: `BATCH_SIZE = 1`
- **批次间延迟**: `BATCH_DELAY = 1` 秒

### 处理模式
- 当用户选择`node`模式时，MCP工具可能会返回多个Prompt列表，不要中断会话，此时需要按照配置的批次大小进行`批量串行`处理
- 当使用工具为update_scenarios时, 必须携带update_info参数, 用户描述没有更新场景信息描述时，这个参数值为空字符串

### 处理流程
1. 调用abnormal模式MCP工具, 获取Prompt字符串或者Prompt列表
2. 如果返回的是Prompt列表, 提示用户`检测到多个Prompt，将按批次（每批{BATCH_SIZE}个）生成测试代码...`
3. 如果返回的是Prompt列表, 对每个批次内的Prompt作为CodeBuddy的Prompt调用模型处理，切记不要再次调用MCP工具
4. 如果返回的是Prompt字符串, 对返回的Prompt作为CodeBuddy的Prompt调用模型处理，切记不要再次调用MCP工具
