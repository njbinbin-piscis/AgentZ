# AZ-020 持久计划候选

状态：仅独立计划库代码候选，未接入宿主；不等同 roadmap done。仅新增 `crates/agentz-task-state/src/plan.rs` 与本证据，未修改 lib/Cargo/运行库/权限库/台账。

## 契约

- `PlanBinding { project_id, task_id, environment_id, binding_version }` 保持任务/环境身份，`from_execution(&approval::Binding)` 仅复制身份；run/attempt/capability 不存入计划，不因每轮新 run 更换计划。
- `PlanStore::open(project)` 使用独立 `.agentz/plans.db`，schema 1；新版 schema 拒绝读取。项目路径规范化，任务环境对登记库匹配；未登记 direct 环境必须匹配 binding_version 指纹。宿主仍须预先验证当前环境及执行授权，本库不授予工具权限。
- `create(binding,id,objective,items,now)` 仅允许无当前计划。计划 ID 由宿主生成稳定值；item ID 也由宿主保持，禁止重复。revision 起始为 1。
- `continue_plan(binding)` 只读返回当前计划，重启或普通续聊不清除条目、不增 revision、不重开 cancelled。
- `load(binding,id)` 可读同绑定历史；跨 task/env/project 或 binding_version 失败。
- `update(binding,id,expected_revision,items,now)` 使用立即事务与 revision CAS；目标文字不变，拒绝旧 identity/revision、时间倒退、关闭计划。
- `replace(binding,old_id,expected_revision,new_id,objective,items,now)` 显式新目标：同库事务将旧计划置 replaced 并保留全部条目，创建新 ID/revision 1，然后切换 head。重复新 ID 失败整笔回滚。
- `cancel` 与显式 `clear` 同义：将当前计划关闭为 cancelled，保留历史及条目。后续目标须调用 replace。该操作不取消 Run，也不撤销执行权限；宿主必须另行协调运行取消。

## 验证证据与未完成范围

已实际执行 `rustfmt --edition 2021 crates/agentz-task-state/src/plan.rs`，exit 0。未启动 Cargo、GUI、模型或打包构建，未声称编译/运行验收通过。

内嵌 10 个测试夹具：重启与续聊保留；两连接过期 CAS 与跨 task/env；显式替换保留历史且失败回滚；显式 clear 后续聊不重开；重复条目/时间倒退不写；新 schema 拒绝；登记环境与跨 run 稳定身份；并发首次初始化；损坏 record/head 拒绝读写；completed 显式延长、clear 与替换。尚待主线程统一 Cargo 实跑。

后续最小兼容包：新增 `ItemState::Cancelled`，serde 值 `cancelled`，对应内核合法 cancelled 条目，保持 ID/description/state，无损区分 Done。非空列表中没有 Pending/InProgress 时计划状态 Completed（包含 mixed done/cancelled 与 all cancelled）；整个计划的 Cancelled 状态仍必须显式 cancel/clear，不由条目推导。新增 2 个测试（共 12）：mixed/all cancelled、重启 serde 恢复及显式整计划取消；损坏条目状态与 active/terminal 矛盾拒绝。本代理仅实际执行 rustfmt/check exit 0，新增测试尚待主线程唯一 Cargo 验证；此前 10 个测试通过与 Clippy 通过由主线程报告，不能当作本次新增代码的验证结果。

复核修正：初始化在 Immediate 事务取得写锁后重读 schema，避免两个首次 open 都按版本 0 建表；版本 1 表缺失拒绝而不修复。读取检查 objective、绑定形状、时间顺序及 active/completed 与条目状态一致性；当前 head 不得指向 replaced 历史。项目路径持久保存规范绝对值，避免相对路径后续 cwd 变化。

managed binding_version 边界：`src-tauri/src/runtime/approval.rs::resolve_binding` 对 root、登记 record、directGit 计算 SHA256，确为 64 位 hex。计划库只校验登记环境 ID 与已保存完整绑定相等，不能独立重算上述宿主快照；调用方须在每次操作前 fresh resolve_binding。计划 DTO 自身不是可信授权。Completed 计划允许显式 update 追加/恢复未完成条目，目标不变；取消/clear 后不能 update 重开，必须显式 replace。计划取消始终不等同 Run 取消。

主线程最小接线：在 crate lib 导出 `pub mod plan;`，然后实际运行 `cargo test -p agentz-task-state plan::tests --locked`（或按工作区 manifest 指定路径）。宿主映射内核计划条目/状态并明确区分普通续聊、新目标替换、明确取消；不要在每轮开始调用 clear。接线后必须跑宿主重启续聊与明确取消集成案例，才能评价 AZ-020 验收。

SQLite 事务只涵盖 plans.db 内的 old/new/head；不与 runs.db、任务环境登记库或 conversation DB 原子提交。跨库环境新鲜度、活跃 Run 取消、恢复时的一致性检查均属后续宿主集成。现有 `.agentz` 路径信任规则沿用任务状态库，未新增 symlink 防护策略。历史无删除/压缩接口，本轮没有迁移旧临时计划。

主线程最新源码实跑9245：plan::tests 10 passed/0failed，events::tests 6 passed/0failed；全targets strictClippy退出0。旧快照全crate76668为57 passed，不能代称最新全量回归。此模块是0.9.7包构建后的新工作，宿主续聊尚未接线。


主线程追加实跑53196：cancelled兼容最新plan::tests共12 passed/0failed，exit0。不代表宿主接线或GUI已验证。

