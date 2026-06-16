# 阶段3：生成代码

## 装饰器顺序（从上到下，不可变）

1. `@TestCaseParam` — 固定第一装饰器
2. `@pytest.mark.<优先级>` — P0/P1/P2/P3
3. `@pytest.mark.<类型标签>` — 只能选一个（互斥）
4. `@pytest.mark.<场景标签>` — 必须属于所选类型对应的可用列表
5. `@pytest.mark.<实例标签>` — 仅模板 A/B（N值动态计算）
6. `@pytest.mark.parametrize` — 模板A/B均有；模板C只有模块专属

**示例装饰器组合：**

```
模板A：TestCaseParam → P1 → FullLink → Basic → ins_N → parametrize(TEST_SCENES)
模板B：TestCaseParam → P1 → <类型> → <场景> → <模块>_N → ins_N → parametrize(TEST_*_SCENES) → parametrize(TEST_SCENES)
模板C：TestCaseParam → P1 → <类型> → <场景> → <模块>_N → parametrize(TEST_*_SCENES)
```

## 测试逻辑填充规则

### 基础规则

- 固定 import：`from entry import *` + `from .__init__ import *`
- 每步用 `step_logger("描述")` 包裹，**描述内容禁止添加任何序号前缀**（包括 `Step N:`、`1、`、`第一步` 等）
- 断言必须含描述性错误信息：`assert code == 200, f"状态码异常: {code}"`
- docstring 必须含 `@author`、`@update_person`、`@description`

### teardown_method 规则

- **环境恢复逻辑只写在 `teardown_method` 中**，用例函数内只设置 flag
- **变量复用原则**：优先使用测试函数内已通过 `self.xxx = ...` 赋值的变量（如 `self.session_id`、`self.CGAPI`），只有当测试函数中确实未定义时，才在 `teardown_method` 中重新定义

```python
def teardown_method(self, method):
    # ✅ 正确：直接复用测试函数中已赋值的 self.CGAPI
    if hasattr(self, "cleanup_flag") and self.cleanup_flag:
        self.CGAPI.set_instance_status(...)

    # ❌ 错误：测试函数已有 self.CGAPI，teardown 中又重新初始化
    # cg_ip = self.cg_param["CGMANAGER_INFO"]["wan_ip"]
    # CGAPI = CG_Manager_Request(cg_ip)
```

### 关键字函数规则

- **优先从 `keywords_api.py` 获取**，不重复实现已有逻辑
- 完整函数列表见 [knowledge/api_methods.md](../knowledge/api_methods.md)

### API 方法查阅强制规则（生成代码前必须执行）

> 🚫 **铁律**：调用任何 API 方法前，**必须先查阅 [knowledge/api_methods.md](../knowledge/api_methods.md)**，确认以下三项，不得凭记忆或猜测：
>
> 1. **方法名是否存在**：方法名必须在文档中有对应条目，不存在的方法禁止调用
> 2. **必填参数名称和类型**：严格按照文档中的参数名传参，不得自行推断参数名
> 3. **返回值字段名**：只能使用文档中明确列出的字段，禁止猜测字段名

**查阅流程（每次调用 API 前必须执行）：**

```
① 确定调用模块（ProxyAPI / RSAPI / CGAPI / MasterAPI）
    ↓
② 在 api_methods.md 中找到对应方法条目
    ↓
③ 核对必填参数名称和类型（如 instance_ids 是 list，不是 instance_id str）
    ↓
④ 核对返回值字段（如 resp["body"]["task_id"]，不是 resp["data"]["task_id"]）
    ↓
⑤ 生成代码
```

### API 调用模块对应规则

> 🚫 **铁律**：**测试步骤文字描述是唯一判断依据**，URL 路径、方法名、个人理解均不能作为判断依据。
>
> 详细规则和典型混淆场景见 **[references/module_config.md](../references/module_config.md) 第四节**。

### 公共函数封装规则

- **同类场景公共函数封装在目录 `__init__.py`**，通过 `from .__init__ import *` 调用
- 详细封装规范见 [references/init_conventions.md](../references/init_conventions.md)

### 删除请求字段

- 删除 body 字段用 `"del_key"`，框架会在发送前自动移除
- 详见 [references/module_config.md](../references/module_config.md) 第七节

### 统计比较规则（新增/减少/不变场景）

当测试步骤需要验证某个数值**新增、减少或不变**时，**必须使用"基准值差值法"**，禁止读取全量数据后直接断言绝对值。

**核心思路**：触发前记录基准值 → 执行操作 → 触发后对比差值

详细代码模板见 [references/code_patterns.md](../references/code_patterns.md)。

**适用场景速查：**

| 场景描述 | 断言方式 |
|----------|---------|
| 日志中某字段新增 | `current > baseline` |
| 日志中某字段不应出现 | `current == baseline` |
| 命令行输出数字增加/减少 | `current > baseline` / `current < baseline` |
| 字典/JSON 数值字段变化 | `current > baseline` / `current == baseline` |
| 列表长度变化 | `current > baseline` |

## 平台选择

默认 `XF_AOSP13_8550`。

| 平台 | 环境 | 说明 |
|------|------|------|
| `XF_AOSP10_865` | 板卡 | 骁龙 865 |
| `XF_AOSP11_VAST` | K8s | VAST 集群 |
| `XF_AOSP11_TIANJI` | K8s | 天机集群 |
| `XF_AOSP13_8550` | 板卡（默认） | 骁龙 8550 |
| `YYB_ARM` | ARM | 应用宝 ARM 环境 |

多平台用 `+` 组合：`XF_AOSP10_865 + XF_AOSP11_VAST + XF_AOSP13_8550`
