# `__init__.py` 公共函数封装规范

> ⚠️ **核心原则**：同一目录下多个测试文件共用的逻辑（包括函数和变量），**必须**封装写入该目录的 `__init__.py`，测试文件通过 `from .__init__ import *` 直接调用，**禁止**在多个测试文件中重复实现相同逻辑。

> 更多代码模式参考见 [code_patterns.md](code_patterns.md)。

## 变量定义位置决策规则

生成代码时，如果测试函数中需要使用某个变量，按以下优先级决定其来源：

| 优先级 | 变量来源 | 说明 |
|--------|----------|------|
| 1 | 测试函数内已通过 `self.xxx = ...` 赋值的变量 | 直接使用，无需重复定义 |
| 2 | 模块 `__init__.py` 中已预定义的变量 | 通过 `from .__init__ import *` 自动导入，直接引用 |
| 3 | `keywords_api.py` / `entry.py` 中的全局变量或函数 | 通过 `from entry import *` 自动导入，直接引用 |
| 4 | 测试文件内**新定义**（仅当以上来源均无时） | 若同目录多个文件都需要，应封装到 `__init__.py` |

> ⚠️ **核心规则**：如果测试函数内**没有**某个变量，或者**不能直接使用**，应优先检查模块 `__init__.py` 中是否已有定义。如果没有且多个测试文件都需要，**必须在 `__init__.py` 中新增定义**，而不是在每个测试文件中各自定义。

## 判断标准：何时应封装到 `__init__.py`

| 场景 | 是否封装 |
|------|----------|
| 同目录 ≥2 个测试文件都需要的断言/验证逻辑 | ✅ 必须封装 |
| 同目录 ≥2 个测试文件都需要的数据处理/格式化 | ✅ 必须封装 |
| 同目录 ≥2 个测试文件都需要的场景初始化步骤 | ✅ 必须封装 |
| 测试函数需要某个期望结果变量（如 `expected_xxx`），但 `__init__.py` 中没有 | ✅ 必须在 `__init__.py` 中新增 |
| 同目录 ≥2 个测试文件都需要同一个常量或数据结构 | ✅ 必须封装 |
| 仅单个测试文件使用的逻辑 | ❌ 不封装，保留在测试文件内 |
| `keywords_api.py` 中已有的通用函数 | ❌ 不封装，直接调用已有函数 |

## 封装示例

```python
# test_case/cgmanager_module/__init__.py
# -*- coding: utf-8 -*-
"""
cgmanager_module 目录公共函数
同目录下所有测试文件通过 from .__init__ import * 调用
"""
from entry import *


# 期望结果变量（多个测试文件共用）
expected_success = {
    'execution_ret': {...},
    'video_ret': {...},
    'audio_ret': {...}
}

expected_alloc_failed = {
    'execution_ret': {'code': -1, ...}
}


def check_pull_stream_result(thread_ret: list, expected_results: dict) -> None:
    """验证拉流结果（cgmanager_module 目录下多个测试文件共用）"""
    check_ret, check_info = validate_thread_ret(thread_ret, expected_results)
    assert check_ret, assert_validate_log(check_ret, check_info)


def assert_api_response(status_code: int, response: dict) -> None:
    """标准 API 响应断言（cgmanager_module 目录下多个测试文件共用）"""
    assert status_code == 200, f"HTTP 状态码异常: {status_code}"
    assert int(response.get("code", -1)) == 0, f"业务码异常: {response.get('code')}"
```

## 测试文件中的调用方式

```python
# test_case/cgmanager_module/test_cgmanager_xxx.py
from entry import *
from .__init__ import *   # 自动导入 __init__.py 中的所有公共函数和变量


class Test:
    @TestCaseParam
    @pytest.mark.P1
    @pytest.mark.CGManagerModule
    @pytest.mark.CGApi
    @pytest.mark.cg_1
    @pytest.mark.parametrize("cg_param", TEST_CGMANAGER_SCENES)
    def test_xxx(self, cg_param, step_logger):
        self.cg_param = cg_param

        step_logger("验证拉流结果")
        # ✅ 直接引用 __init__.py 中的预定义变量
        check_pull_stream_result(thread_ret, expected_success)

        # ❌ 错误：在测试文件中重复定义 expected_success
        # expected_success = {'execution_ret': {...}, ...}
```

## 封装函数命名规范

| 类型 | 命名模式 | 示例 |
|------|----------|------|
| 断言/验证函数 | `check_<场景>_<内容>` | `check_pull_stream_result` |
| 数据提取函数 | `get_<对象>_<内容>` | `get_xhd_instance_list` |
| 初始化函数 | `init_<对象>_<内容>` | `init_proxy_api` |
| 断言响应函数 | `assert_<对象>_<内容>` | `assert_api_response` |
