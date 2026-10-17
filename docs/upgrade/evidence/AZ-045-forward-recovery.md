# AZ-045 转发恢复候选实现

日期：2026-10-07。独占修改 remote/forward.rs；主线程负责 ExtHost 接线与统一构建。先读取并保留原有格式化 diff。

## 实现

- 缓存 Entry 保存 authority；复用要求 authority 按 ASCII 大小写匹配且句柄仍活跃。SSH child 使用 try_wait 判断，list 同样剔除死条目。不同目标同端口替换旧条目，不返回旧目标的转发。未新增 UI/API authority 字段，保持 start/stop/list 形状。
- SSH 参数显式添加 StrictHostKeyChecking=accept-new、ConnectTimeout=15；未主动连接主机、修改用户 known_hosts。
- start 用异步锁串行，防止同端口并发建多个进程。stop/stop_all 推进停止 epoch；已经在启动但未登记的句柄会在登记时失败并销毁，避免停用期间迟到的 start 重新登记。
- Docker/WSL listener 捕获 bind_authority 的连接世代，tcp.connect/write/close 全部使用 request_generation。不自动重放 bytes，不对新服务器关闭旧 id。
- tunnel/early 以 (generation,id) 为键，主线程已将事件路由接到 on_server_event_generation。旧无世代事件入口无剩余调用，已移除，避免传错世代0。
- TunnelRegistration RAII 在失败、取消和正常退出时清理 map，并 abort 下行任务；相同键被新 channel 替换后，旧 guard 不误清理新 channel。下行完成后 select 退出本地上行读取，防止远端 close 后仍等本地写入。
- accept_loop 用 JoinSet 持有所有本地连接；停止 listener 时一并取消已接入任务。stop_all 清理当前 tunnel 与 early 数据。

## 验证范围

新增5项待执行测试：注册 scope 销毁清理 map并取消下行、旧 guard 保留替换 channel、同 id 不同世代隔离、真实 owned 本地进程退出后不活跃、同端口不同 authority 不复用。进程夹具仅 cmd/sh exit 0，不启动SSH；没有声称测试通过。

已运行 rustfmt（一次因Windows映射文件占用失败，重试成功）与 git diff --check（通过，既有CRLF提示）。未运行 cargo，未运行真实SSH、主机密钥变化、网络断连验收。

## 剩余风险

- SSH启动沿用800ms初始探测，尚不能证明本地监听已经就绪；慢连接可能提前返回。退出只在start/list查询时清理，尚无主动状态事件。
- SSH stderr 在存活阶段未持续消费，长时大量日志可能填满管道。
- 未注册 id 的早到事件缓冲仍无容量上限，应增加总字节限制；当前stop_all负责清理。
- 用户写入的结果不确定时不会重试。tcp.write失败仅本地资源立即清理；远端socket依赖服务器连接结束回收，未在错误路径再等180秒close请求。
- 停止 epoch 为全局，一次stop某端口也会让其他正在启动的转发返回取消错误，偏保守且可重新显式启动。
- 真正的主机密钥拒绝与SSH监听恢复需要隔离known_hosts/owned sshd夹具，仍待验收。

## 后续：就绪、诊断和早帧预算

主线程报告前5项测试已通过。本节新增实现尚未运行cargo：SSH不再固定等待800ms，而是在20秒deadline内探测localhost指定端口，child退出/检查失败/超时均返回错误；诊断管道自spawn起持续后台读取，环形尾部容量8KiB，句柄销毁会取消消费任务。探测产生一次本地TCP连接，能确认监听接受连接；不能证明远端目标服务可用，端口释放到SSH绑定间仍有既有竞争窗口。

早帧buffer限定128个连接、4MiB累计payload、每连接128帧（包含空帧）、单帧64KiB；过大encoded输入先拒绝，避免解码巨量分配。超限将整个连接世代标为failed、删除该世代早帧、向已注册隧道发送close，后续注册返回明确错误，该世代不会继续交付截断的流。新连接世代可恢复；stop_all清除预算状态。

新增6项待统一执行测试：owned本地listener延迟就绪、无监听超时、duplex持续输出超过诊断尾部容量、累计字节budget、连接数和零字节帧budget、超大frame关闭已注册隧道且拒绝再次注册。localhost测试只启动owned powershell/sh sleep进程，不启动SSH/外部主机。不宣称新增测试通过。

本节修复了前节800ms与stderr未消费、early无界三项限制；仍有注册后unbounded channel慢消费者可能积压、child退出仅查询时清理、全局停止epoch保守取消等限制，真实SSH验收仍未运行。

## 后续：注册队列预算与连接世代生命周期

主线程报告前11项forward测试已通过。本节独占实现未运行cargo：注册队列改为bounded 8槽，所有事件/早帧重放仅try_send；注册连接总数最多128，单个listener的已接入/等待连接也最多128。满队列、重放容量不足或注册连接超限会fail当前世代；使用独立watch关闭信号（不经过数据队列），即使queue满也能终止下行。下行在等待frame和write_all两处都优先监听关闭信号，阻塞本地写可被取消；超限不会静默丢字节后继续流。单帧64KiB限制保持，最大注册排队payload约64MiB，另有正在写的一帧/连接和早帧4MiB；没有无限数据队列。

新增同步API：begin_generation(newgen)仅接受更大的世代，保留独立SSH转发，移除所有旧broker listener并关闭旧sink；end_generation(gen)仅结束匹配当前的世代，旧EOF不影响新连接。failed HashSet改为当前世代Option和单个failed布尔值，重复同authority重连的失败元数据O(1)，早帧缓存每次新世代重置。非当前世代的事件/注册拒绝，不允许迟到旧数据重新污染。

锁序固定为forwards→tunnels→early（生命周期），其余注册/事件/guard仅tunnels→early；锁内无await。主线程负责接线：start应在既有target切换stop_all之后、部署await之前begin_generation，否则stop_all会关闭刚开始的世代。stop/EOF按匹配世代调用end_generation；高水位必须随主通道推进。

新增6项待运行测试：满队列关闭绕过queue、duplex阻塞写被watch取消、10000次世代切换恒定元数据、同authority重启移除旧listener且旧注册/EOF无效、128注册连接预算、早帧重放不能静默溢出。调整原世代与预算测试适配新状态，涉及全局映射的测试用独立异步串行锁避免相互重置。rustfmt和git diff --check通过，未cargo/真实SSH。

前节unbounded注册通道与同authority失败集合增长限制已解决；剩余child退出状态仍查询时更新、SSH本地端口竞争窗口、远端服务未就绪无法由本地监听证明、停止epoch保守取消，真实SSH验收仍未运行。
