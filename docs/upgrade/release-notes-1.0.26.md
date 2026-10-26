# AgentZ 1.0.26 发布说明

发布日期:2026-10-26。完整英文变更见 `CHANGELOG.md` 的 1.0.26 条目。

## 修复

- **BotZ 成员被强制只读且没有 pool 工具。** 成员回合经过后台只读限制,`shell`、写文件、`pool_chat`、`pool_org` 都被删掉,所以 1.0.25 的 pool 工具接线没有生效。现在成员在自己的 worktree 里保留全部工具,只关闭 `delegate`、`call_fish`、`task`。
- **搜索遇到受保护条目就整体失败。** 搜索范围里有 `.agentz`、`.codez`、`.piscis`、`.git`、`.env*`、`.secret_key` 时会报 `protected metadata or secrets`。现在这些条目被跳过,搜索工具本身也不会进入工具状态目录或读取 `.env`、`.secret_key` 的内容。

## 未改动

- 委托子 agent 在不可写模式下仍是只读;`file_read` 的授权根目录校验不变。
