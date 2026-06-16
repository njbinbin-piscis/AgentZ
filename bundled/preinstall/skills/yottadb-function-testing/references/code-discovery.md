# 代码检索指南

本文档提供在项目中检索代码、API、函数和模式的详细指南，是防止"幻觉"的核心工具。

## 一、检索优先原则

### 1.1 何时必须检索

| 场景 | 必须检索 | 原因 |
|------|----------|------|
| 调用 DAO 方法 | ✅ | 确认方法存在和参数签名 |
| 使用 Protocol 字段 | ✅ | 确认字段名称正确 |
| 继承基类 | ✅ | 确认基类存在和方法 |
| 使用枚举值 | ✅ | 确认枚举成员正确 |
| 复用通用函数 | ✅ | 确认函数存在和用法 |
| 编写新逻辑 | ⚠️ | 优先查找已有实现 |

### 1.2 检索工具优先级

1. **grep** - 精确匹配函数名、类名、字段名
2. **codebase_search** - 语义搜索相似实现
3. **read_file** - 阅读完整文件内容
4. **list_dir** - 探索目录结构

## 二、DAO 层检索

### 2.1 检索 DAO 方法签名

```bash
# 检索系统管理 DAO 所有方法
grep "def " test/dao/sys_admin_dao.py

# 检索特定方法
grep -A 10 "def update_sc_alloc_affinity" test/dao/sys_admin_dao.py

# 检索用户管理 DAO
grep "def " test/dao/user_admin_dao.py

# 检索数据管理 DAO
grep "def " test/dao/data_admin_dao.py
```

### 2.2 检索 DAO 方法参数

```bash
# 查看方法完整定义（包含参数）
grep -A 20 "def create_database_account" test/dao/user_admin_dao.py

# 查找方法调用示例
grep -r "create_database_account" test/case/ --include="*.py" | head -20
```

### 2.3 DAO 文件快速索引

| DAO 文件 | 主要功能 |
|----------|----------|
| `sys_admin_dao.py` | 账户、SC、工作流、分区管理 |
| `user_admin_dao.py` | 账户、数据库、Collection 用户操作 |
| `data_admin_dao.py` | 副本、快照、分区数据操作 |
| `controlplane.py` | 控制面统一入口 |
| `kubernets_dao.py` | K8s 操作 |
| `chaos_dao.py` | 混沌工程（故障注入） |

## 三、基类检索

### 3.1 检索基类方法

```bash
# TSDB 基类方法
grep "def " test/case/tsdb/test_tsdb_base.py

# 数据面基类方法
grep "def " test/case/data_plane/test_cluster_case_base.py

# 系统基类方法
grep "def " test/case/system_case.py
```

### 3.2 检索基类属性

```bash
# 查找 __init__ 中定义的属性
grep -A 50 "def __init__" test/case/tsdb/test_tsdb_base.py | grep "self\."

# 查找特定属性的使用
grep "self.occupy_sc_list" test/case/tsdb/test_tsdb_base.py
```

## 四、Protocol Buffer 检索

### 4.1 检索消息定义

```bash
# 检索所有消息定义
grep -r "message " test/pb/

# 检索特定消息
grep -A 30 "message Account" test/pb/
```

### 4.2 检索枚举定义

```bash
# 检索所有枚举
grep -r "enum " test/pb/

# 检索资源状态枚举
grep -A 10 "enum ResourceStatus" test/pb/common.proto

# 检索 Raft 角色枚举
grep -A 5 "enum RaftRole" test/pb/common.proto
```

## 五、已有实现检索

### 5.1 检索相似测试用例

```bash
# 按功能关键词检索
grep -r "alloc_affinity" test/case/ --include="*.py" | head -30

# 按测试场景检索
grep -r "test_update_sc" test/case/ --include="*.py"

# 按 docstring 检索
grep -r "test_scene:" test/case/tsdb/ --include="*.py"
```

### 5.2 检索测试模式

```bash
# 检索工作流检查模式
grep -r "check_workflow_finished" test/case/ --include="*.py" | head -20

# 检索资源创建模式
grep -r "create_tsdb_resource_and_init_info" test/case/ --include="*.py" | head -20

# 检索清理模式
grep -r "clean_account_and_resources" test/case/ --include="*.py" | head -20
```

## 六、常用检索场景

### 6.1 需要调用一个 API

```bash
# 1. 确认方法存在
grep "def method_name" test/dao/*.py

# 2. 查看完整签名
grep -A 15 "def method_name" test/dao/xxx_dao.py

# 3. 查找使用示例
grep -r "method_name" test/case/ --include="*.py" | head -10
```

### 6.2 需要使用一个枚举

```bash
# 1. 在 proto 中查找定义
grep -r "enum EnumName" test/pb/

# 2. 查找导入方式
grep -r "EnumName" test/case/ --include="*.py" | grep "from\|import"

# 3. 查找使用示例
grep -r "EnumName\." test/case/ --include="*.py" | head -10
```

### 6.3 需要继承一个基类

```bash
# 1. 确认基类存在
grep "class ClassName" test/case/*/test_*_base.py

# 2. 查看基类方法
grep "def " test/case/xxx/test_xxx_base.py

# 3. 查找继承示例
grep -r "class Test.*ClassName" test/case/ --include="*.py" | head -10
```

## 七、检索检查清单

生成代码前，确保完成以下检索：

- [ ] **基类**：`grep "class TestXxxBase" test/case/`
- [ ] **DAO 方法**：`grep "def method_name" test/dao/`
- [ ] **Protocol 字段**：`grep "field_name" test/pb/`
- [ ] **枚举值**：`grep "ENUM_VALUE" test/pb/`
- [ ] **已有实现**：`grep -r "similar_pattern" test/case/`
- [ ] **导入语句**：`grep "from xxx import" test/case/`

## 八、常见检索命令速查

```bash
# DAO 方法
grep "def " test/dao/sys_admin_dao.py
grep "def " test/dao/user_admin_dao.py

# 基类方法
grep "def " test/case/tsdb/test_tsdb_base.py

# Protocol 消息
grep -A 20 "message Name" test/pb/

# 枚举定义
grep -A 10 "enum Name" test/pb/

# 函数调用示例
grep -r "function_name" test/case/ --include="*.py" | head -20

# 导入语句
grep -r "from module import" test/case/ --include="*.py" | head -10

# 类定义
grep -r "class ClassName" test/ --include="*.py"
```
