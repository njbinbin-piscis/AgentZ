# AZ043 Root 实施检查点

2026-10-10。三个作者首轮owned生命周期实现后，Root实际fullTypeScript Session93562退出0；合并DAP client7/controller9测试Session25246退出0，16 passed/3.33s。这些结果仅对应首轮源码，不代表后续修复已验证。

交叉独立审查发现：畸形remote_dir parse失败会落回本地执行；host stop只按UUID且允许历史UUID重用，stop先于迟到start也未留撤销记录；client listen/start缺connectdeadline仍可永久挂起。Root主动中断尚在编译的Cargo94551，实际exit1，不记作test通过或源码编译失败。

退回host作者修显式remote failclosed与有界used/cancelled会话身份历史，先stop/后start与UUID重用失败关闭；client作者修有界connect并清理迟到listen/start，不仅Promise.race。Controller首轮9受控异步fixtures与状态store保护保留，修完最终源码再统一TSC/16+新增fixtures和一个Cargo定向验证。

真实适配器、AppHandle/IPC、OS子树/远程tree、原生GUI与异常/源码跳转验收未执行；UUID是关联/归属不是新增权限，不提升正式Gate。
## 第二轮冻结与实际验证
Host退回问题已修，canonicalUUID与4096项不可逐出的used/cancelled预算、stop-before-start/replay拒绝、畸形remote早拒绝、remote准备后锁内验证spawn；client listen/start期限与迟到清理已修。Root最新Session20953 full TypeScript实际退出0；Session5042 client9/controller9实际18 passed/3.44s。7源manifest已冻结。

新的唯一Cargo Session84370 commands::dap offline/locked仍编译中；11项嵌入fixture尚未报告运行/通过，前次主动取消不作通过。独立crossreview重新核最终修复，待Cargo最终结果再冻结限定候选证据/登记，不提前声称宿主或原生验收完成。

## Native compilation repair
Cargo session 84370 exited 1: E0308 at dap.rs lines 414, 450, 483 in test assertions borrowing temporary MutexGuard. Corrected the three assertions to explicitly dereference the guard; production logic unchanged. rustfmt succeeded. Replacement single Cargo session 92435 runs the same offline locked DAP test command; result pending. Previous native acceptance remains unverified. Frontend final tsc exit 0 and 18 Vitest tests passed.


## Final native result
Session 92435 exited 0: offline locked DAP library tests 11 passed, 0 failed, 254 filtered; build 4m16s, tests 0.12s. Three test-only MutexGuard borrows corrected with explicit dereference. No production behavior changed from cross-reviewed host; refreshed manifest records actual bytes. Final frontend 18 passed and full TypeScript exit 0. Native adapter, AppHandle IPC, GUI, descendant cleanup and project-switch acceptance remain unverified.
