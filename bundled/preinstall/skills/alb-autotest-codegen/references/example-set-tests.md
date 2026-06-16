# Set 类接口测试用例参考

> **本文件是 set 类接口测试代码生成的参考示例，包含公共模块速查、设计要点和正向用例示例。**
> - 如果目标文件中**没有已有用例**，以本文件为主要参考来生成代码
> - 如果目标文件中**已有用例**，需同时参考本文件和已有用例，以已有用例的编码风格为准，以本文件的规范和模式为补充

## 设计要点

- **公共模块复用**：`set_common.py` 提供参数构造、清理、校验、断言等全部公共函数（详见下方速查表）
- **clean fixture 工厂函数模式**：`yield register` 返回注册函数，用例中调用 `clean_set(_set_id=..., _set_name=..., _product=...)` 注册清理参数
- **set 类特有规则**：不需要 LD 配置校验、不需要 VIP 绑定、不需要 `wait_job_done`，成功响应判断用 `assert_response_success(response)`
- **DB 校验覆盖三张表**：alb_set、alb_set_az_relation + alb_az、alb_set_capacity（**无论接口是否报错，都必须做 DB 校验**）
- **API 校验**：通过 get_set 接口校验所有字段（**仅当接口成功返回 set_id 时才做**）
- **所有函数调用参数必须使用 `key=value` 形式**，不允许使用位置参数
- **clean_set 规则**：只要 `set_name`/`set_id` 有值（无论成功还是失败场景，非 delete_set），必须包含 `clean_set` fixture 并注册 teardown；若 `set_name`/`set_id` 为空或 None，则不需要

## 一、公共模块接口速查表与关键代码

> **导入方式**：`from testcases.api.set import set_common`
> **生成 set 类接口测试代码时，必须复用此公共模块中的函数，不得重复实现。**

### 函数速查表

| 函数 | 用途 | 关键参数 |
|------|------|----------|
| `clean_set` (fixture) | 工厂函数模式的 teardown fixture | `_set_id`, `_set_name`, `_product` |
| `clean_az` (fixture) | AZ 记录清理 fixture | `_az_id` |
| `clean_set_verify` | 删除集群并校验 DB 已清理 | `set_id`, `product`, `alb_api`, `alb_config`, `oss_db`, `case_logger` |
| `assert_response_success` | 校验接口成功响应（无 code/message，有 request_id） | `response` |
| `assert_response_failed` | 校验接口失败响应（有 code/message） | `response` |
| `assert_field_equal` | 通用字段断言 | `actual_value`, `expected`, `field`, `context` |
| `generate_set_param` | 构造 add_set 请求参数 | `product`, `set_name`, `platform_id`, `region`, `az_list`, `remark` |
| `generate_az_list` | 生成 AZ 列表 | `nums`（默认 0 返回 None） |
| `generate_update_set_param` | 构造 update_set 请求参数（仅包含非 None 字段） | `product`, `set_id`, `set_name`, `remark`, `status`, `az_list` |
| `verify_add_set_db_data` | 校验 DB 三张表（set_id 为空时通过 set_name 查询） | `params`, `az_list`, `alb_config`, `oss_db`, `case_logger`, `set_id` |
| `verify_delete_set_db_data` | 校验删除后三张表为空 | `set_id`, `oss_db`, `case_logger` |
| `verify_update_set_db_data` | 校验更新后字段值正确 | `set_id`, `expected_fields`, `oss_db`, `case_logger` |
| `verify_set_api` | 通过 get_set 接口校验集群信息 | `set_id`, `params`, `az_list`, `product`, `alb_api`, `alb_config`, `case_logger` |
| `pre_add_az_to_db` | 向 DB 插入 AZ 记录（前置数据） | `oss_db`, `az_info` |

### clean_set fixture 代码（工厂函数模式）

```python
@pytest.fixture
def clean_set(
        case_logger,
        alb_api,
        alb_config,
        oss_db,
):
    """独立的 fixture teardown，使用工厂函数模式，支持传入 set_id, set_name 和 product 参数"""
    _registered = []

    def register(
            _set_id=None,
            _set_name=None,
            _product=None
    ):
        _registered.append({"set_id": _set_id, "set_name": _set_name, "product": _product})

    yield register

    case_logger.step("【teardown】清理集群")
    for item in _registered:
        set_name = item["set_name"]
        product = item["product"]
        set_id = item["set_id"]

        conditions = []
        if set_id is not None:
            conditions.append(f"id={set_id}")
        if set_name is not None:
            conditions.append(f"name='{set_name}'")
        if not conditions:
            case_logger.info("set_name 和 set_id 均未提供，skip teardown")
            continue
        condition = " AND ".join(conditions)

        sql = f"SELECT id FROM alb_set WHERE {condition}"
        rows = oss_db.execute_sql(command=sql)
        if len(rows) == 0:
            case_logger.info(f"no set found for {condition}, skip teardown")
            continue

        clean_set_verify(
            set_id=rows[0]["id"],
            product=product,
            alb_api=alb_api,
            alb_config=alb_config,
            oss_db=oss_db,
            case_logger=case_logger,
        )
```

### 参数构造函数签名

```python
def generate_set_param(
        product,
        set_name,
        platform_id=None,    # 默认 1
        region=None,          # 默认 "ap-guangzhou"
        az_list=None,
        remark=None           # 默认 ""
):
    # 返回包含所有字段的 dict

def generate_az_list(nums=0):
    # nums=0 返回 None；否则返回 nums 个 AZ dict 列表
    # 每个 AZ: {"az_id": 10000+i, "zh_name": f"广州{i}区", "en_name": f"ap-guangzhou-{i}"}

def generate_update_set_param(
        product,
        set_id,
        set_name=None,
        remark=None,
        status=None,
        az_list=None,
):
    # 返回仅包含非 None 字段的 dict
```

### clean_set 使用决策表

| 场景 | set_name 有值? | 需要 clean_set? |
|------|---------------|-----------------|
| AddSet 正向（创建成功） | ✅ | ✅ |
| AddSet 异常（set_name 超长/含空格等有值） | ✅ | ✅ |
| AddSet 异常（set_name 为 None 或空字符串） | ❌ | ❌ |
| AddSet 异常（region/az_list 等其他字段异常，set_name 有值） | ✅ | ✅ |
| DeleteSet 正向（先创建再删除） | ✅ | ✅（防止删除失败时残留） |
| DeleteSet 异常（set_id 不合法，不创建前置资源） | ❌ | ❌ |
| GetSet 正向（先创建再查询） | ✅ | ✅ |
| UpdateSet 正向（先创建再更新） | ✅ | ✅ |

## 二、AddSet 正向用例示例

```python
#!/usr/bin/env python3
# -- coding: utf-8 --

import pytest

__author__ = "kairosqi"

from testcases.api.set import set_common


@pytest.mark.P1
@pytest.mark.Auto
@pytest.mark.Controller
@pytest.mark.ALB
@pytest.mark.CaseDescription(
    name="创建集群，所有参数均合法/首次创建集群，获取锁成功",
    design_case_uuid=[
        "dcec6330-a29d-42bd-b9a8-c01c9259eb37",
        "551ed3dc-4cf4-421b-8a89-cf9818805831"
    ],
    tcase_uuid=[
        "dcec6330-a29d-42bd-b9a8-c01c9259eb37",
        "551ed3dc-4cf4-421b-8a89-cf9818805831"
    ],
    test_scene="ALB集群&LD测试用例 > 集群 > 创建集群AddSet > 正常创建",
    test_step="1、创建集群，所有参数均合法\n2、验证数据库中的集群配置\n3、调用 get_set 接口校验集群信息",
    expect_result="1、创建成功\n2、DB 中集群记录存在且字段与入参一致\n3、get_set 接口返回集群信息与创建入参一致",
    note="",
    tapd_url="",
    create_person="kairosqi",
    update_person="kairosqi",
    version="",
    is_passed=False
)
def test_add_set_all_params_valid(
        alb_api,
        case_logger,
        alb_config,
        oss_db,
        clean_set,
):
    """测试创建集群，所有参数均合法"""
    case_name = "创建集群，所有参数均合法"
    case_logger.info(f"开始执行用例: {case_name}")

    # 读取配置
    product = alb_config.get("Account").get("Default").get("Product")
    set_name = "auto-test-set-all-params"

    # 注册 teardown 清理参数
    clean_set(
        _set_id=None,
        _set_name=set_name,
        _product=product,
    )

    # 步骤1: 构造请求参数并创建集群
    case_logger.step("步骤1: 创建集群，所有参数均合法")

    az_list = set_common.generate_az_list(nums=1)
    params = set_common.generate_set_param(
        product=product,
        set_name=set_name,
        az_list=az_list,
    )
    response = alb_api.add_set(
        data=params
    )
    case_logger.info(f"创建集群响应: {response}")

    # 验证创建成功
    set_common.assert_response_success(response)
    assert "set_id" in response["data"], f"创建集群响应中缺少 set_id，响应: {response}"
    set_id = response["data"]["set_id"]
    case_logger.info(f"集群创建成功，set_id: {set_id}")

    # 步骤2: DB 校验 - 验证集群数据已正确写入 DB
    case_logger.step("步骤2: 验证数据库中的集群配置")
    set_common.verify_add_set_db_data(
        set_id=set_id,
        params=params,
        az_list=az_list,
        alb_config=alb_config,
        oss_db=oss_db,
        case_logger=case_logger,
    )

    # 步骤3: get 接口查询校验 - 验证 get_set 返回数据与创建入参一致
    case_logger.step("步骤3: 调用 get_set 接口校验集群信息")
    set_common.verify_set_api(
        set_id=set_id,
        params=params,
        az_list=az_list,
        product=product,
        alb_api=alb_api,
        alb_config=alb_config,
        case_logger=case_logger,
    )

    case_logger.info(f"用例 {case_name} 执行完成")
```

**要点**：
- `clean_set` 在函数参数中声明，读取配置后**立即**调用注册清理
- 完整的三步流程：创建 → DB 校验 → get 接口校验
- `verify_set_api` 必须包含 `alb_config=alb_config` 参数
- 使用 `assert_response_success` 而非手动 assert

## 三、其他接口类型要点速查

### 3.1 异常用例模式（AddSet / 其他 set 类）

| 场景 | clean_set? | DB 校验? | get_set 校验? |
|------|-----------|---------|-------------|
| set_name 为 None/空字符串 | ❌ | ✅（通过 set_name 查无数据） | ❌ |
| set_name 有值但不合法（超长/含空格等） | ✅ | ✅ | ❌ |
| region/az_list 等其他字段异常 | ✅（set_name 有值时） | ✅ | ❌ |

- 异常用例使用 `assert_response_failed(response)` 校验接口返回错误
- DB 校验使用 `verify_add_set_db_data` 不传 `set_id`（通过 set_name 查询确认无数据）
- 多种异常值使用 `@pytest.mark.parametrize` 合并

### 3.2 DeleteSet 正向用例要点

- 需要先创建前置集群，然后删除
- 仍需 `clean_set`（防止删除步骤失败时集群残留）
- 使用 `verify_delete_set_db_data` 校验三张表硬删除
- 通过 get_set 接口校验集群不存在（total_count == 0）

### 3.3 GetSet 正向用例要点

- 需要先创建前置集群，然后查询
- 需要 `clean_set`
- 使用 `assert_field_equal` 逐字段校验返回数据

### 3.4 UpdateSet 正向用例要点

- 需要先创建前置集群，然后更新
- 使用 `generate_update_set_param` 构造更新参数（仅包含需要更新的字段）
- 使用 `verify_update_set_db_data` 校验 DB 中字段已更新
- 同时通过 get_set 接口校验更新后的字段值

> **详细编码与测试规范**：参见 `references/standards.md`
