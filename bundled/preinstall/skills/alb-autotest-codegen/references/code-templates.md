# 代码模板

> **注意**：本文件提供文件布局骨架和通用模板。具体代码细节以参考示例为准：
> - **set 类接口** → `references/example-set-tests.md`（含公共模块速查和正向用例示例）
> - **LD 类接口** → `references/example-ld-tests.md`（含三层校验模式和正向用例示例）
> - **其他接口** → `references/example-complete.md`

## 一、完整文件结构模板（推荐布局）

```python
#!/usr/bin/env python3
# -- coding: utf-8 --

import pytest

__author__ = "{作者名}"

# ==================== 1. 默认值常量 ====================
DEFAULT_CAPACITY_INFO = { ... }

# ==================== 2. 参数构造辅助函数 ====================
def build_{resource}_item(**overrides):
    ...

def build_{action}_{resource}_params(product, app_id, ...):
    ...

# ==================== 3. 参数化测试数据常量（可选） ====================
SET_ID_INVALID_CASES = [
    ("", "set_id不能为空"),
    ...
]

# ==================== 4. 统一 clean fixture（每种资源仅一个） ====================
@pytest.fixture
def clean_{resource}(case_logger, alb_api, alb_config, oss_db):
    yield
    ...

# ==================== 5. 正向测试用例 ====================
# 完整流程：构造参数 → 调用接口 → DB校验 → get接口校验 → LD配置校验

# ==================== 6. 异常参数校验用例（使用 parametrize） ====================
# 构造异常参数 → 调用接口 → 校验返回错误 → DB校验无新增数据
```

## 二、公共参数构造辅助函数模板

```python
DEFAULT_CAPACITY_INFO = {
    "max_connections": 10000,
    "max_new_connections": 5000,
    "max_bandwidth": 1000,
}

def build_{resource}_item({key_field}="{default}", **overrides):
    """构造单个{资源}项，通过 overrides 覆盖差异字段"""
    item = {
        "{field1}": overrides.get("{field1}", "{default1}"),
        "{key_field}": {key_field},
    }
    return item

def build_{action}_{resource}_params(product, app_id, {parent_id}, {resource}_list=None, **overrides):
    """构造请求参数，通过 overrides 覆盖任意字段"""
    params = {
        "product": product,
        "app_id": app_id,
        "{parent_id_field}": {parent_id},
        "{resource}_list": {resource}_list if {resource}_list is not None else [build_{resource}_item()],
    }
    params.update(overrides)
    return params
```

**使用方式**：

```python
# 正向用例 - 使用默认参数
params = build_add_ld_params(product, app_id, set_id)

# 异常用例 - 只覆盖差异字段
params = build_add_ld_params(product, app_id, set_id="")
params = build_add_ld_params(product, app_id, set_id, ld_list=[build_ld_item(weight=-1)])
```

## 三、统一 clean fixture 模板

```python
@pytest.fixture
def clean_{resource}(case_logger, alb_api, alb_config, oss_db):
    """统一的{资源}清理 fixture"""
    yield
    case_logger.step("【teardown】清理{资源名称}")

    app_id = alb_config.get("Account").get("Default").get("AppID")
    product = alb_config.get("Account").get("Default").get("Product")

    # 使用通用查询条件覆盖所有测试可能创建的资源
    sql = f"SELECT {resource_id_field} FROM {table} WHERE name LIKE 'auto-test-%' AND status != 'deleted'"
    rows = oss_db.execute_sql(
        command=sql
    )
    if len(rows) > 0:
        for row in rows:
            resource_id = row["{resource_id_field}"]
            params = {"product": product, "app_id": app_id, "{resource_id_field}": resource_id}
            delete_response = alb_api.delete_{resource}(
                data=params
            )
            assert delete_response.get("code") is None or delete_response.get("code") == 0, \
                f"清理数据失败，delete_{resource} 返回: {delete_response}"
            # 再查 DB 校验已清理
            verify_sql = f"SELECT {resource_id_field} FROM {table} WHERE {resource_id_field}='{resource_id}' AND status != 'deleted'"
            verify_rows = oss_db.execute_sql(
                command=verify_sql
            )
            assert len(verify_rows) == 0, \
                f"调用接口删除数据成功，但 DB 有残留数据，{resource_id_field}={resource_id}"
    else:
        case_logger.info("no {resource} found, skip cleanup")
```

## 四、异常参数 parametrize 模板

```python
@pytest.mark.parametrize(
    "invalid_value, expected_err",
    [
        ("", "{field}不能为空"),
        (None, "{field}不能为空"),
        ("nonexistent-value", "{field}不存在"),
    ],
    ids=["{field}_empty", "{field}_none", "{field}_nonexistent"]
)
@pytest.mark.P2
@pytest.mark.Auto
@pytest.mark.Controller
@pytest.mark.ALB
@pytest.mark.CaseDescription(
    name="{action}_{resource}异常参数校验-{field}异常",
    design_case_uuid=["uuid1", "uuid2", "uuid3"],
    tcase_uuids=["uuid1", "uuid2", "uuid3"],
    ...
)
def test_{action}_{resource}_invalid_{field}(
        alb_api, case_logger, oss_db, alb_config,
        clean_{resource}, invalid_value, expected_err,
):
    # 步骤1: 构造异常参数（使用公共构造函数）
    # 步骤2: 调用接口
    # 步骤3: 校验接口返回错误
    # 步骤4: DB 校验无新增数据（异常用例必须）
    ...
```
