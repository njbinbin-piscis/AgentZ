# AZ043 host independent source review

2026-10-10。复核者未实现 dap.rs/dap_frame.rs；只读源码复核，未自行运行 Cargo/native adapter/GUI。

## 观察到的正确性防线

reserve 在异步终止旧 child、远程连接和 spawn 前分配单调 generation 与 UUID，清旧 stdin。install 仅匹配 lease 才安装；旧 spawn 失败的 take_owned 带 generation，不能撤销新 owner。stdout/stderr 发布在 manager 锁内检查 lease 后同步 emit，旧 EOF 同样只清自己的 generation。send 在同锁内检查 UUID、限 1MiB JSON object，2s 写超时/partial-write 失败后撤销 owner，禁止重试部分帧；旧 UUID stop 无匹配时 no-op。

帧解析器单 chunk 8192 bytes、header 16KiB、body 1MiB；checked arithmetic、纯十进制长度、重复 Content-Length、UTF8/JSON object 和 EOF 未完成帧均拒绝。无无界 read_line/body allocation。未知 header 可接受属于兼容行为，非权限来源。

## 具体边界 / 建议

1. **P2：畸形 remote_dir 本地回退。** dap_start 使用 remote_dir.and_then(vfs::parse)，parse None 走本地 spawn。显式远程意图应解析失败拒绝，不能静默改为本地 command/cwd。此为真实执行边界，不是帧测试可证明。
2. **P2：UUID 可重用后的旧 stop。** reserve 只拒当前相同 UUID；撤销后同 UUID 可再次占用。旧 lease event/install 有 generation 防护，但外部 dap_stop 仅 UUID，能停止相同 UUID 的重用实例。当前客户端每对象新 UUID、one-shot 防止常规 UI 路径，但 host 未强制历史 UUID 不重用。可拒历史重复或增加所有操作 generation token；不要声称 host 已证明任意 UUID 重用隔离。
3. kill_on_drop/start_kill/wait timeout 为直接 adapter/SSH transport 的尽力终止；terminate 忽略 kill/wait 失败，stop Ok 不等于所有 OS 进程已回收。debuggee、远程进程树和继承 pipes 的后代未被 job/process-group 证明约束。stdout/stderr 后台任务在后代保持 pipe 时可继续等待。该限制不能用 client dispose deadline 改称确认停止。

## 测试覆盖与未覆盖

4 个 lifecycle 源码 fixtures：reserve/旧 stop 与失败 lease、pending start stop、真实合成 child 的 stale install 保留新 PID、invalid/duplicate 当前 UUID 不撤销。3 个 frame fixtures：拆帧/多帧、坏长度/JSON/重复 header、无终止 header/不完整 body。

真实 child fixture 使用 Windows cmd set /p（或 sh read），但断言主要是 manager child ownership/PID identity；未验证被拒绝 child 的 OS PID 消失、跨进程树、native debugger 握手、实际 partial-write timeout 或 AppHandle emit 路由。主线程在运行统一 Cargo，本文不预报测试成功。
