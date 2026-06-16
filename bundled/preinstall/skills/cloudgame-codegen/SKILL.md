---
name: "cloudgame-codegen"
description: "CloudGame autotest code generator - 根据CloudGame项目的测试设计文档生成pytest自动化测试用例代码。当用户提供测试设计规范（通过UUID或直接描述）并需要将其转换为遵循项目约定的Python pytest测试用例时使用。支持多个模块（xhd、proxy、master、resource_svr、cgmanager、wx_video），并具有特定的命名规则、目录结构和测试框架。"
description_zh: "AI 驱动测试用例生成与管理工作流，集成 TestBuddy 脑图可视化和 TCase 自动化测试代码生成能力"
version: "1.0.0"
source: codebuddy
source_plugin: "tsmart"
---

# CloudGame Autotest Code Generator

将测试设计文档转换为符合 CloudGame 自动化测试框架规范的 pytest 测试代码。

## 🚫 防幻觉铁律（最高优先级，贯穿全流程）

> **违反以下任一铁律，生成的代码视为无效，必须重新生成。**

1. **API 方法防幻觉**：调用的**每一个 API 方法**都必须能在 [knowledge/api_methods.md](knowledge/api_methods.md) 中找到对应条目，包括方法名、必填参数名称和类型、返回值字段。**不存在的方法禁止调用，不存在的字段禁止访问。**

2. **场景变量防幻觉**：访问场景变量的**每一个字段名**（如 `param.XHD_INFO['xxx']`、`cg_param.CGMANAGER_INFO['xxx']`）都必须能在 [knowledge/scene_variables.md](knowledge/scene_variables.md) 中找到对应条目。**禁止编造不存在的字段。**

3. **API 模块防幻觉**：**测试步骤文字描述是唯一判断依据**，步骤写"通过 Proxy"就用 `ProxyAPI`，写"通过 RS"就用 `RSAPI`，写"通过 CGManager"就用 `CGAPI`。URL 路径、方法名、个人理解均不能作为判断依据。详见 [references/module_config.md](references/module_config.md)。

---

## 工作流总览

```
输入 → 阶段1：解析 → 阶段2：路由 → 阶段3：生成 → 阶段4：检查 → 输出
```

| 阶段 | 文档 | 核心任务 |
|------|------|---------|
| 阶段1：解析输入 | [workflows/phase1_parse.md](workflows/phase1_parse.md) | 提取模块名、类型、场景标签、用例等级 |
| 阶段2：路由决策 | [workflows/phase2_route.md](workflows/phase2_route.md) | 确定模板 A/B/C，计算 N 值 |
| 阶段3：生成代码 | [workflows/phase3_generate.md](workflows/phase3_generate.md) | 填充装饰器、测试逻辑、teardown |
| 阶段4：生成前检查 | [workflows/phase4_checklist.md](workflows/phase4_checklist.md) | 逐项核查，任一失败则回退修正 |

---

## 模块配置速查

| 模块 | 类型标签 | 模块专属场景变量 | 实例标签 | 模块参数名 |
|------|----------|-----------------|----------|------------|
| xhd | `FullLink` / `XHDModule` | `TEST_SCENES` | `ins_N` | `param` |
| proxy | `ProxyModule` | `TEST_PROXY_SCENES` | `proxy_N` | `proxy_param` |
| master | `MasterModule` | `TEST_MASTER_SCENES` | `master_N` | `master_param` |
| resource_svr | `ResourceSvrModule` | `TEST_RESOURCE_SCENES` | `rs_N` | `rs_param` |
| cgmanager | `CGManagerModule` | `TEST_CGMANAGER_SCENES` | `cg_N` | `cg_param` |
| wx_video | `CGManagerModule` | `TEST_WV_VIDEO_SCENES` | — | `wv_param` |

---

## 文档索引

### 防幻觉知识库（只读参考数据）

| 需要什么 | 查哪里 |
|----------|--------|
| API 方法速查（防幻觉清单） | [knowledge/api_methods.md](knowledge/api_methods.md) |
| 场景变量字段速查（防幻觉） | [knowledge/scene_variables.md](knowledge/scene_variables.md) |

### 生成规则参考文档

| 需要什么 | 查哪里 |
|----------|--------|
| 模块配置、场景标签、IP提取、API初始化、代码片段 | [references/module_config.md](references/module_config.md) |
| 环境标签生成规则、N值计算 | [references/environment_tags.md](references/environment_tags.md) |
| 通用代码模式（统计比较、响应验证等） | [references/code_patterns.md](references/code_patterns.md) |
| `__init__.py` 封装规范 | [references/init_conventions.md](references/init_conventions.md) |
| 模板完整代码 | [references/test_template.py](references/test_template.py) |