# AZ-019 显式可信失活对账领域候选

2026-10-09。新增独立 recovery.rs；不改run.rs/lib.rs/host/台账。单文件rustfmt实际exit0，SHA256 `48176ed20a2c87c9c99721dbff24260e38f5bbbabca8ec56a011c99ec8d33043`。本代理未运行Cargo、真实进程、GUI或模型；主线程接lib导出后统一运行。

## 安全限定

现Run/Attempt无持久boot/PID，宿主未证明single-instance，因此**不能在app startup看到Running就自动标Interrupted**。API只接受可信host `OwnerEvidence::Alive / Unverifiable / ConfirmedGone{evidence_id}`；ConfirmedGone是调用前必须独立成立的失活证明，crate不能验证真实OS事实。该enum/CapturedRun不序列化给renderer或模型，不注册IPC。evidence_id仅审计引用，不能代替可信证明；host未来接线仍须有owner登记/互斥或有效执行租约，以免证明后其他进程继续执行。当前没有自动启动扫描、自动resume、重跑工具或创建retry。

CapturedRun固定完整Binding、expected active execution和exact updated_ms。既有库 READ_WRITE/noCREATE；缺event namespace明确拒绝，不把旧v1空event自动激活成恢复证据。Immediate事务内复用Run严格布局/所有history元数据校验；核project/task/env/currentAttempt/精确状态及时间CAS。Alive/Unverifiable保留状态，不写库。

ConfirmedGone仅对Queued/Running/Cancelling进行Run+Attempt Interrupted更新，然后同事务append真实run.execution生命周期事件（execution.interrupted稳定ID，previousexecution/evidence引用/automatic_resume=false）；commit一次，事件失败两状态row回滚。两连接抢同capture只一成功；之后旧capture失败，不恢复或重复发事件。预算Unknown/审批/工具OS状态不自动释放，Run中断不意味着已执行副作用可回滚。

## 六个真实SQLite fixture（尚未实跑）

- Alive/Unverifiable preserve且DBbytes不变。
- ConfirmedGone中断后reopen，exact一条事件、历史Attempt数量不变，旧capture重复拒绝。
- 两独立连接barrier竞争，只有一个CAS成功。
- event INSERT触发器ABORT，Run/Attempt保留Running、没有中断事件。
- 错环境、retry后旧capture、负历史Attempt时间拒绝。
- 缺库不建目录；缺event namespace不repair、DBbytes不变。

建议主线程最小 `pub mod recovery;` 导出（不需要修改RunAPI），运行 `cargo test -p agentz-task-state recovery::tests --offline` 及严格Clippy；独立审查后刷新上游Run指纹仅在Run确实改动时需要，本包未动Run。

## 未完成

尚无host可信失活来源、启动扫描/owner注册、真实多进程/杀进程/重启旅程、UI解释和模型/GUI验收。Unverifiable必须继续显示待对账，不能为推进速度伪造ConfirmedGone。候选领域API只为后续安全host契约提供原子中断能力，不表示AZ019全任务done或正式Gate通过。

主线程38615实际六fixture均失败在初始化：managed task environment未登记，尚未执行恢复断言。本代理修fixture为真实direct Binding环境及RunStore.create_direct；未削弱生产校验，生产路径未变。单文件rustfmt exit0，未运行Cargo。旧通过声明不存在；主线程需重跑。
