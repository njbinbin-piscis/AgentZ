# 第一实施周期记录

2026-10-04，基线a787b6d，只有Codex执行。产品源码修改尚未提交/发布。

完成：AZ-001/002/003/005/006工程启动契约及基线；AZ-007持久登记；AZ-008后端解析和验证。M0 revision2 Gate通过，不代表桌面或发行验收。

评审：AZ-009按任务续聊与重开路由已接入；AZ-012 journal查询/Undo登记路由已接入。两项仍需要原生桌面或实际journal回滚证据，因此未标记done。

阻塞：AZ-004真实模型基准没有固定样本入口与配置/预算证据。依据ADR-002移入MB，不阻塞确定性修复，但仍阻断稳定版发布。

代码：新独立agentz-task-state crate；SQLite schema1；宿主登记读取命令；执行前和队列后校验；UI使用session UUID和后端查询；失效环境不静默换目录；journal路径同源。

验证：7项Rust登记/校验测试通过（含真实Git夹具）；新crate clippy -D warnings通过；最终宿主cargo check通过；前端typecheck及44项测试通过（6项新增）；台账门禁自检通过。

未执行：本轮完整Tauri打包、真实模型、原生桌面E2E、真实journal回滚、macOS/Linux。本轮完整desktop lib测试启动后停止，抽离领域crate后真实测试通过；不计完整desktop测试通过。不是全套workspace clippy/test通过。

复盘：原M0效果基准依赖会延误确定性修复，记录ADR并保留发布门槛；纯登记领域测试不应依赖首次完整Tauri测试构建；实时进度需单独生成视图；自检不冒充独立用户验收。

下一批：先完成两项review的实测证据；随后AZ-011完整变更采集、AZ-013评审UI、AZ-014/015可恢复归档。历史任务无法可靠推断原worktree时明确提示，保留数据，不猜目录。普通local/free生命周期与团队成员环境留在后续范围。
