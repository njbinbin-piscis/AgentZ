# 阶段4：生成前检查清单

> **使用说明**：生成代码后，必须逐项确认以下所有检查项。任何一项失败则回到对应步骤修正，不得跳过。

## 快速检查流程

```
1. 基础结构检查（#1-3）
2. 参数化检查（#4）
3. 装饰器检查（#5-7）
4. 环境标签检查（#8-9）
5. 代码逻辑检查（#10-14）
6. API 合规检查（#15-20）
```

## 基础结构检查

| # | 检查项 | 正确示例 | 常见错误 |
|---|--------|----------|----------|
| 1 | 类名固定为 `Test` | `class Test():` | ~~`class TestProxy():`~~ |
| 2 | `@TestCaseParam` 是第一个装饰器 | `@TestCaseParam` 在最上方 | ~~`@pytest.mark.P1` 在第一位~~ |
| 3 | 模板 C 无 `param` 参数 | `def test_xxx(self, proxy_param, step_logger)` | ~~`def test_xxx(self, param, step_logger)`~~ |

## 参数化检查

| # | 检查项 | 正确示例 | 常见错误 |
|---|--------|----------|----------|
| 4 | 模板A用 `TEST_SCENES`；模板B双参数化；模板C只用 `TEST_*_SCENES` | 见模板代码 | ~~模板B只有一个parametrize~~、~~模板C用了TEST_SCENES~~ |

## 装饰器检查

| # | 检查项 | 正确示例 | 常见错误 |
|---|--------|----------|----------|
| 5 | 断言含错误描述 | `assert code == 200, f"状态码异常: {code}"` | ~~`assert code == 200`~~ |
| 6 | 场景标签在所选类型对应的可用列表中，类型标签只有一个 | 见 knowledge/module_config.md | ~~同时出现 `FullLink` 和 `XHDModule`~~ |
| 7 | 装饰器顺序：TestCaseParam → 优先级 → **类型** → **场景** → 实例 → parametrize | 见 assets/test_template.py | ~~场景标签在类型标签前~~ |

## 环境标签检查

| # | 检查项 | 正确示例 | 常见错误 |
|---|--------|----------|----------|
| 8 | 环境标签完整性：A含`ins_N`，B含`<模块>_N`+`ins_N`，C含`<模块>_N` | 见 knowledge/environment_tags.md | ~~模板B缺少ins_N~~ |
| 9 | N值根据测试步骤动态计算，范围1-8，默认N=1 | 步骤含"3个实例"→`ins_3` | ~~N值固定为1未动态计算~~ |

## 代码逻辑检查

| # | 检查项 | 正确示例 | 常见错误 |
|---|--------|----------|----------|
| 10 | 环境恢复逻辑只在 `teardown_method` 中，用例函数内只设置 flag | `self.stop_flag = True`；teardown 中执行清理 | ~~用例函数末尾直接调用 `stop_game()`~~ |
| 11 | `teardown_method` 优先复用 `self.xxx` 已有变量，没有才重新定义 | 直接用 `self.CGAPI.xxx()` | ~~测试函数已有 `self.CGAPI` 但 teardown 中又重新初始化~~ |
| 12 | 关键字函数优先从 `keywords_api.py` 获取 | `validate_thread_ret(...)` | ~~自行实现已有函数~~ |
| 13 | 同类场景公共函数封装在目录 `__init__.py`；测试函数内没有的变量在 `__init__.py` 中定义后引用 | `from .__init__ import *` | ~~每个文件各自定义相同逻辑~~；~~`__init__.py` 没有该变量却直接使用~~ |
| 14 | 涉及新增/减少/不变的数值验证，必须使用"基准值差值法"：触发前记录基准值，触发后对比差值，禁止直接断言绝对值或用文件总行数作锚点 | 触发前 `baseline = grep \| wc -l`，触发后 `current > baseline` | ~~`cat log \| grep keyword`直接断言~~；~~`wc -l`总行数作锚点后`tail -n +N`~~；~~`assert resp["count"] == 5`固定值~~ |

## API 合规检查

| # | 检查项 | 正确示例 | 常见错误 |
|---|--------|----------|----------|
| 15 | 所有 API 调用在 `knowledge/api_methods.md` 中有据可查 | 见 api_methods.md 速查表 | ~~使用 `alloc()`、`assign()` 等不存在的方法~~ |
| 16 | 拉流前检查 alloc/stop 是否重复 | `alloc_op=False` / `stop_op=False` | ~~已调用 alloc 后又调用默认 `run_main_wait()`~~ |
| 17 | 删除 body 字段用 `"del_key"` | `stop_game(session_id="del_key")` | ~~手动构造缺字段的 dict~~ |
| 18 | 所有 API/关键字返回值字段名必须来自 `knowledge/api_methods.md` 文档，禁止凭直觉猜测字段名 | `cmd_result[0].get("out_msg", "")` | ~~`cmd_result[0].get("stdout", "")`~~、~~`.get("output", "")`~~（未在文档中列出的字段） |
| 19 | 访问场景变量的字段名（如 `param.XHD_INFO['xxx']`、`cg_param.CGMANAGER_INFO['xxx']`）必须来自 `knowledge/scene_variables.md`，禁止编造不存在的字段 | `param.CGMANAGER_INFO['lan']`（文档中存在） | ~~`param.CGMANAGER_INFO['lan_ip']`~~（该字段在 `TEST_CGMANAGER_SCENES` 中，不在 `TEST_SCENES` 中） |
| 20 | **API 调用模块必须与测试步骤描述严格一致，步骤文字是唯一判断依据，URL路径/方法名/个人理解均不能作为依据。模板A用 `self.param.XxxAPI`，模板B/C用 `self.XxxAPI`（从对应 `xxx_param` 初始化）。步骤写"通过 Proxy"就用 `ProxyAPI`，写"通过 RS"就用 `RSAPI`，写"通过 CGManager"就用 `CGAPI`，以此类推。**详见 `references/module_config.md` 第四节** | 步骤写"通过 proxy 触发挂载" → 模板A：`self.param.ProxyAPI.instance_level_mounting(...)`；模板B/C：`self.ProxyAPI.instance_level_mounting(...)` | ~~步骤写"通过 proxy 触发"但代码用了 `self.param.RSAPI.mount_game_image(...)` 或 `self.RSAPI.mount_game_image(...)`~~（`resource_svr_api.py` 中 URL 含 `proxy` 不代表是 Proxy 模块的 API） |

> ✅ **全部通过后**，代码可以输出给用户。
> ❌ **任何一项失败**，回到对应步骤修正后重新检查。
