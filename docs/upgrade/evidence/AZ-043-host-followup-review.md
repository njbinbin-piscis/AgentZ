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
