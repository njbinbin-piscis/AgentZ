# 阶段2：路由决策

## 第一步：确定测试类型（互斥，只能选一个）

```
需求标题含 XHD/xhd？
├── 是 → 类型 = XHDModule（第一优先）
└── 否 → 步骤涉及拉流操作？
    ├── 是（实例拉流功能验证）→ XHDModule（第一优先）
    ├── 是（基础拉流验证）→ FullLink
    └── 否 → 类型 = 模块对应类型（ProxyModule / MasterModule / ...）
```

> ⚠️ **类型互斥原则**：每个测试函数只能有**一个**类型标签，不可同时出现多个。

## 第二步：根据类型选择模板

```
类型 = FullLink 或 XHDModule？
├── 是 → 模板 A
└── 否 → 步骤涉及实例操作？
    ├── 是（拉流/启停游戏/实例上下线）→ 模板 B
    └── 否（纯 API 调用/查询配置）→ 模板 C
```

## 第三步：动态计算实例数量 N

| 优先级 | 触发条件 | 关键词 | N 值 |
|--------|----------|--------|------|
| 1 | 步骤中有明确数量描述 | "N个实例"、"N台服务器"、"并发N路拉流" | 描述中的数字 |
| 2 | 步骤涉及并发操作 | "并发"、"同时"、"并行"、"多路"、"批量" | 并发规模（通常2-4） |
| 3 | 步骤需要验证多实例交互 | "交互"、"同步"、"集群"、"多节点" | 验证所需最小实例数（通常2-3） |
| 4 | 无上述描述 | — | 1（默认值） |

**N 值范围**：1-8，超出时使用最接近的边界值。

## 三模板完整对比

| 维度 | 模板 A | 模板 B | 模板 C |
|------|--------|--------|--------|
| **适用场景** | FullLink / XHDModule | 非XHD + 需实例操作 | 非XHD + 纯接口 |
| **parametrize** | `TEST_SCENES` | `TEST_*_SCENES` + `TEST_SCENES`（双参数化） | 仅 `TEST_*_SCENES`（单参数化） |
| **函数参数** | `self, param, step_logger` | `self, <模块参数名>, param, step_logger` | `self, <模块参数名>, step_logger` |
| **实例标签** | `ins_N` | `<模块>_N` + `ins_N` | `<模块>_N` |

> ⚠️ **TEST_SCENES 专属规则**：`TEST_SCENES` 是 XHD 模块专属变量，**只有模板 A/B 才使用**。模板 C 不使用 `TEST_SCENES`，也不含 `param` 参数。

## 模板 B 双参数化结构

```python
@pytest.mark.<类型标签>
@pytest.mark.<场景标签>
@pytest.mark.<模块>_N
@pytest.mark.ins_N
@pytest.mark.parametrize("<模块>_param", TEST_<模块大写>_SCENES)
@pytest.mark.parametrize("param", filter_testcases(TEST_SCENES, ENV_INFO=XF_AOSP13_8550))
def test_xxx(self, <模块>_param, param, step_logger):
    pass
```

## 模板 C 单参数化结构

```python
@pytest.mark.<类型标签>
@pytest.mark.<场景标签>
@pytest.mark.<模块>_N
@pytest.mark.parametrize("<模块>_param", TEST_<模块大写>_SCENES)
def test_xxx(self, <模块>_param, step_logger):
    pass
```

各模块具体示例见 [knowledge/module_config.md](../knowledge/module_config.md)。
