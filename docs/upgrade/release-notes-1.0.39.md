# AgentZ 1.0.39 发布说明

发布日期：2026-11-01。完整变更见 `CHANGELOG.md` 的 1.0.39 节。

## 修复

- **`file_search` 把文件路径当成没有匹配。** `path` 必须是目录。传入文件时返回 `Path is not a directory`，不再返回 `No matches` 或 `No files found`。工具说明改为：要搜单个文件，传它所在目录并设置 `include` 为文件名，或改用 `file_read`。

## 说明

- 引擎引用为 piscis-engine `v0.8.70`。安装包仍编译 vendor 中的同一份内核。
