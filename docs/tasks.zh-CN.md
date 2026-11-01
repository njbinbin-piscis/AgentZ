# AgentZ 任务追踪

[English](tasks.md) | **简体中文**

> 本文是 [tasks.md](tasks.md) 的中文翻译。两者不一致时以英文版为准。

用本文件跨会话追踪多步骤工作。请与 git 历史保持同步，让人和 Agent 共享同一个事实来源。

## 状态说明

| 状态 | 含义 |
|--------|---------|
| `pending` | 未开始 |
| `in_progress` | 进行中 |
| `blocked` | 等待外部输入 |
| `done` | 已交付并验证 |

## 进行中的任务

| ID | 标题 | 状态 | 负责人 | 备注 |
|----|-------|--------|-------|-------|
| task-1 | 示例：资源管理器“在文件夹中新建”修复 | done | — | 仅供参考 |
| upgrade-plan | 产品成熟化路线图与原子执行台账 | done | Codex | [路线图](upgrade/README.md)、[任务台账](upgrade/tasks.md)及本地 CLI 已交付；validate/render/门禁自检通过。台账现有 83 项（截至 2026-11-02：65 项 `done`，18 项 `deferred`）；状态权威在 `upgrade/plan.json`。 |

开始工作时添加行。完成后删除或移到 **已完成**。

## 已完成

| ID | 标题 | 关闭时间 |
|----|-------|--------|
| — | — | — |

## 提交约定

在提交标题或正文中引用任务 ID，保持历史可追溯：

```text
fix(codez): reveal selected file in folder manager [task-1]

- add reveal_in_folder Tauri command
- wire FileTree context menu
```

规则：

1. 改动对应已追踪任务时，每个提交只带一个主要的 `[task-N]`。
2. PR 描述中使用同一个 `task-N`。
3. 关闭任务时，在同一个 PR 中（或合并后立即）把本文件中对应行设为 `done`。

## 会话交接

中途停止时：

1. 把状态设为 `in_progress` 或 `blocked`。
2. 在 **备注** 中写一行下一步的具体动作。
3. 在备注中列出失败的测试或未提交的文件。

## Agent 工作流

1. 处理多步骤请求时，先读本文件。
2. 大改动前创建或更新一行。
3. 仅在相关测试通过后标记 `done`。
4. 顺手修复不要编造任务 ID——备注留空或写 `—`。
