# AZ-045 SSH 与旁路恢复只读审查

日期：2026-10-07。范围：remote/mod.rs、forward.rs、ssh_setup.rs、lsp.rs、mirror.rs。仅阅读代码；未运行 SSH、修改 known_hosts、启动构建或验证真实主机密钥变化。以下运行结果均为代码路径推断，不能替代真实远程验收。

## 可行动问题

1. **P1：增量镜像失败后可能永久漏掉文件变更。** mirror.rs:109-121 未检查传输进程退出状态；extract:179-185 对 tar 失败仅记录警告并返回成功；sync_script:130 在传输前推进远端 stamp。已有成功镜像在增量传输中断后，下一轮以新 stamp 过滤，失败那轮未交付的旧变更可能被跳过。最小安全方案：传输退出码/提取失败必须返回错误，失败时删除本地 `.agentz-mirror-ok`，使下次按 fresh 全量补偿；更完整方案是每次创建独立候选 stamp，仅在本地提取和删除同步成功后提交。受控测试：先建立镜像，修改文件，模拟非零退出/截断归档，确认同步不标成功且下一轮全量包含该文件。文件名兼容失败需要明确降级状态，不能静默通过。

2. **P1：远程 LSP 双管道 EOF 后仍可能保持就绪。** lsp.rs:164-166 创建 out_tx，reader/log 各持一个 clone，但 bridge 自身持有原 sender 至循环结束。两任务 EOF 后 out_rx.recv() 不会返回 None；WebSocket 仍开且客户端不写时，done 保持 false，start:44 复用失效 port。最小方案：spawn reader/log 后 drop 原 out_tx，并把 stdout reader 完成纳入 select（否则 stderr 长时不关闭仍阻止及时退出）。Session 还应保存 bridge JoinHandle，stop/stop_all 终止 listener/bridge，避免尚未接入 WebSocket 的桥在 child 被杀后继续监听。受控测试：真实本地夹具 child 的 stdout EOF、stderr EOF、stderr 保持打开三种情况，不需 SSH；确认 done 和 WebSocket 关闭。

3. **P1：转发 tcp.write 错误绕过资源清理。** forward.rs:235-240 `.await?` 直接退出 pipe，跳过 tunnels.remove(id) 和 down.abort()。down 可因 map 中 sender 长期等待；重连后 id 若由新服务器复用，旧连接状态可能污染新连接。最小方案：RAII guard 管理注册的 tunnel 与 down handle；写失败路径统一清理，不跨新连接发送旧 tcp.close。事件 map 和 early buffer 必须纳入连接世代或断连清空。受控测试：注入 write failure，确认 map 移除、下行任务终止；新世代同 id 不接收旧帧。

4. **P2：SSH 转发登记后不监测 child 退出。** forward.rs:126 对已有 remote_port 无条件返回缓存；800ms 延时仅检查初始退出，后续 SSH 死亡 list/start 仍返回旧条目。且建立连接慢于800ms会提前报告成功，缺少 ConnectTimeout。最小方案：持有 child 的监视任务，在退出时按条目 identity 清理；等待本地监听就绪或退出/超时后才返回成功。缓存键应包含 authority/连接世代，避免同端口跨目标复用。检查 stop_all 调用能降低但不能消除并发竞态。

5. **P2：镜像缓存键将远端路径整体转小写。** mirror.rs:49-54 导致 Linux `/Work` 与 `/work` 共用镜像、锁和 stamp。最小方案：仅规范 authority，保留 path 大小写；更新现有专门断言大小写不敏感的测试。受控测试：同 authority 不同大小写路径必须不同 mirror_dir，避免旧缓存混用。

## SSH 策略与重试观察

- mod.rs:155 普通 SSH、ssh_setup.rs:68 安装密钥入口显式 `StrictHostKeyChecking=accept-new`；forward.rs 与 interactive_shell_argv 未显式配置该选项，继承用户配置，无法据源码证明所有入口都拒绝已变更密钥。建议集中生成公共 host-key/ConnectTimeout/keepalive 参数，让非交互转发使用相同策略；交互终端的认证提示可以保留。真实主机密钥拒绝必须在隔离 known_hosts 夹具中验收，不触碰用户文件。
- mod.rs 普通 SSH 有 ConnectTimeout=15、ServerAliveInterval=15，run/run_with_stdin 没有完整业务 deadline；远端进程保持连接但挂起时仍会无限等待。ssh_setup 有60秒外层 deadline，但 password write 的 `?` 可绕过末尾 child.kill；建议所有退出路径用子进程 guard，并给 setup SSH 增加连接超时。
- 当前所读模块未见对 fs/脚本写入的显式自动重试。`authorized_keys` 用 grep 检查后追加，在顺序重复调用上有去重意图，但不是并发原子事务；扩展上传通过 package.json 判断完成，失败的部分解压可能被下次误判为完成，建议临时目录+成功完成标记。部署 server.js 使用临时文件+mv；并发部署同版本共用 tmp 文件仍需独立临时名。
- 单次请求已发出而回复丢失属于结果不确定，不能因断连简单重发副作用；镜像全量补偿属读取恢复，应该和用户写入重放分开处理。

## 可独立实现的模块

- mirror.rs：失败恢复/完成标记与大小写缓存隔离，可独立于 ExtHost 请求状态修复。
- lsp.rs：桥任务 EOF 与 stop 生命周期，可独立于主通道；不要与其他代理同文件并行编辑。
- forward.rs：转发资源 guard、世代和 child 退出监听；需要主代理确认 ExtHost 世代接口后实施。

建议先解决镜像失败补偿与 LSP EOF，因为它们直接影响断连后是否能恢复，随后验证转发资源释放与统一 SSH 参数。以上不构成 AZ-045 已验收证明。

## 本轮授权实现：镜像失败关闭

在保留原有 diff 后，仅修改 mirror.rs：启动远端同步前移除本地完成标记（移除失败就终止，未触碰远端 stamp）；短时缓存复用也必须具有完成标记；远端传输非零退出直接失败；本地 tar 解压失败返回错误；解压子进程设置 kill_on_drop；成功标记写入失败向上传播。故远端 stamp 已推进后的传输、提取或后续流程失败，以及 future 被取消，都保留标记缺失，下一轮全量读取补偿。

新增三项测试：已有完成标记失效且重复失效可用、标记移除错误被拒绝、损坏归档不被当作成功。只运行 git diff --check（通过，存在既有CRLF提示），没有启动 cargo；测试由主线程统一运行，不宣称测试通过。

剩余风险：本地镜像尚非目录级原子切换，失败时仍可能有部分更新，旧索引必须遵守同步错误；远端 find/filter 管道错误没有全面 pipefail 协议，删除同步也有既有忽略本地IO错误问题；路径整体小写问题未改。临时完成标记失效是恢复兜底，未实现远端两阶段 stamp 提交；真实SSH断连未验收。Windows不支持的远端文件名现在会显式报错，需要后续产品选择降级策略。

## 后续授权实现：大小写隔离与缓存迁移

mirror.rs 的 key 改为解析远程 URI，仅将 decoded authority 转 ASCII 小写，保留 decoded path 大小写并维持末尾斜线归一化。authority 带长度前缀，避免字符串拼接歧义；无效 URI 保留原字符串作为独立无效 key，不再整体小写。

本地目录改为 `remote-mirror/v2-<hash>`，远端 stamp 改为 `mirror-v2-<hash>.stamp`，内存 key 也含 v2 版本前缀。旧目录和 stamp 均不复用且不删除，首次同步因新目录无成功标记而全量读取，避免迁移旧碰撞镜像。旧数据仅是缓存，将来可单独清理。

更新原大小写不敏感测试为仅 authority ASCII 大小写归一化，并覆盖 URI 编码等价及非 ASCII authority 不折叠；新增 `/Work` 和 `/work` 缓存隔离、旧命名空间不复用两项测试。git diff --check 通过。未运行 cargo/SSH；所有新增和修改测试仍待主线程统一验证。保留之前失败关闭修改；前节“路径整体小写问题未改”风险在本节实现后已解决，其他剩余风险不变。
