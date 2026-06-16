# 代码模式参考

> 本文档收录测试代码中的**通用代码模式**，供生成代码时直接参考复用。

## 统计比较模式（基准值差值法）

### 适用场景

当测试步骤需要验证某个数值**新增、减少或不变**时，必须使用此模式。

| 场景描述 | 数据来源示例 | 断言方式 |
|----------|-------------|---------|
| 日志中某字段新增 | `grep 'keyword' log \| wc -l` | `current > baseline` |
| 日志中某字段不应出现 | `grep 'keyword' log \| wc -l` | `current == baseline` |
| 命令行输出数字增加 | `wc -l`、`ps aux \| wc -l` | `current > baseline` |
| 命令行输出数字减少 | `ls \| wc -l` | `current < baseline` |
| 字典/JSON 数值字段变化 | `resp["count"]`、`resp["data"]["total"]` | `current > baseline` / `current == baseline` |
| 列表长度变化 | `len(resp["list"])` | `current > baseline` |

### 模板一：日志字段统计（通过 instance_id 过滤）

```python
# ① 记录基准值（触发前）
step_logger("记录触发前该实例日志字段的基准数量")
baseline_cmd = [{"dst_ip": cg_lan_ip, "cmd": f"grep '{instance_id}' /path/to/log | grep 'KEYWORD' | wc -l"}]
_, baseline_result = remote_run_cmd_batch(baseline_cmd)
baseline_out = baseline_result[0].get("out_msg", "0").strip() if baseline_result else "0"
log_baseline = int(baseline_out) if baseline_out.isdigit() else 0
logger.info(f"触发前基准数量：{log_baseline}，实例：{instance_id}")

# ② 执行触发操作
step_logger("执行触发操作")
# ... 触发代码 ...

# ③ 查询当前值并断言（触发后）
step_logger("检查日志字段是否新增/不变")
log_cmd = f"grep '{instance_id}' /path/to/log | grep 'KEYWORD' | wc -l"
cmd_data = [{"dst_ip": cg_lan_ip, "cmd": log_cmd}]
ret, cmd_result = remote_run_cmd_batch(cmd_data)
out = cmd_result[0].get("out_msg", "0").strip() if ret and cmd_result else "0"
current_count = int(out) if out.isdigit() else 0

# 新增场景
assert current_count > log_baseline, \
    f"日志中未新增 KEYWORD 字段，实例：{instance_id}，触发前：{log_baseline}，触发后：{current_count}"
# 不变场景（不应出现）
# assert current_count == log_baseline, \
#     f"日志中出现了不应有的 KEYWORD 字段，实例：{instance_id}，触发前：{log_baseline}，触发后：{current_count}"
```

### 模板二：接口返回数值字段比较

```python
# ① 记录基准值（触发前）
step_logger("记录触发前的基准数值")
code, resp = self.RSAPI.some_query_api(...)
assert code == 200, f"查询接口异常: {code}"
baseline_value = resp.get("data", {}).get("count", 0)
logger.info(f"触发前基准值：{baseline_value}")

# ② 执行触发操作
step_logger("执行触发操作")
# ... 触发代码 ...

# ③ 查询当前值并断言（触发后）
step_logger("检查数值是否新增/减少/不变")
code, resp = self.RSAPI.some_query_api(...)
assert code == 200, f"查询接口异常: {code}"
current_value = resp.get("data", {}).get("count", 0)

# 新增场景
assert current_value > baseline_value, \
    f"数值未增加，触发前：{baseline_value}，触发后：{current_value}"
# 减少场景
# assert current_value < baseline_value, \
#     f"数值未减少，触发前：{baseline_value}，触发后：{current_value}"
# 不变场景
# assert current_value == baseline_value, \
#     f"数值发生了意外变化，触发前：{baseline_value}，触发后：{current_value}"
```

### 禁止的错误写法

```python
# ❌ 错误：直接读全量日志断言绝对值，历史遗留记录会干扰结果
log_cmd = f"cat /path/to/log | grep '{instance_id}' | grep 'KEYWORD'"
assert out.strip(), "未找到 KEYWORD"  # 历史记录也会匹配！

# ❌ 错误：用文件总行数作锚点，无法过滤其他实例的日志
wc_cmd = [{"dst_ip": ip, "cmd": "wc -l < /path/to/log"}]
# tail -n +行数 后仍包含其他实例的记录

# ❌ 错误：直接断言接口返回值等于某个固定数字，忽略初始状态
assert resp["count"] == 5, "数量不对"  # 初始状态未知时不可靠
```

## 响应验证标准模式

```python
# 标准 HTTP + 业务码双重验证
assert status_code == 200, f"HTTP 状态码异常: {status_code}"
assert int(response['code']) == 0, f"业务码异常: {response['code']}"
```

## 平台分支模式

```python
if self.param.ENV_INFO in XF_AOSP11_VAST + XF_AOSP11_TIANJI:
    ret, out, err = kubectl_exec_cmd(action="exec_xhd", pod_id=f"xhd-{test_xhd}", cmd=cmd)
else:
    cmd_data = [{"dst_ip": self.param.CGMANAGER_INFO["lan"],
                 "cmd": f"cd /android/cg_manager;./adb_shell.sh {test_xhd} '{cmd}'"}]
    ret, cmd_result = remote_run_cmd_batch(cmd_data)
```

## 多值参数化模式

```python
@pytest.mark.parametrize("touch_info", [
    pytest.param(mult_touch_info, id="mult_touch"),
    pytest.param(mult_add_touch_info, id="mult_add_touch"),
])
def test_example(self, param, step_logger, touch_info):
    pass
```
