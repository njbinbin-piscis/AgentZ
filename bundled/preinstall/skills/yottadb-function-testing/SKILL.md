---
name: "yottadb-function-testing"
description: "YottaDB 分布式数据库功能测试代码生成工具。当用户需要生成 YottaDB 测试代码、根据 UUID/文本用例/节点生成测试、编写 TSDB/MEMDB/SSDKV 等模块的功能测试时触发此技能。"
description_zh: "AI 驱动测试用例生成与管理工作流，集成 TestBuddy 脑图可视化和 TCase 自动化测试代码生成能力"
version: "1.0.0"
source: codebuddy
source_plugin: "tsmart"
---

> **AgentZ note:** Configure external services under Settings → MCP Servers / Connectors. Use `api_connector`, `shell`, `file_read`, and `codebase_search` instead of vendor-specific MCP tool names.

# YottaDB 功能测试代码生成 Skill

## 一、系统角色

作为 **YottaDB 分布式数据库系统** 的专业测试代码生成器。该系统包含以下核心模块：

| 模块 | 说明 | 目录位置 |
|------|------|----------|
| TSDB | 时序数据库 | test/case/tsdb/ |
| MEMDB | 内存数据库 | test/case/memdb/ |
| Control Plane | SSDKV 控制面 | test/case/control_plane/ |
| Data Plane | SSDKV 数据面 | test/case/data_plane/ |
| FasterKV | FasterKV 存储 | test/case/fasterkv/ |
| BDB | BDB 数据库 | test/case/bdb/ |
| Marvel | Marvel 管道 | test/case/marvel/ |

## 二、核心工作流程

### 1. 任务接收与处理

以下所有工作流需要启动`plan`模式，确保整个工作流都使用`plan`模式

### 2.1 接收任务时的处理流程(plan模式)

1. 接收任务
2. 解析任务参数
3. 识别目标模块
4. 检索模块架构 → 参考 `references/module-architecture.md`
5. 确定继承基类
6. 检索相关 API → 参考 `references/api-reference.md`
7. 搜索项目中类似实现 → 参考 `references/code-discovery.md`
8. 生成测试代码 → 参考 `references/code-templates.md`
9. 代码完整性验证 → 参考 `references/best-practices.md`
10. 最后给出可以直接执行的测试命令，如：
    ```bash
    nosetests -s --tests /data/zhiyan/yottadb/test/case/{module}/{sub_module}/test_{filename}.py:Test{ClassName}.test_{method_name} > output 2>&1
    ```

### 2.2 关键原则

1. **代码检索优先**：生成代码前必须检索项目中已有的实现模式
2. **复用已有函数**：优先使用项目中封装好的通用函数
3. **防止幻觉**：不确定的 API 或字段必须通过检索确认
4. **完整可执行**：生成的代码必须完整，无 TODO 占位符

## 三、项目架构概览

### 3.1 核心目录结构

- test/case/ - 测试用例主目录
  - system_case.py - 系统测试基类 TestSystemCase
  - tsdb/test_tsdb_base.py - TSDB 基类 TestTsdbBase
  - data_plane/test_cluster_case_base.py - 数据面基类
  - control_plane/test_control_plane_base.py
  - memdb/test_memdb_base.py
- test/dao/ - 数据访问对象层
  - controlplane.py - ControlPlane 统一入口
  - sys_admin_dao.py - 系统管理 DAO
  - user_admin_dao.py - 用户管理 DAO
- test/baselib/base_assert.py - 断言方法库
- test/yottadblib/yottadb_lib.py - YottaDB 工具库
- test/global_var.py - 全局变量

### 3.2 类继承关系

TestSystemCase → TestClusterCaseBase → TestTsdbBase → 具体测试类

## 四、快速参考

### 4.1 常用 DAO 入口

- self.control_plane - ControlPlane 实例
- self.sys_admin - SystemAdminDao
- self.user_admin - UserAdminDao

### 4.2 断言方法

从 baselib.base_assert 导入：
- assert_equal - 相等
- assert_not_equal - 不相等
- assert_true - 真值
- assert_not_none - 非 None
- assert_in - 包含

### 4.3 资源状态枚举

```python
from trpc_client.stub.common_pb2 import (
    ResourceStatus,      # 资源状态
    RaftRole,            # Raft 角色
    APIType,             # API 类型
    ConsistencyPolicy,   # 一致性策略
)
```

## 五、代码生成检查清单

- [ ] 基类选择正确
- [ ] API 存在性已确认
- [ ] 无 TODO 占位符
- [ ] 有断言验证
- [ ] 资源清理完整
- [ ] docstring 包含 design_case_uuid

## 六、重要约束

### 禁止行为
1. 禁止猜测 API
2. 禁止生成 TODO
3. 禁止使用 pytest 装饰器

### 必须遵守
1. 使用 global_var.logger.info 记录日志
2. 使用 assert_* 函数断言
3. 在 tearDown 中清理资源
4. docstring 包含 design_case_uuid

## 七、参考文档

详细信息请参考 `references/` 目录下的文档：

| 文档 | 用途 | 何时读取 |
|------|------|----------|
| module-architecture.md | 了解模块结构和类继承 | 确定基类时 |
| api-reference.md | 查找可用 API | 调用接口时 |
| code-templates.md | 获取代码模板 | 生成代码时 |
| code-discovery.md | 检索项目代码 | 查找类似实现时 |
| best-practices.md | 遵循最佳实践 | 代码验证时 |
| tcase-mcp-rule.md | TCase MCP 调用规范 | 调用 MCP 时 |