# LD 类接口测试用例参考

> **本文件是 LD 类接口测试代码生成的参考示例，包含公共模块速查、三层校验模式和正向用例示例。**
> - 如果目标文件中**没有已有用例**，以本文件为主要参考来生成代码
> - 如果目标文件中**已有用例**，需同时参考本文件和已有用例，以已有用例的编码风格为准，以本文件的规范和模式为补充

## 设计要点

- **公共模块复用**：`ld_common.py` 提供参数构造、清理、DB 校验、get_ld API 校验、LD(nginx) 校验等全部公共函数（详见下方速查表）
- **clean fixture 工厂函数模式**：`yield register` 返回注册函数，用例中调用 `clean_ld(_set_id=..., _ld_ip=..., _force=True, _product=...)` 注册清理参数
- **三层校验模式**：正向用例必须依次执行 DB 校验 → get_ld API 校验 → LD(nginx) 校验；异常用例同样三层校验（expect_exist=False）
- **LD 类特有规则**：
  - 不需要 `wait_job_done`（AddLd 是同步操作）
  - 成功响应判断用 `assert_response_success(response)`，失败用 `assert_response_failed(response)`
  - DB 校验覆盖两张表：`alb_ld`、`alb_ld_support_ip`
  - **LD(nginx) 校验**：正向场景通过 `command.executor` 远程到 LD 机器验证 alb_agent 进程、nginx 进程和配置目录；异常场景通过 get_ld API 确认 LD 未被注册（不直接连接可能无效的 LD IP）
- **所有函数调用参数必须使用 `key=value` 形式**，不允许使用位置参数
- **`command` fixture**：所有有 set_id 的用例必须声明 `command` fixture 参数，用于远程命令执行
- **clean_ld 规则**：只要涉及正向创建场景（add_ld 成功），必须包含 `clean_ld` fixture 并在接口返回成功后注册 teardown；异常场景不需要 `clean_ld`
- **set_id 来源**：从 `alb_config.get("Set").get("Default")` 获取默认 set_id

## 一、公共模块接口速查表与关键代码

> **导入方式**：`from testcases.api.ld.ld_common import clean_ld, DEFAULT_CAPACITY_INFO, build_ld_item, build_add_ld_params, verify_add_ld_db_data, verify_ld_api, verify_ld_nginx`
> **断言函数导入**：`from testcases.api.common import assert_response_success, assert_response_failed`
> **生成 LD 类接口测试代码时，必须复用此公共模块中的函数，不得重复实现。**

### 函数速查表

| 函数 | 用途 | 关键参数 |
|------|------|----------|
| `clean_ld` (fixture) | 工厂函数模式的 teardown fixture | `_set_id`, `_ld_ip`, `_force`, `_product` |
| `clean_ld_verify` | 删除 LD 并校验 DB 和 API 已清理 | `set_id`, `ld_ip`, `force`, `product`, `alb_api`, `oss_db`, `case_logger`, `alb_config` |
| `assert_response_success` | 校验接口成功响应（无 code/message，有 request_id） | `response` |
| `assert_response_failed` | 校验接口失败响应（有 code/message） | `response` |
| `assert_field_equal` | 通用字段断言 | `actual_value`, `expected`, `field`, `context` |
| `build_ld_item` | 构造单个 ld_list item | `**overrides`（svr_asset_id, ld_ip, weight, az_id, type, local_ip_list, capacity_info） |
| `build_add_ld_params` | 构造 add_ld 请求参数 | `product`, `set_id`, `ld_list`, `**overrides` |
| `verify_add_ld_db_data` | 校验 DB 两张表（alb_ld + alb_ld_support_ip） | `set_id`, `ld_list`, `oss_db`, `case_logger`, `expect_exist` |
| `verify_ld_api` | 通过 get_ld 接口校验所有字段 | `set_id`, `ld_list`, `product`, `alb_api`, `alb_config`, `case_logger`, `expect_exist` |
| `verify_ld_nginx` | 远程到 LD 机器校验 nginx/alb_agent 进程和配置 | `set_id`, `ld_list`, `product`, `alb_api`, `alb_config`, `command`, `case_logger`, `expect_exist` |
| `DEFAULT_CAPACITY_INFO` | 默认 capacity_info 常量 | 包含全部 10 个字段 |

### clean_ld fixture 代码（工厂函数模式）

```python
@pytest.fixture
def clean_ld(
        alb_api,
        case_logger,
        oss_db,
):
    """独立的 fixture teardown，使用工厂函数模式，支持传入 set_id, ld_ip, force 和 product 参数"""
    _registered = []

    def register(
            _set_id,
            _ld_ip,
            _force,
            _product,
    ):
        _registered.append({"product": _product, "set_id": _set_id, "ld_ip": _ld_ip, "force": _force})

    yield register

    case_logger.step("【teardown】清理LD")
    for ld in _registered:
        product = ld["product"]
        set_id = ld["set_id"]
        ld_ip = ld["ld_ip"]
        force = ld["force"]

        sql = f"SELECT id FROM alb_ld WHERE ip = '{ld_ip}' AND set_id = {set_id}"
        rows = oss_db.execute_sql(command=sql)
        if len(rows) == 0:
            case_logger.info(f"ld: {ld_ip} 不存在，skip teardown")
            continue

        clean_ld_verify(
            set_id=set_id,
            ld_ip=ld_ip,
            force=force,
            product=product,
            alb_api=alb_api,
            oss_db=oss_db,
            case_logger=case_logger,
        )
```

### 参数构造函数签名

```python
DEFAULT_CAPACITY_INFO = {
    "http_max_new_conn": 10000,
    "http_qps": 5000,
    "https_max_new_conn": 8000,
    "https_qps": 4000,
    "max_active_conn": 20000,
    "max_conn": 50000,
    "in_max_bandwidth": 1000,
    "out_max_bandwidth": 1000,
    "in_max_pkt_size": 1500,
    "out_max_pkt_size": 1500,
}

def build_ld_item(**overrides):
    """构造单个 ld_list item，通过 overrides 覆盖差异字段"""
    item = {
        "svr_asset_id": overrides.get("svr_asset_id", "SVR-20240112001"),
        "ld_ip": overrides.get("ld_ip", "10.0.1.100"),
        "weight": overrides.get("weight", 100),
        "az_id": overrides.get("az_id", 1001),
        "type": overrides.get("type", "NFV"),
        "local_ip_list": overrides.get("local_ip_list", ["192.168.1.100", "192.168.1.101"]),
        "capacity_info": overrides.get("capacity_info", dict(DEFAULT_CAPACITY_INFO)),
    }
    return item

def build_add_ld_params(product, set_id, ld_list=None, **overrides):
    """构造 AddLd 请求参数，通过 overrides 覆盖任意字段"""
    params = {
        "product": product,
        "set_id": set_id,
        "ld_list": ld_list if ld_list is not None else [build_ld_item()],
    }
    params.update(overrides)
    return params
```

### clean_ld 使用决策表

| 场景 | ld_ip 有效且可能成功? | 需要 clean_ld? |
|------|---------------------|----------------|
| AddLd 正向（创建成功） | ✅ | ✅（在接口成功返回后注册） |
| AddLd 异常（set_id 异常/不存在） | ❌ | ❌ |
| AddLd 异常（ld_ip 异常/空） | ❌ | ❌ |
| AddLd 异常（ld_list 为空/超限） | ❌ | ❌ |
| AddLd 异常（weight/az_id/svr_asset_id 等参数异常，但 ld_ip 有效） | ❌ | ❌ |
| DeleteLd 正向（先创建再删除） | ✅ | ✅（防止删除失败时残留） |
| DeleteLd 异常（不创建前置资源） | ❌ | ❌ |

> **核心原则**：异常用例预期接口会失败，不会真正创建 LD，因此不需要 `clean_ld`。只有正向用例（接口可能成功创建 LD）才需要。

### 三层校验模式说明

LD 类用例的校验遵循 **ALB 架构链路**（alb-api → Config Center → LD Agent → Nginx）进行三层校验：

| 层级 | 校验内容 | 公共函数 | 校验时机 |
|------|---------|---------|---------|
| 第 1 层：DB 校验 | `alb_ld` 表 + `alb_ld_support_ip` 表 | `verify_add_ld_db_data` | 所有用例必须（无论成功/失败） |
| 第 2 层：get_ld API 校验 | 通过 get_ld 接口校验字段一致性 | `verify_ld_api` | 有合法 set_id 的用例 |
| 第 3 层：LD(nginx) 校验 | 远程到 LD 机器检查进程和配置 | `verify_ld_nginx` | 有合法 set_id 的用例 |

**正向场景**（`expect_exist=True`）：
- DB 校验字段与入参一致
- get_ld API 返回数据与入参一致
- LD(nginx)：远程 SSH 检查 `alb_agent` 进程、`nginx` 进程、nginx conf 目录存在

**异常场景**（`expect_exist=False`）：
- DB 校验无记录
- get_ld API 返回 total_count=0
- LD(nginx)：通过 get_ld API 确认 LD 未注册（不 SSH 到可能无效的 IP）

## 二、AddLd 接口测试用例示例

### 2.1 正向用例（capacity_info 所有字段正确）

```python
#!/usr/bin/env python3
# -- coding: utf-8 --

import pytest

__author__ = "kairosqi"

from testcases.api.common import assert_response_success, assert_response_failed
from testcases.api.ld.ld_common import (
    clean_ld,
    build_ld_item,
    build_add_ld_params,
    verify_add_ld_db_data,
    verify_ld_api,
    verify_ld_nginx,
)


@pytest.mark.P1
@pytest.mark.Auto
@pytest.mark.Controller
@pytest.mark.ALB
@pytest.mark.CaseDescription(
    name="capacity_info所有字段填写正确",
    design_case_uuid="113fab56-7b6f-4f07-9cc3-bcfb530618f5",
    tcase_uuid="113fab56-7b6f-4f07-9cc3-bcfb530618f5",
    test_scene="ALB集群&LD测试用例 > LD > 创建LD AddLd > 参数校验 > capacity_info 所有字段填写正确",
    test_step="1、添加LD，capacity_info中所有字段正确\n2、验证接口响应成功\n3、DB 校验 LD 数据与入参一致\n4、get_ld 接口校验 LD 数据与入参一致\n5、LD(nginx) 校验 LD 已注册且 nginx 正常",
    expect_result="1、成功\n2、DB 中 LD 记录字段与入参一致\n3、get_ld 接口返回 LD 数据与入参一致\n4、LD(nginx) 校验通过",
    note="验证 capacity_info 所有字段填写正确时的完整流程",
    tapd_url="https://tapd.woa.com/tapd_fe/20426334/story/detail/1020426334123756014",
    create_person="kairosqi",
    update_person="kairosqi",
    version="",
    is_passed=False,
)
def test_add_ld_capacity_info_all_fields_correct(
        alb_api,
        case_logger,
        alb_config,
        oss_db,
        command,
        clean_ld,
):
    """测试添加LD时capacity_info所有字段填写正确"""
    case_logger.step("步骤1: 获取product配置和默认set_id")
    product = alb_config.get("Account").get("Default").get("Product")
    set_id = alb_config.get("Set").get("Default")

    case_logger.step("步骤2: 构建请求参数，capacity_info所有字段正确")
    ld_ip = "10.0.1.100"
    ld_list = [build_ld_item(ld_ip=ld_ip)]
    data = build_add_ld_params(
        product=product,
        set_id=set_id,
        ld_list=ld_list,
    )
    case_logger.info(f"请求参数: {data}")

    case_logger.step("步骤3: 调用add_ld接口")
    response = alb_api.add_ld(
        data=data,
    )
    case_logger.info(f"响应: {response}")

    case_logger.step("步骤4: 验证接口响应")
    # 如果成功，注册 clean_ld 清理
    if "code" not in response:
        clean_ld(
            _set_id=set_id,
            _ld_ip=ld_ip,
            _force=True,
            _product=product,
        )
        assert_response_success(response)

        case_logger.step("步骤5: DB 校验 LD 数据与入参一致")
        verify_add_ld_db_data(
            set_id=set_id,
            ld_list=ld_list,
            oss_db=oss_db,
            case_logger=case_logger,
            expect_exist=True,
        )

        case_logger.step("步骤6: get_ld 接口校验 LD 数据与入参一致")
        verify_ld_api(
            set_id=set_id,
            ld_list=ld_list,
            product=product,
            alb_api=alb_api,
            alb_config=alb_config,
            case_logger=case_logger,
            expect_exist=True,
        )

        case_logger.step("步骤7: LD(nginx) 校验 LD 已注册且 nginx 正常")
        verify_ld_nginx(
            set_id=set_id,
            ld_list=ld_list,
            product=product,
            alb_api=alb_api,
            alb_config=alb_config,
            command=command,
            case_logger=case_logger,
            expect_exist=True,
        )
    else:
        # 如果因其他原因失败（如 set 不存在的 az_id 等），只验证 capacity 不是失败原因
        assert "capacity" not in str(response.get("message", "")).lower(), \
            f"capacity_info应该是有效的，但返回了capacity相关错误: {response}"
```

**要点**：
- `clean_ld` 在函数参数中声明，**在接口成功返回后**调用注册清理（先注册再断言，防止断言失败后资源残留）
- 完整三层校验流程：DB 校验 → get_ld 接口校验 → LD(nginx) 校验
- `verify_ld_nginx` 必须包含 `command=command` 参数
- `set_id` 从 `alb_config.get("Set").get("Default")` 获取
- 使用 `assert_response_success` 而非手动 assert

> **异常用例模式说明**：异常用例（set_id/ld_ip/weight/az_id/capacity_info 等参数异常）不需要 `clean_ld`，但函数参数中**必须包含 `command`**（用于 LD(nginx) 校验）。校验步骤为：接口失败断言 → DB 校验（expect_exist=False）→ get_ld 校验（expect_exist=False）→ LD(nginx) 校验（expect_exist=False）。其中第 2-4 步仅在 set_id 为有效正整数时执行。具体判断规则参见上方"clean_ld 使用决策表"。

## 三、LD 类接口与 set 类接口的关键差异

| 特性 | set 类接口 | LD 类接口 |
|------|----------|----------|
| 公共模块路径 | `testcases.api.set.set_common` | `testcases.api.ld.ld_common` |
| 导入方式 | `from testcases.api.set import set_common` | `from testcases.api.ld.ld_common import (...)` |
| 校验层级 | DB + get_set API（2 层） | DB + get_ld API + LD(nginx)（3 层） |
| clean fixture | `clean_set`（需 set_name/set_id） | `clean_ld`（需 set_id/ld_ip/force/product） |
| clean 注册时机 | 读取配置后立即注册 | 接口成功返回后注册 |
| LD 配置校验 | **不需要** | **必须**（正向用 SSH，异常用 API） |
| `command` fixture | **不需要** | 所有有 set_id 的用例必须声明 |
| `wait_job_done` | 不需要 | 不需要 |
| set_id 获取 | 接口创建返回 | `alb_config.get("Set").get("Default")` |
| 异常用例 DB 校验条件 | 有些场景无 set_id 可查 | set_id 为有效正整数时才查 |

> **详细编码与测试规范**：参见 `references/standards.md`
