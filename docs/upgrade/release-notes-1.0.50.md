# AgentZ 1.0.50 发布说明

发布日期：2026-11-04。完整变更见 `CHANGELOG.md` 的 1.0.50 条目。

## 修复

- **Linux 和 macOS 安装包可以再编过。** `code_run` 的诊断函数在非 Windows 上没有被调用，`-D warnings` 让 1.0.49 的 Linux 和 macOS 任务失败，GitHub 上只留下了 Windows 安装包。这些函数现在只在 Windows 和测试里编译。
- **内核引用改为 piscis-engine `v0.8.73`。** 同一批工具说明已在上游打标签。安装包仍编译仓库里的那份内核。

## 内核

- piscis-engine `v0.8.73`。
