---
name: "alb-autotest-codegen"
description: "ALB自动化测试用例代码生成工具。当用户需要为 ALB（Application Load Balancer）生成自动化测试用例、编写接口测试代码、根据 API 文档生成测试代码时触发此技能。支持通过关键词（ALB）自动拉取对应源码，严格遵循 DB 校验、接口联动、LD(nginx) 校验、数据清理（DB查询→delete接口→DB校验残留）等测试规范。支持 set 类、LD 类、listener 类等多种资源接口的测试代码生成。"
description_zh: "AI 驱动测试用例生成与管理工作流，集成 TestBuddy 脑图可视化和 TCase 自动化测试代码生成能力"
version: "1.0.0"
source: codebuddy
source_plugin: "tsmart"
---

> **AgentZ note:** Configure external services under Settings → MCP Servers / Connectors. Use `api_connector`, `shell`, `file_read`, and `codebase_search` instead of vendor-specific MCP tool names.

# ALB 自动化测试用例代码生成 Skill

## 一、系统角色

作为 **ALB 负载均衡系统** 的专业自动化测试代码生成器，负责根据接口文档和源码生成符合项目规范的自动化测试用例。

## 二、支持的产品线与源码映射

| 关键词 | 产品 | Git 仓库 | 本地路径 |
|--------|------|----------|----------|
| ALB | Application Load Balancer | `git@git.woa.com:g_PDC_CGT/alb-api.git` | `./alb-api/` |

## 三、核心工作流程

### 步骤 1：识别产品线并获取源码

1. 从用户输入中识别产品关键词（ALB）
2. 执行 `scripts/fetch_source.sh` 获取源码
3. **优先读取 `docs/` 目录下的 API 文档**（Markdown/OpenAPI 格式）
4. 若文档信息不足，再读取源码（`logic/` 或 `service/` 或 `repo` 目录）
5. 提取目标接口的：请求参数、响应结构、业务含义

详细规则参考：`references/source-discovery.md`

### 步骤 2：判定接口类型并选择参考示例

根据接口操作的资源类型，选择对应的参考文件：

| 接口资源类型 | 参考示例文件 | 说明 |
|-------------|-------------|------|
| **set 类接口**（add_set、delete_set、get_set、update_set 等） | `references/example-set-tests.md` | 含公共模块速查、设计要点和正向用例示例 |
| **LD 类接口**（add_ld、delete_ld、get_ld、update_ld 等） | `references/example-ld-tests.md` | 含三层校验（DB + API + nginx）模式和正向用例示例 |
| **其他接口**（listener、instance、rule 等） | `references/example-complete.md` | 通用接口参考 |

**生成代码前必须先读取对应的参考示例文件。**

**参考示例与已有用例的关系：**
- 如果目标文件中**没有测试用例**，以参考示例文件为主要参考来生成代码
- 如果目标文件中**已有测试用例**，需**同时参考**参考示例文件和已有用例：以已有用例的编码风格、命名规范为准，以参考示例文件的规范、校验模式和公共模块用法为补充

### 步骤 3：读取目标文件已有用例（如存在）

在生成代码前，检查目标写入文件是否已包含测试代码：

1. **确定目标文件路径**：根据接口名称推断目标测试文件路径（如 `testcases/api/set/test_add_set.py`、`testcases/api/ld/test_add_ld.py`）
2. **检查文件是否存在且非空**：若目标文件已存在且包含测试代码，**必须先读取该文件的完整内容**
3. **提取已有用例的编码风格与模式**：从已有代码中提取以下信息作为生成参考：
   - 已有的 import 语句和依赖引用方式
   - fixture 的定义和使用模式
   - 函数命名风格（如 `test_add_set_001_xxx` 或 `test_add_ld_xxx` 的命名规律）
   - 断言方式和校验逻辑的写法
   - 配置读取和参数组织方式
   - clean/teardown 的注册方式
4. **确保风格一致性**：生成的新用例必须与已有用例保持**一致的编码风格、命名规范和组织结构**，新增用例应自然衔接已有用例（如编号连续、结构对齐）
5. **避免重复**：若已有用例已覆盖某个测试场景，跳过该场景，不生成重复用例，但是要在该用例上加上对应的design_case_uuid

> **注意**：已有用例和参考示例文件都是生成代码的重要参考来源。已有用例提供**编码风格和命名规范**，参考示例文件提供**规范、校验模式和公共模块用法**。两者需结合使用。若已有代码存在不规范之处，不应沿用，应以 `references/standards.md` 规范为准。

### 步骤 4：生成测试代码

严格遵循以下规范生成代码：

- **测试框架**：pytest（**不使用 allure**）
- **编码与测试规范**：`references/standards.md`（合并了代码风格、测试流程、禁止行为等所有规范）
- **代码模板**：`references/code-templates.md`（文件布局和通用骨架模板）

**set 类接口**：必须遵循 `references/example-set-tests.md` 中的设计要点和 clean_set 使用决策表，所有特殊规则（公共模块复用、DB 校验、clean_set 注册等）均在该文件中定义。

**LD 类接口**：必须遵循 `references/example-ld-tests.md` 中的设计要点和三层校验模式（DB → get_ld API → LD(nginx)），所有特殊规则（公共模块复用、clean_ld 注册、nginx 校验等）均在该文件中定义。

**若步骤 3 已读取目标文件的已有用例**，生成代码时还须遵循：
- 保持与已有用例一致的 import 组织、fixture 使用、命名风格和代码结构
- 新增用例的编号应紧接已有用例的最大编号继续递增
- 不重复生成已有用例已覆盖的测试场景

### 步骤 5：代码完整性验证与用例自检

生成代码后，**必须对照 `references/standards.md` 第十七章「代码完整性检查清单」逐项验证**每个生成的用例，包括基础规范、测试流程、代码质量三个维度。不同资源类型还需额外检查对应的专项检查项。

**若步骤 3 读取了已有用例**，还须额外验证：
- 新增用例与已有用例的编码风格、命名规范是否一致
- 新增用例的编号是否紧接已有用例连续递增
- 是否存在与已有用例重复的测试场景

**如果任何一项不符合，必须对用例进行修改，直到所有检查项全部通过。**