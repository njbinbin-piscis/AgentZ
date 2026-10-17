# AZ-013 并行复核：评审实例与迟到操作

2026-10-07，用户授权子代理并行后复核既有 AZ-013 review 候选。读取任务台账、原 AZ-013-review-ui.md 及既有 diff，保留原完整变更评审/归档/合并事务实现，仅编辑 AgentTaskReview.tsx、其测试及本证据，CSS 无需改变。

发现具体缺陷：列表和文件加载已使用 cancelled 标志，但 mergePreview、merge、discard、openPr 的异步操作未绑定评审实例。切换任务时旧预检完成后仍会继续提交旧任务合并；旧操作结果会覆盖新任务状态、恢复位置及 onResolved 通知。即使 task id 不变，环境绑定改变也未完整清除旧状态。

修复：外层根据项目、任务 id、environment id、worktree 路径、分支和冻结基点生成 React key，隔离不同评审身份的状态。内部 active 生命周期标志在卸载时失效：旧 mergePreview 完成后不再提交 merge；已提交合并、归档或 PR 的迟到结果不写入新评审或通知新上下文。已发送后端操作并未撤销，不宣称替代后端事务/运行租约。

实跑 `npx vitest run src/workspaces/workz/AgentTaskReview.test.tsx`：17 passed / 0 failed，session 34310，退出码 0，测试耗时 168ms、总耗时 6.37s。新增 5 项覆盖四类来源及 rename 旧路径、预检期间切任务阻止提交、归档迟到隔离、同任务换环境清除旧结果、同路径旧任务文件响应忽略；既有二进制提示、列表/文件读取失败、未提交阻止合并、冻结评审 HEAD 与恢复引用测试继续通过。只运行 mock API 的 React 组件测试，没有提交真实合并、归档或 PR。

修改两文件 prettier 完成；针对两文件 ESLint session 57596 经原执行子代理恢复查询，确认最终退出码 0、无诊断。2026-10-07追加只读复核时该 node 进程已退出；未重复启动 lint 或全量构建。

交互/后端边界只读复核：WorkZ 父组件关闭评审时卸载该组件，因此 active 失效；新身份 key 也覆盖同 task 换 environment/branch/base 的情况。mergePreview IPC 传 stable task id 与 reviewed task HEAD，merge IPC 同时传 expected target HEAD；宿主持有仓库/执行租约，独立 worker 持续完成事务。因此关闭窗口只阻止未发送的后续 merge 和迟到 UI，不撤销已发送事务，符合后端注释与行为。列表/文件读取的 cancelled 标志仍保留，不因本轮实例隔离失效。

仍有后端边界缺口（只读发现、未改）：openPr IPC 只传 project/branch/base/title/body，没有 stable task id、reviewed HEAD、运行租约或预检版本；宿主 best-effort push 后直接 gh pr create。前端已加载/无脏状态禁用规则并不能证明点击到后台 push 之间源码不变。此范围需要独立产品/外部动作契约实现，不在本轮组件竞态修复内，也没有运行真实发布操作。已发送但卸载的操作结果不再由此评审实例展示，持久回执/任务恢复仍由对应路线图任务补齐。

未运行 GUI，不替代原生桌面目录与评审列表核对，AZ-013 保持 review，不计为 done。台账由主线程统一更新。
