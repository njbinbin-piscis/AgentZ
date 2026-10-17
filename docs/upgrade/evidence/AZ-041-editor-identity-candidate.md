# AZ041 editor operation identity candidate

2026-10-10。限定独立纯模块，未标记路线图完成。

## 契约

`EditorConflictTracker` 同步跟踪 project scope generation、每次打开的新 tab identity、本地 revision，以及独立 read/save request generation。调用端必须在每次编辑（包括同内容 ABA）、关闭、重开和项目变化时同步更新，不能用延迟持久快照代替实时状态。

`beginRead(path, { allowDiscard?: boolean })` / `beginSave(path)` 捕获当前身份和内容。settle 仅接受身份、revision、内容、dirty 与最新请求均一致的结果。失败读取保持内容；dirty 文件删除必须保留缓冲；默认 dirty 外部内容返回 conflict。显式确认后 allowDiscard 只允许覆盖确认时捕获的 revision，确认后的输入仍令响应 stale，失败或删除不会提前清 dirty。

成功 save 只清捕获版本的 dirty，并使旧 read 无效；不执行磁盘写入。调用端 settle 后更新 React tabs 时，若新内容/dirty 已匹配 tracker，不应重复计为本地 edit。snapshot(path) 返回脱离内部状态的快照。

## 实跑

- `npx vitest run src/workspaces/codez/editorConflict.test.ts`：exit 0，9 tests，30ms（总 4.28s）。覆盖 read 乱序及重复、同内容 ABA、同路径重开、项目 ABA、保存后继续输入、dirty 删除/读取失败、确认 discard 后输入与失败。
- 两文件 scoped ESLint：exit 0。
- source SHA256：`18913faf070f34c48c1ffef4496aed0cba018f1f2b34400a7f0143e6beadf8a0`；test：`2190804568407936733fee6edc2318237b9e8d1f77084509505ed6cc93698f48`。

## 边界

这是本地异步响应与缓冲身份保护，未证明宿主写入原子性、磁盘多写者 CAS、进程崩溃恢复或 GUI 交互验收。旧保存已经写磁盘后发现本地 revision 变化，只能保留新编辑 dirty，不能撤销该磁盘写入。CodeZ 实际接线、完整 TypeScript 检查及独立复核由主线程统一完成。
