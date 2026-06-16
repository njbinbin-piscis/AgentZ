# CloudGame 环境标签生成规则

> **本文档是环境标签规则的唯一权威来源。** [SKILL.md](../SKILL.md) 和 [workflows/phase4_checklist.md](../workflows/phase4_checklist.md) 中的环境标签规则均以本文档为准。

## 标签类型说明

| 标签类型 | 格式 | 应用场景 | 示例 |
|----------|------|----------|------|
| XHD 实例标签 | `ins_N` | 模板A（XHDModule/FullLink）和模板B | `@pytest.mark.ins_1` |
| 模块实例标签 | `<模块>_N` | 模板B（非XHD+需要实例）和模板C | `@pytest.mark.proxy_1` |

> ⚠️ **标签定义顺序（强制）**：`@TestCaseParam` → 优先级标签 → **类型标签** → **场景标签** → 实例标签 → `parametrize`

**各模块类型标签与场景标签对照表：**

| 模块 | 类型标签（唯一） | 场景标签（可选其一） |
|------|----------------|--------------------|
| proxy | `ProxyModule` | `ProxyApi` / `ProxyConfig` / `ProxyReliability` / `ProxyFunction` / `ProxyImage` |
| master | `MasterModule` | `MasterApi` / `MasterConfig` / `MasterReliability` / `MasterHeartbeat` / `MasterFunction` |
| resource_svr | `ResourceSvrModule` | `ResourceSvrApi` / `ResourceSvrConfig` / `ResourceSvrReliability` / `ResourceSvrFunction` |
| cgmanager | `CGManagerModule` | `CGApi` / `CGConfig` / `CGReliability` / `CGFunction` / `CGWVideo` / `WVVideo` |
| xhd/fulllink | `XHDModule` / `FullLink` | `Basic` / `XHDPullStream` / `XHDConfig` / `XHDReliability` / `XHDInstanceOp` |

**各模块实例标签前缀：**

| 模块 | 标签前缀 | 示例 |
|------|----------|------|
| proxy | `proxy_` | `proxy_1`, `proxy_3` |
| master | `master_` | `master_1`, `master_2` |
| resource_svr | `rs_` | `rs_1`, `rs_4` |
| cgmanager | `cg_` | `cg_1`, `cg_2` |

## 模板应用规则

| 模板类型 | 必须包含的标签 | 约束 |
|----------|---------------|------|
| **模板A**（XHDModule/FullLink） | `ins_N` | 必须有XHD实例标签 |
| **模板B**（非XHD+需要实例） | `<模块>_N` + `ins_N` | 必须同时包含，两个N值各自根据测试步骤独立计算，无需一致 |
| **模板C**（非XHD+纯接口） | `<模块>_N` | 只有模块实例标签，无 `ins_N` |

## N 值动态计算规则

### 计算优先级（从高到低）

| 优先级 | 触发条件 | 关键词 | N 值 |
|--------|----------|--------|------|
| 1 | 步骤中有明确数量描述 | "N个实例"、"N台服务器"、"N个XHD"、"并发N路拉流" | 描述中的数字 |
| 2 | 步骤涉及并发操作 | "并发"、"同时"、"并行"、"多路"、"批量" | 并发规模（通常2-4） |
| 3 | 步骤需要验证多实例交互 | "交互"、"同步"、"数据一致性"、"集群"、"多节点" | 验证所需最小实例数（通常2-3） |
| 4 | 无上述描述 | — | 1（默认值） |

**N 值范围限制**：有效范围 1-8，超出时使用最接近的边界值（N<1→1，N>8→8）。

### 常见场景示例

| 场景描述 | N 值 | 标签（模板A） | 标签（模板B，以proxy为例） |
|----------|------|--------------|--------------------------|
| 验证单个XHD实例的基础拉流功能 | 1 | `ins_1` | `proxy_1` + `ins_1` |
| 同时启动3个实例进行并发拉流 | 3 | `ins_3` | `proxy_3` + `ins_3` |
| 验证2个实例之间的数据同步 | 2 | `ins_2` | `proxy_2` + `ins_2` |
| 批量配置5台服务器的参数（模板C） | 5 | — | `master_5`（master模块） |

## 代码示例

### 模板A：单实例拉流测试

```python
@pytest.mark.FullLink
@pytest.mark.Basic
@pytest.mark.ins_1  # 默认N=1
@pytest.mark.parametrize("param", filter_testcases(TEST_SCENES, ENV_INFO=XF_AOSP13_8550))
def test_xhd_basic_pull(self, param, step_logger):
    pass
```

### 模板A：并发拉流测试（3个实例）

```python
@pytest.mark.FullLink
@pytest.mark.Basic
@pytest.mark.ins_3  # 根据并发数量确定N=3
@pytest.mark.parametrize("param", filter_testcases(TEST_SCENES, ENV_INFO=XF_AOSP13_8550))
def test_xhd_concurrent_pull(self, param, step_logger):
    pass
```

### 模板B：代理+XHD实例测试（N值各自独立）

```python
@pytest.mark.ProxyModule
@pytest.mark.ProxyApi
@pytest.mark.proxy_2  # 模块实例标签（根据proxy侧步骤计算）
@pytest.mark.ins_1    # XHD实例标签（根据XHD侧步骤独立计算，无需与proxy_N一致）
@pytest.mark.parametrize("proxy_param", TEST_PROXY_SCENES)
@pytest.mark.parametrize("param", filter_testcases(TEST_SCENES, ENV_INFO=XF_AOSP13_8550))
def test_proxy_multi_instance(self, proxy_param, param, step_logger):
    pass
```

### 模板C：批量API调用测试

```python
@pytest.mark.MasterModule
@pytest.mark.MasterApi
@pytest.mark.master_4  # 批量API调用，需要4个目标
@pytest.mark.parametrize("master_param", TEST_MASTER_SCENES)
def test_master_batch_api(self, master_param, step_logger):
    pass
```

## 常见错误

```python
# ❌ 错误：场景标签在类型标签前面
@pytest.mark.ProxyApi     # 场景标签不能在类型标签前
@pytest.mark.ProxyModule
@pytest.mark.proxy_1

# ✅ 正确：先类型标签，再场景标签
@pytest.mark.ProxyModule
@pytest.mark.ProxyApi
@pytest.mark.proxy_1

# ❌ 错误：模板B缺少ins_N标签
@pytest.mark.proxy_3  # 只有模块标签，缺少XHD标签

# ✅ 正确：模板B的两个N值各自独立，根据各自步骤计算，无需一致
@pytest.mark.proxy_2  # proxy侧需要2个实例
@pytest.mark.ins_1    # XHD侧只需要1个实例，与proxy_N无需一致

# ❌ 错误：N值超出范围
@pytest.mark.ins_10   # N值超出范围，应该改为ins_8

# ❌ 错误：N值未动态计算（步骤明确需要3个实例，但N值固定为1）
@pytest.mark.ins_1
```

通过遵循这些规则，可以确保生成的测试用例包含完整且正确的环境配置信息，提高测试的准确性和覆盖率。