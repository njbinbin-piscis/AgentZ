# 编码与测试规范

> **参考示例**：所有生成的用例必须与对应的参考示例代码风格完全一致。
> - **set 类接口** → `references/example-set-tests.md`（含公共模块速查和正向用例示例）
> - **LD 类接口** → `references/example-ld-tests.md`（含三层校验模式和正向用例示例）
> - **其他接口** → `references/example-complete.md`

## 一、文件头部规范

```python
#!/usr/bin/env python3
# -- coding: utf-8 --

import pytest

__author__ = "{作者名}"
```

**禁止**：导入 `allure`、使用 `loguru.logger`、使用测试类（`class`）

## 二、用例装饰器规范

每个测试函数必须包含以下装饰器，**顺序固定**：

```python
@pytest.mark.P1
@pytest.mark.Auto
@pytest.mark.Controller
@pytest.mark.ALB
@pytest.mark.CaseDescription(
    name="{用例名称}",
    design_case_uuid="{UUID 或 UUID 列表}",
    tcase_uuid="{UUID 或 UUID 列表}",
    test_scene="{用例场景路径}",
    test_step="1、{步骤1}\n2、{步骤2}",
    expect_result="1、{结果1}\n2、{结果2}",
    note="",
    tapd_url="",
    create_person="{创建人}",
    update_person="{更新人}",
    version="",
    is_passed=False
)
def test_{场景描述}(alb_api, case_logger, ...):
```

**`design_case_uuid` 和 `tcase_uuid` 格式**：

| 场景 | 格式 | 示例 |
|------|------|------|
| 无 UUID | 空字符串 | `design_case_uuid=""`，`tcase_uuid=""` |
| 一对一 | 字符串 | `design_case_uuid="706ae2f9-..."`，`tcase_uuid="706ae2f9-..."` |
| 一对多（parametrize） | 列表 | `design_case_uuid=["uuid1", "uuid2"]`，`tcase_uuid=["uuid1", "uuid2"]`（同时在 parametrize 中每行传对应 UUID 字符串） |

> **注意**：`tcase_uuid` 与 `design_case_uuid` 的格式规则完全一致，两者必须同时出现、同时维护。

## 三、函数参数格式规范

**所有函数必须满足以下两点要求：**

1. **函数定义：参数不要和函数名写在同一行，每个参数必须单独一行**
2. **函数调用：所有参数必须使用 `key=value` 的形式显式传递，禁止使用位置参数**

```python
# ✅ 正确：函数定义，每个参数单独一行
def test_get_set_single_id(
        alb_api,
        case_logger,
        alb_config,
        oss_db,
        clean_set,
):
    ...

# ❌ 错误：函数定义，参数和函数名写在同一行
def test_get_set_single_id(alb_api, case_logger, alb_config, oss_db, clean_set):
    ...

# ❌ 错误：函数定义，部分参数换行但不是每个参数单独一行
def test_get_set_single_id(
        alb_api, case_logger, alb_config,
        oss_db, clean_set,
):
    ...

# ✅ 正确：函数调用，key=value 形式
response = alb_api.add_set(
    data=params,
)
set_common.verify_add_set_db_data(
    set_id=set_id,
    params=params,
    az_list=az_list,
    alb_config=alb_config,
    oss_db=oss_db,
    case_logger=case_logger,
)

# ❌ 错误：函数调用，使用位置参数
clean_set_verify(set_id, product, alb_api, alb_config, oss_db, case_logger)

```

## 四、Fixture 使用规范

### 4.1 常用 Fixture

```python
def test_xxx(
        alb_api,        # AlbApi 实例
        case_logger,    # 日志记录器（step/info/error）
        vip_config,     # VIP 配置（IPv4VPC 等）
        oss_db,         # 数据库操作（execute_sql）
        alb_config,     # alb.yml 配置
        command,        # 远程命令执行器（LD 上执行命令，LD 类用例必须声明）
):
```

> **注意**：`command` fixture 用于远程 SSH 到 LD 机器执行命令。LD 类用例中所有涉及 set_id 的用例必须声明此 fixture，即使是异常用例（用于 LD(nginx) 校验）。set 类用例不需要。

### 4.2 Teardown Fixture（必须使用 yield 模式）

所有用例必须使用独立 `@pytest.fixture` + `yield` 模式清理数据，**禁止在用例末尾内联清理**。

**清理规则**：
- **禁止直接从 DB 删除数据**，必须调用对应的 delete 接口
- 清理前先从 DB 查询资源是否存在，存在才调用 delete 接口
- 删除后必须再查 DB 校验数据已被清理，如有残留则报错
- 清理顺序：**先删子资源，再删父资源**
- 每步用 `case_logger.step("【teardown】...")` 记录

### 4.3 统一 clean fixture 规则（强制）

- **每种资源类型只允许一个 `clean_{resource}` fixture**
- **禁止**按硬编码值拆分（如 `clean_ld_weight_boundary` + `clean_ld_capacity_info`，应合并为 `clean_ld`）
- 通过 DB 查询**所有测试可能创建的资源**统一清理（使用 name 前缀、set_id 等可识别特征）

## 五、接口测试标准流程

### 5.1 create/add 接口

1. 获取资源配置（VIP、证书等，如需要）
2. 构造请求参数
3. 下发创建请求 + `wait_job_done`（如有 job）
4. DB 校验（`oss_db.execute_sql` 或 `alb_api.get_xxx`）
5. get 接口查询校验
6. LD 配置校验（按资源类型决定，**set 类跳过**）
7. fixture teardown 清理

> **LD 类 add 接口的三层校验**：LD 类 create/add 接口必须完成三层校验：
> 1. DB 校验（`verify_add_ld_db_data`）— 校验 `alb_ld` 和 `alb_ld_support_ip` 两张表
> 2. get_ld API 校验（`verify_ld_api`）— 通过 get_ld 接口校验字段一致性
> 3. LD(nginx) 校验（`verify_ld_nginx`）— 远程 SSH 检查 alb_agent 和 nginx 进程
> 
> 异常场景三层校验均传 `expect_exist=False`。仅当 set_id 为有效正整数时才执行三层校验。

### 5.2 update/modify 接口

1. 创建前置资源 + `wait_job_done`（如有 job）
2. 构造更新参数
3. 下发更新请求 + `wait_job_done`（如有 job）
4. DB 校验
5. get 接口查询校验
6. LD 配置校验（**set 类跳过**）
7. fixture teardown 清理

### 5.3 delete/remove 接口

1. 创建前置资源 + `wait_job_done`（如有 job）
2. 构造删除参数
3. 下发删除请求 + `wait_job_done`（如有 job）
4. DB 校验（数据已删除）
5. get 接口校验（资源不存在）
6. LD 配置校验（**set 类跳过**）
7. fixture teardown 清理

### 5.4 get/list/query 接口

1. 创建前置资源（可选）
2. 构造查询参数
3. 下发查询请求
4. 响应字段校验
5. DB 数据一致性校验

## 六、配置读取规范

```python
app_id = alb_config.get("Account").get("Default").get("AppID")
product = alb_config.get("Account").get("Default").get("Product")

alb_vip_list = vip_config.get("IPv4VPC")
vpc_id = alb_vip_list[0]["vpcid"]
```

## 七、日志规范

使用 `case_logger`，**不使用** `loguru.logger` 或 `print`：

```python
case_logger.step("步骤1: 创建实例 create_instance")
case_logger.info(f"创建实例响应: {response}")
case_logger.error("VIP绑定任务失败")
```

步骤命名：正常步骤 `"步骤N: {描述}"`，teardown `"【teardown】{描述}"`

## 八、异步任务等待规范

```python
if "job_id" in response:
    job_id = response["job_id"]
    param = {"product": product, "app_id": app_id, "job_ids": [job_id]}
    job_result = alb_api.wait_job_done(
        data=param
    )
    if job_result != "success":
        case_logger.error("{操作}任务失败")
        assert False, "{操作}任务失败"
```

**禁止**使用 `time.sleep` 替代 `wait_job_done`。

## 九、DB 查询规范

```python
sql = f"SELECT cert_id FROM alb_cert WHERE app_id={app_id} AND type='SVR' LIMIT 1"
rows = oss_db.execute_sql(
    command=sql
)
assert rows, f"SQL语句（{sql}）从DB中未查询到可用的SVR证书"
```

## 十、远程命令执行规范（LD 配置校验）

```python
_, output, _ = command.executor(
    command=f"cat {nginx_config_path}",
    ip=ld_ip
)
assert output, f"期望在LD {ld_ip} 上获取到配置文件"
```

**注意**：set 类接口不需要 LD 配置校验。

## 十一、公共参数构造函数规范

**当同一文件中多个用例参数结构相似时，必须抽取公共参数构造辅助函数**：

- 命名为 `build_{resource}_params(**overrides)` 或 `build_{resource}_item(**overrides)`
- 所有字段有合理默认值，通过 `overrides` 覆盖差异字段
- 复杂嵌套结构定义为模块级常量 `DEFAULT_{STRUCT_NAME}`
- 放在文件顶部（fixture 之前）

## 十二、`@pytest.mark.parametrize` 规范

**测试逻辑相同、仅参数值不同的用例，必须使用 `@pytest.mark.parametrize` 合并**：

- `ids` 必须有清晰可读的描述
- `@pytest.mark.parametrize` 放在所有 `@pytest.mark` 装饰器之前
- 多个文本用例合并时，`design_case_uuid` 和 `tcase_uuid` 必须同时出现在 `@pytest.mark.parametrize` 参数列表和 `@pytest.mark.CaseDescription` 中
  - `@pytest.mark.parametrize` 中：每个参数组合对应一个 UUID 字符串（`design_case_uuid` 和 `tcase_uuid` 各一个）
  - `@pytest.mark.CaseDescription` 中：使用列表格式汇总所有 UUID（`design_case_uuid` 和 `tcase_uuid` 各一个列表）

```python
# ✅ 正确示例
@pytest.mark.parametrize(
    "set_name, case_name, design_case_uuid, tcase_uuid",
    [
        (None, "set_name不下发", "uuid1", "uuid1"),
        ("", "set_name长度小于1个字符", "uuid2", "uuid2"),
    ],
    ids=["set_name_not_provided", "set_name_length_less_than_min"],
)
@pytest.mark.CaseDescription(
    name="创建集群set_name参数校验失败",
    design_case_uuid=["uuid1", "uuid2"],
    tcase_uuid=["uuid1", "uuid2"],
    ...
)
def test_add_set_set_name_invalid(set_name, case_name, design_case_uuid, tcase_uuid, alb_api, ...):
    ...
```

| 场景 | 是否使用 parametrize |
|------|---------------------|
| 同一字段多种异常值 | ✅ 必须 |
| 同一字段多种边界值 | ✅ 必须 |
| 正向用例与异常用例 | ❌ 不使用 |

## 十三、异常用例 DB 校验规范

异常场景用例在断言接口返回错误后，**必须查询 DB 验证数据未被写入**，确保数据完整性。

## 十四、接口报错处理规范

### 14.1 AddSet 接口

**无论接口是否报错（有无 `code` 和 `message` 字段），都必须做 DB 校验。**

- **接口成功**（无 `code`/`message`，有 `set_id`）：执行 DB 校验 + get 接口校验
- **接口报错**（有 `code`/`message`，无 `set_id`）：只执行 DB 校验，跳过 get 接口校验

接口成功判断使用 `set_common.assert_response_success(response)`，不要手动 assert `"code" not in response`。

### 14.2 AddLd 接口

**异常用例断言接口失败后，必须进行三层校验（均传 `expect_exist=False`）。**

- **接口成功**（正向用例）：先注册 `clean_ld` → `assert_response_success` → DB 校验 → get_ld API 校验 → LD(nginx) 校验
- **接口失败**（异常用例）：`assert_response_failed` → DB 校验 → get_ld API 校验 → LD(nginx) 校验（三层均 `expect_exist=False`）
- **set_id 无效**（None/0 等）：仅断言接口失败，跳过三层校验；set_id 为正整数但不存在时，执行三层校验

## 十五、参数限制以源码为准

生成用例时，如果文本测试用例中对参数的限制（如字段长度、取值范围、枚举值等）与源码实现不一致，**一律以源码为准**。生成前必须先读取源码（`logic/` 或 `service/` 目录）确认实际限制，再生成对应的测试用例。

## 十六、禁止行为清单

1. 禁止使用 `allure` 装饰器或 `allure.step`
2. 禁止使用测试类
3. 禁止将 `CASE_DESCRIPTION` 写在 docstring 中
4. 禁止使用 `loguru.logger`
5. 禁止跳过 DB 校验或数据清理
6. 禁止在用例末尾内联清理
7. 禁止硬编码敏感信息
8. 禁止使用 `time.sleep`
9. 禁止定义未使用的变量
10. 禁止 teardown 中直接从 DB 删除数据
11. 禁止函数参数和函数名写在同一行
12. 禁止多个用例重复构造相同参数（必须抽取公共函数）
13. 禁止逻辑相同的用例写成独立函数（必须 parametrize）
14. 禁止同一资源类型有多个 clean fixture
15. 禁止函数调用使用位置参数（必须 `key=value` 形式）
16. 禁止 AddSet 接口报错时跳过 DB 校验
17. 禁止以文本用例描述的参数限制为准（必须以源码为准）
18. **禁止 set 类接口成功场景（非 delete_set）不注册 `clean_set` teardown**（例外：`set_name`/`set_id` 为空或 None 的异常场景无需）
19. **禁止 LD 类接口正向用例缺少三层校验**（DB + get_ld API + LD(nginx)，三层缺一不可）
20. **禁止 LD 类用例异常场景注册 `clean_ld`**（异常场景不会真正创建 LD，无需清理）
21. **禁止 LD 类用例遗漏 `command` fixture**（所有有 set_id 的 LD 用例必须声明 `command` 参数）
22. **禁止 LD(nginx) 异常校验直接 SSH 到 LD 机器**（异常场景 ld_ip 可能无效，必须通过 get_ld API 校验）

## 十七、代码完整性检查清单

生成代码后逐项检查：

### 基础规范
- [ ] 文件头部正确（无 allure）
- [ ] 装饰器完整且顺序正确
- [ ] `design_case_uuid` 和 `tcase_uuid` 已填写（parametrize 用例同时在参数列表中每行传对应 UUID）
- [ ] 顶层函数，无测试类
- [ ] 使用 `case_logger.step/info/error`
- [ ] 函数参数每行一个
- [ ] 所有函数调用参数使用 `key=value` 形式

### 测试流程
- [ ] 测试流程完整（按接口类型，参见第五章）
- [ ] 异步操作使用 `wait_job_done`
- [ ] 独立 fixture teardown（yield 模式）
- [ ] 每种资源统一一个 clean fixture
- [ ] 异常用例包含 DB 校验
- [ ] 正向用例包含 DB + get 接口 + LD 校验（set 类跳过 LD）

### 代码质量
- [ ] 相似用例使用 parametrize
- [ ] 公共参数抽取辅助函数
- [ ] 无未使用变量
- [ ] 参数限制已与源码核对一致

### set 类接口专项（仅 set 类适用）
- [ ] 复用 `set_common.py` 中的公共函数
- [ ] AddSet：无论接口是否报错都有 DB 校验
- [ ] AddSet：接口成功判断使用 `assert_response_success`
- [ ] AddSet：`verify_set_api` 包含 `alb_config` 参数
- [ ] clean_set 规则：成功场景函数参数包含 `clean_set` 并注册 teardown
- [ ] clean_set 规则：set_name 为 None/空字符串的异常场景不需要 `clean_set`

### LD 类接口专项（仅 LD 类适用）
- [ ] 复用 `ld_common.py` 中的公共函数（`build_ld_item`、`build_add_ld_params`、`verify_add_ld_db_data`、`verify_ld_api`、`verify_ld_nginx`）
- [ ] 导入方式正确：`from testcases.api.ld.ld_common import (...)`，断言函数从 `testcases.api.common` 导入
- [ ] 三层校验完整：正向用例包含 DB 校验 + get_ld API 校验 + LD(nginx) 校验
- [ ] 异常用例三层校验均传 `expect_exist=False`
- [ ] 所有有 set_id 的用例均声明 `command` fixture 参数
- [ ] `verify_ld_nginx` 调用包含 `command=command` 参数
- [ ] `set_id` 从 `alb_config.get("Set").get("Default")` 获取
- [ ] clean_ld 规则：仅正向场景（接口成功返回后）注册 `clean_ld` teardown
- [ ] clean_ld 规则：异常场景不需要 `clean_ld`
- [ ] clean_ld 注册时机：在接口成功返回后（`if "code" not in response`），先注册 clean_ld 再 assert
- [ ] set_id 为无效值（None/0/不存在）时，跳过三层校验（仅做接口断言）
