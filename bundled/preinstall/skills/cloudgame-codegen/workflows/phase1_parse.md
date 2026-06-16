# 阶段1：解析输入

## 输入模式

### Mode 1：UUID（TestBuddy 脑图节点）

1. 通过 `load_session.py` 获取 `select_node`
2. 根据节点类型提取测试设计信息

```
select_node.type = STORY/BUG → 先执行需求分析再生成用例
select_node.type = FEATURE/TEST_POINT → 直接从节点提取测试设计字段
```

**节点字段映射：**

| 节点属性 | 映射到 |
|----------|--------|
| `name` | 场景 / 用例描述 |
| `instance.priority` | 用例等级（P0~P3） |
| `instance.test_attr` | 测试属性（`<类型>-<场景>`） |
| `instance.precondition` | 前置条件 |
| `instance.steps` | 步骤描述 |
| `instance.cleanup` | 环境恢复 |

### Mode 2：直接描述

| 字段 | 说明 | 示例 |
|------|------|------|
| 场景 | `【模块】场景内容` | `【cgmanager】挂载类型校验` |
| 用例描述 | 用例功能说明 | `验证轻量挂载配置触发` |
| 用例等级 | P0/P1/P2/P3（默认 P1） | `P1` |
| 测试属性 | `<类型>-<场景>` | `CGManagerModule-CGApi` |
| 前置条件 | 测试前提 | `实例已启动` |
| 步骤描述 | 测试步骤 | `调用接口并验证返回` |
| 环境恢复 | 清理步骤 | `恢复实例状态` |

## 提取规则

- **模块名** = 场景字段 `【】` 内的内容 → 用于查模块配置表
- **类型** = 测试属性 `-` 前的部分 → 用于路由决策
- **场景标签** = 测试属性 `-` 后的部分 → 用于 pytest marker

> **如果模块名无法确定，必须询问用户，不得猜测。**

## 文件命名规则

全部翻译为英文、下划线连接：

| 元素 | 来源 | 示例 |
|------|------|------|
| 场景目录 | 测试属性-场景 | `Basic` → `basic/` |
| 文件名 | `test_<模块>_<场景内容>` | `test_cgmanager_mount_type_validation.py` |
| 测试函数 | `test_<用例描述>` | `test_validate_mount_type` |
| 测试类名 | 固定 `Test` | `class Test():` |

目录结构：`test_case/<模块>_module/test_<模块>_<场景内容>.py`
