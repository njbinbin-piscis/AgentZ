# ADR-006 候选代码依赖与正式验收依赖分离

2026-10-07，主线程授权实现本地台账工具候选，产品打包期间不修改产品源码。仅编辑 upgrade-plan.mjs、其隔离验证脚本与本证据；未修改正式 plan.json，也未批准任何正式上游实现或改变任务状态。

## 保留的验收约束

原 dependencies、里程碑前置与 Gate 均不变。doing/review/done仍需要原任务依赖全部done及原Gate满足，preparing不能直接done，完成数/Gate通过数不因实现复核增加。done记录仍不可变，任务/里程碑循环与状态转换保护保持。

## 仅候选开工的新规则

下游任务可显式登记 code_dependencies，必须是原 dependencies的唯一子集；未显式列出的原依赖仍需done。对于列出的非done上游，只有其状态为review或preparing、有独立实现复核且源码/证据指纹当前匹配时，才能让下游进入preparing。没有复核记录的下一层候选不继承许可，不递归免证据。

实现复核限定scope/verification，不表示上游全部验收。复核人必须与实现owner字段不同，且调用者必须显式传 independent-review true。工具仅记录主线程对独立复核的明确声明，不能凭名称认证真实独立性，也不会运行测试、推断通过、自动批准或替代主线程阅读证据。

## 主线程明确操作接口

源指纹JSON格式：

```json
{"files":[{"path":"crates/agentz-task-state/src/run.rs","sha256":"64位小写SHA256"}]}
```

所有源文件及证据/manifest必须存在于仓库或该隔离台账目录，realpath不能逃逸；files非空、路径唯一、SHA256必须与实际文件匹配。

```text
node scripts/upgrade-plan.mjs record-implementation-review AZ-017
  --reviewer "Codex (主线程独立复核)"
  --independent-review true
  --scope "已复核的Run/Attempt存储与身份接口；GUI/模型未验收"
  --verification "实际运行的命令、结果和覆盖范围"
  --evidence docs/upgrade/evidence/实际独立复核证据.md
  --source-manifest docs/upgrade/evidence/实际当前源码指纹.json
  --note "明确的限定实现复核决定"

node scripts/upgrade-plan.mjs update AZ-018
  --owner "Codex (事件实现)" --status preparing
  --code-dependencies AZ-017
  --note "仅消费限定已复核代码接口；原验收依赖/Gate保留"
```

示例是接口说明，不能复制占位证据去宣称通过。本轮没有执行这些命令修改正式台账。

record-implementation-review只接受preparing/review上游，保存reviewer、限定scope、verification、时间、源文件SHA，以及证据和源manifest自身SHA。它不改变status或完成信息。下游消费、validate、summary及任何正常更新时都会检查所消费的最新指纹；源码/证据/manifest变化立即拒绝过期复核。刷新命令允许读取过期候选记录，但仍要求重新明确flags、最新manifest与实际证据，并在保存前验证全部候选依赖。不能用旧manifest自动追认新源码；同时存在多个无关过期上游时需先形成可一起验证的复核/处置流程，不推断它们继续可用。

## 隔离实跑

upgrade-plan.verify.mjs仅复制正式台账到owned临时目录。保留原状态/Gate/完成材料/不可变done/新增任务/循环测试，新增：未复核候选拒绝、缺explicit flag拒绝、同owner自审拒绝、空manifest/缺证据拒绝、有scope与当前指纹可仅preparing、源SHA变化拒绝与明确刷新、证据变化拒绝、review/done仍受原约束、未经独立复核的孙级候选拒绝、新边不允许替代原dependencies、done/remaining/Gate计数不变。

最终实跑 `node scripts/upgrade-plan.verify.mjs` 退出码0（会话29669），全部既有及新增断言通过；正式台账字节在验证脚本结束处与运行前比较，未改变。随后只读 `node scripts/upgrade-plan.mjs validate` 退出码0，83 tasks / 11 milestones一致。生成的隔离tasks/progress文件位于临时目录，不作为正式完成证据。

该工具候选解决“实现接口已交付但仅真实验收资源缺失”导致的代码排期阻塞；不豁免真实GUI、模型、平台、秘密、授权或发行验收。主线程应先审阅工具与独立复核证据，再选择性应用正式记录。
