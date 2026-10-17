# AZ043 bounded implementation review

Independent host review by parallel_plan and frontend review by ssh_audit are preserved below. Root integration addendum: native compilation required three test-only explicit MutexGuard dereferences; actual final offline locked 11 tests passed. Frontend 18 tests and full TypeScript passed. Production host behavior unchanged; manifest refreshed. This is limited implementation review, not original acceptance or release approval.


# AZ043 host followup independent review

2026-10-10。仅只读源码复核；保留旧独立复核及其当时问题，本文不覆盖原记录。未运行 Cargo/native debugger/GUI。

核对实际 SHA256：dap.rs `2acf7ba0dcb9a8fbce320d83d7046b89e4209ed3a0807179f8c3b5768b9de496`；dap_frame.rs `35ac101ac0035f66ce7c1a0f145132a5c34298ed8dca0ed65d8870a9c9eac1ea`。

## 复核结果

前次两项具体 P2 已在源码闭合：Some remote_dir 必须成功解析远程 URI，否则在 reserve/revoke/spawn 之前报错，无本地回退；canonical lowercase UUID 校验及 manager-lifetime used_or_cancelled 集合禁止 stopped/failed/used UUID 再 reserve。

unknown stop 在 manager 锁内登记 canonical UUID，故 stop 先于迟到 start reserve 时拒绝启动。4096 个 lifetime 标记为固定预算，无 eviction；满后所有新 reserve 拒绝并提示重启。满时无法追加 unknown marker，但所有新 start 已拒绝，因此无绕过窗口，当前 owner 不因容量检查而撤销。重复/别名 UUID 在 reserve 前拒绝。

spawn_owned 在网络准备之后取得短锁，检查 captured generation/UUID，再同步 spawn 和安装；取消 preparation 后不会执行 spawn。此锁无 network await，但 OS spawn 的同步耗时仍可能延迟 stop，不能称硬实时取消。后续 lease emit/EOF 与失败 take_owned 防线保持。

未发现这次更改引入新的阻塞正确性问题。capacity exhausted 是明示的 failclosed 可用性限制，需产品记录，不应自动清 marker 放行。

## 源码 fixture 与真实验收限制

新增源码 fixture 涵盖 stop-before-start、UUID replay/uppercase 拒绝、4096 预算不 eviction/不撤销 owner、cancelled preparation 使用不存在 executable 验证未进入 spawn、畸形 remote input 不成为 local。原 real synthetic child manager ownership 与 frame fixtures保留。

主线程 Cargo/TSC/前端统一回归尚在运行，此文不预报其结果。kill_on_drop、start_kill+2s wait 仍只对直接 child 尽力清理；未证明 Windows debuggee/SSH remote process-tree、继承 pipes 后台任务或真实 debugger 生命周期全部结束。客户端有界 stop/connect 等待不能替代 OS cleanup 证明。


# AZ043 frontend deadline follow-up review

2026-10-10. Independent read-only inspection of the client author’s final connection-deadline changes. No source edits, tests, Cargo, TypeScript build, adapter launch or GUI were run by this reviewer. Root owns the running host/frontend/TypeScript checks and their eventual results.

Full reviewed SHA256:
- `src/extensions/debug/dapClient.ts`: ACB362B4DDBEA5CF7EB8AF2707463AE0897AE4109E7DA896CB4F6B0FC904D509
- `src/extensions/debug/dapClient.test.ts`: F5DD7872DC2C8BEF200DAEF4447522A38E9C505F6897DEB25128E4C8C403E7D3
- Unchanged controller: E533C74A33CA200271A6FF37A8DD11BCFF5A80186DA2A184FB0E647A4BBE9877
- Unchanged store: E4D581446D5D9A961AE6B65F9887CD986D7511540FFE45A066E043AA29FFFFC6
- Host used for contract comparison: 2ACF7BA0DCB9A8FBCE320D83D7046B89E4209ED3A0807179F8C3B5768B9DE496 (reviewer authored host; this is not independent host acceptance).

## Result

No new blocking defect found in the final client deadline changes. The earlier unbounded acquisition finding is addressed at the JavaScript operation boundary. Listener registration and host start each race a validated positive deadline (default 10 seconds per phase, not one combined 10-second connection deadline). Phase timers clear in finally on success, failure or timeout. Connection failure revokes local state/listener/pending request ownership and performs same-UUID bounded stop before rejecting. Cleanup can add the configured stop timeout to the phase deadline; no promise of instantaneous timeout or immediate OS reaping is made.

The real IPC promises retain explicit late handlers instead of being abandoned by Promise.race. Listener timeout does not assign the listener to the client: its late successful registration calls its returned unlisten once, and no dap_start is subsequently sent. Successful registration followed by disposal calls the local returned unlisten before throwing, while registered owned listeners are cleared by revoke. Start timeout revokes and stops its captured UUID immediately; late success repeats stop with that same UUID, never with the returned potentially foreign identity. Late rejection is consumed. Terminal clients reject future requests without sending. Repeated revoke finds no listener or pending entries, so previously owned cleanup is not repeated via those paths.

Stop-before-start now closes across the current broker contract: the first cleanup stop records canonical UUID cancellation even if reservation has not arrived. The late host reservation is refused; started UUIDs cannot be reused. When the 4096-marker budget is full, all new reservations fail closed without evicting cancellation markers. A late cleanup for old UUID cannot stop a newer UUID. This addresses the specific pre-reservation dispatch ordering finding from the earlier review; it does not claim cancellation of already-completed debuggee side effects.

The prior controller/store review remains applicable because their hashes are unchanged: synchronous detach, captured session/epoch and pause guards, negotiated configurationDone and failing initialized deadline are retained.

## Fixture inspection and remaining evidence

The two added fixtures use actual DapClient with deferred mocked Tauri IPC and fake timers. They inspect never-answering listen/start deadline rejection, late listener disposal, late start same-UUID stop, no start after listener expiry, timer cleanup and terminal request rejection. Existing seven fixtures still cover UUID filtering, send failure, exit, disposal during listen/start, bounded stop and mismatched reply. This review inspected these tests without executing them; do not use this document as a test-pass receipt.

A deadline settles the client-facing operation but cannot cancel an unresolved native IPC Promise itself; the retained continuation intentionally exists until it completes. Bounded broker stop, tombstones and conditional spawn protect current ownership on eventual dispatch. No native adapter/GUI walkthrough, process-tree reaping guarantee, automatic project-switch stop, source-navigation or exception-policy acceptance is added here.


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
