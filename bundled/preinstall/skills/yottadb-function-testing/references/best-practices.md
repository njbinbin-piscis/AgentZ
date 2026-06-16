# 最佳实践与常见问题

本文档汇总功能测试代码生成的最佳实践、常见错误和解决方案。

## 一、代码生成最佳实践

### 1.1 生成流程

```
1. 分析测试用例描述
   ├── 识别测试模块（TSDB/MEMDB/Control Plane 等）
   ├── 识别测试类型（CRUD/工作流/压测等）
   └── 识别涉及的资源（Account/SC/Partition 等）

2. 检索项目代码
   ├── 确定继承基类
   ├── 确认 API 存在性
   ├── 查找相似实现
   └── 收集可复用代码

3. 生成代码框架
   ├── 类定义和继承
   ├── __init__ 方法
   ├── setUp/tearDown
   └── 测试方法结构

4. 实现测试逻辑
   ├── 按步骤实现
   ├── 添加断言验证
   ├── 添加日志记录
   └── 处理资源清理

5. 代码质量检查
   ├── 无 TODO 占位符
   ├── 无未使用导入
   ├── docstring 完整
   └── 资源清理完整
```

### 1.2 命名规范

| 元素 | 规范 | 示例 |
|------|------|------|
| 测试类名 | `Test` + 功能描述 | `TestSubCluster` |
| 测试方法名 | `test_` + 操作 + 场景 | `test_update_sc_alloc_affinity` |
| 资源名前缀 | 集群名 + 类名 | `cluster-gz-TestSubCluster` |
| 变量名 | 小写 + 下划线 | `account_id`, `sc_list` |

## 二、资源管理最佳实践

### 2.1 资源创建

```python
# ✅ 正确：创建前先清理
existing_id = self.control_plane.sys_admin.get_account_id_by_name_without_app_id(account_name)
if existing_id != "":
    self.account_id = existing_id
    self.clean_account_and_resources()

account_id = self.create_tsdb_resource_and_init_info(...)

# ❌ 错误：不检查直接创建
account_id = self.create_tsdb_resource_and_init_info(...)  # 可能因重名失败
```

### 2.2 资源清理

```python
# ✅ 正确：使用列表管理多个账户
def __init__(self):
    self.account_ids = []  # 管理多个账户

def test_xxx(self):
    account_id = self.create_tsdb_resource_and_init_info(...)
    self.account_ids.append(account_id)  # 注册到清理列表

def tearDown(self):
    for account_id in self.account_ids:
        self.account_id = account_id
        self.clean_account_and_resources()
```

## 三、工作流处理最佳实践

```python
# ✅ 正确：指定期望状态
res = self.control_plane.sys_admin.check_workflow_finished(
    rsp.workflow_id, 
    status=4  # 明确期望成功
)
assert_true(res)

# 对于预期失败的场景
res = self.control_plane.sys_admin.check_workflow_finished(
    rsp.workflow_id, 
    status=6  # 期望回滚
)
assert_true(res)

# ✅ 正确：先检查错误码
rsp = self.sys_admin.update_sc_alloc_affinity(...)
assert_equal(rsp.err_code, 0)  # 先检查请求是否成功
res = self.sys_admin.check_workflow_finished(rsp.workflow_id)
assert_true(res)
```

## 四、断言使用最佳实践

```python
# 检查相等
assert_equal(actual, expected)

# 检查不等
assert_not_equal(actual, expected)

# 检查真值
assert_true(condition)

# 检查非空
assert_not_none(obj)
assert_not_equal(string, "")

# 检查列表包含
assert_in(element, list)

# 检查大小比较
assert_greater(a, b)  # a > b
assert_not_less(a, b)  # a >= b
```

## 五、日志记录最佳实践

```python
def test_xxx(self):
    global_var.logger.info("start to exec %s" % self.case_name)
    
    global_var.logger.info("Step 1: 创建账户")
    account_id = self.create_tsdb_resource_and_init_info(...)
    global_var.logger.info("created account_id: %s" % account_id)
    
    global_var.logger.info("Step 2: 占用 SC")
    self.occupy_sc(sc_cnt=2, alloc_affinity=account_name)
    
    global_var.logger.info("test case %s finished" % self.case_name)
```

## 六、常见错误及解决

### 6.1 幻觉错误

**问题**：引用不存在的 API 或字段

```python
# ❌ 错误：使用不存在的方法
rsp = self.sys_admin.get_sc_by_id(sc_id)  # 方法不存在

# ✅ 解决：先检索确认
# grep "def get_sc" test/dao/sys_admin_dao.py
```

### 6.2 参数错误

```python
# ❌ 错误：参数顺序错误
rsp = self.sys_admin.update_sc_alloc_affinity(
    alloc_affinity=affinity,
    sub_cluster_ids=[sc_id]  # 顺序反了
)

# ✅ 正确：先检索参数顺序
rsp = self.sys_admin.update_sc_alloc_affinity(
    sub_cluster_ids=[sc_id],
    alloc_affinity=affinity
)
```

### 6.3 基类选择错误

```python
# ❌ 错误：TSDB 测试继承 Control Plane 基类
class TestTsdbFeature(TestControlPlaneBase):

# ✅ 正确：选择合适的基类
class TestTsdbFeature(TestTsdbBase):
```

### 6.4 Protocol 字段错误

```python
# ❌ 错误：字段名错误
account_id = rsp.account.id  # 正确是 base_info._rid

# ✅ 正确：使用正确字段
account_id = rsp.account.base_info._rid
```

## 七、docstring 规范

```python
def test_example(self):
    """
    test_scene: 测试场景描述（必需）
    test_step:
        1、步骤1描述（必需）
        2、步骤2描述
    expect_result:
        1、步骤1预期结果（必需）
        2、步骤2预期结果
    note: 备注信息（可选）
    level: 0（必需，0/1/2）
    author: userid（必需）
    update_person: （可选）
    design_case_uuid: uuid-string（必需）
    """
```

## 八、测试隔离原则

```python
# ✅ 每个测试方法独立可执行
def test_case_1(self):
    # 完整的前置准备
    # 测试逻辑

def test_case_2(self):
    # 不依赖 test_case_1 的执行
    # 有自己完整的前置准备

# ✅ 使用唯一的资源名
account_name_1 = self.account_name + "-test1"
account_name_2 = self.account_name + "-test2"
```

## 九、检查清单

### 代码生成前

- [ ] 已识别目标模块和基类
- [ ] 已确认 API 存在性
- [ ] 已查找相似实现参考
- [ ] 已确认 Protocol 字段正确

### 代码生成后

- [ ] 无 TODO/FIXME/XXX 占位符
- [ ] 所有导入都被使用
- [ ] docstring 字段完整
- [ ] 每个步骤有断言验证
- [ ] 资源清理完整
- [ ] 日志记录完整
- [ ] 可独立执行
