# AZ-045 接线复审与候选验收矩阵

日期：2026-10-07。审查者：Codex 子代理。主线程管理最终集成、构建和台账。此文件为本地审查证据，不表示AZ-045已完成。

## 世代接线与锁序

源码阅读确认：ext_host_start在改变target的stop_all、旧stdin与pending清理之后、部署await之前调用begin_generation；reader先获取current_generation guard，当前EOF/read error才end_generation；stop先停止旧转发，再begin新高水位并end。旧EOF无法清理新世代，stop不会把新连接当作已运行。start部署失败时connected保持false，后续bind_authority拒绝，因此仅begin成功不意味着连接ready。

同步锁顺序为reader generation→forwards→tunnels→early；forward生命周期forwards→tunnels→early；注册/事件/guard仅tunnels→early；start先await bind_authority（inner/generation），释放后才获取forwards。未发现反向获取generation的forward回调，未发现本轮接线锁反转。锁内watch通知不await，不触发同步manager回调。本结论是静态审查，非长时并发实测。

## 具体P2发现与修复

发现：满队列/预算失败后，fail_tunnels清理registered sink但accept_loop仍活跃，缓存start/list可能返回旧成功端口，即使所有新注册都会因failed拒绝。该问题不会跨世代继续写入，但导致错误成功状态与无效重试。

本轮主线程授权独占修复forward.rs：Docker/WSL start在缓存复用之前bind authority捕获generation；cached_forward在返回缓存前要求该代仍被accepts，否则返回明确“reconnect the remote broker”错误。登记新句柄之前再次核对代状态，防创建期间预算失效。list剔除failed broker listener；独立SSH不受broker失败影响。锁序forwards→early，不在锁内await。

新增2项测试：同authority活跃缓存在预算fail后不报告成功；list从原健康listener到失败后移除句柄。未运行cargo。rustfmt、git diff --check通过。原本43项remote过滤通过是主线程报告的此前快照，不能覆盖这两项后续改动。

未发现需要新增范围的P1接线问题。仍待验收：真实SSH断连/重连/主机密钥变化，跨文件/LSP/终端/Agent统一GUI状态。SSH本地监听探测不能证明远端目标服务ready；cached SSH child退出状态仍查询时更新。真实主机环境不具备时不得用本地夹具替代验收。

## 验证归属与矩阵

| 项目 | 证据归属 | 当前结论 |
| --- | --- | --- |
| URI错误与Unicode | 主线程rustc 4项，remote-recovery-candidate记录 | 历史本地通过 |
| 绑定session、旧write零帧 | 主线程会话55905 7项，本地owned替换子进程 | 历史本地通过，非SSH |
| forward初版 | 主线程77670 5项 | 历史本地通过 |
| forward就绪/诊断/early预算 | 主线程13793 11项 | 历史本地通过 |
| bounded/lifecycle集成 | 主线程报告remote过滤43通过/5 ignored | 此前快照通过；5个真实远程ignored未跑 |
| failed broker缓存状态 | 本轮子代理新增2测试 | 尚未执行，由主线程统一构建 |
| 真实SSH故障与host key变化 | 没有运行证据 | 未验收 |
| GUI/model/cross-platform/rollback | 没有本轮运行证据 | 不宣称通过 |

scripts/upgrade-remote-candidate.mjs只读取指定源文件、台账与证据，向stdout生成SHA-256源码指纹及验收矩阵；不写文件、不执行构建/SSH/GUI、不改变台账。历史语句原样列入报告，指纹仅标识当前源码，不能当作测试二进制指纹或验收成功。再次改代码后必须重新运行生成，不复用此前快照。

本轮工具验证：node --check通过；JSON与Markdown输出均成功。JSON含13个源码指纹、8行矩阵，readOnly=true/runsTests=false/acceptancePassed=false。本次failed缓存修复后forward.rs SHA-256为 `4eba84ca86eb610d496dd224928f659bc494d166ca7b3fc5e3197a803c0e43cd`（37985字节），该指纹没有对应本轮新测试运行结果；主线程后续修复会使它过期。

## 只读盘点：隔离SSH夹具可行性

主线程随后报告最新forward19项已通过。以下仅执行Get-Command、指定文件存在检查、Get-Service和Python `-B` 的模块存在查询；没有生成密钥、创建临时配置、启动/停止服务、连接SSH或修改known_hosts。

现有工具：Windows系统OpenSSH的ssh.exe、sshd.exe、ssh-keygen.exe、ssh-keyscan.exe均存在；sshd_config_default、sftp-server.exe存在。已有系统sshd服务处于Running/Automatic，应视为用户环境，绝不连接其端口22、重启或改其配置。Node、Python3.12、wsl.exe存在；Docker未发现。wsl.exe存在不代表发行版已安装或可用，本轮未启动WSL。extension-host/dist/host.js存在。Python现有cryptography存在，paramiko和asyncssh未安装；本轮不安装依赖。

结论：工具层面具备尝试owned独立sshd的条件，不需要用户主机地址、用户私钥或模型配置。**尚不能证明**当前Windows进程令牌允许独立sshd正常启动/认证，必须由下一轮受控预检确认；如果需要修改系统服务、用户账户、授予权限或安装组件，停止该分支，不通过改变用户环境绕过。密码/认证token绝不借用用户配置。

### 最小验收方案（尚未执行）

1. 创建唯一owned临时目录，里面独立生成hostA、hostB、client测试密钥；sshd配置绝对路径显式HostKey、AuthorizedKeysFile、PidFile、日志；仅ListenAddress 127.0.0.1和独立随机高端口，不用默认配置/系统host key。服务以owned子进程前台运行，记录PID与退出码，只终止自己创建的child，不用全局taskkill或服务管理。
2. SSH client传入`-F`独立空配置、UserKnownHostsFile和GlobalKnownHostsFile均为夹具文件、IdentitiesOnly=yes、IdentityFile为夹具client key、IdentityAgent=none、BatchMode=yes、ConnectTimeout=短值、StrictHostKeyChecking=accept-new；命令参数固定，不使用用户ssh config/agent/默认密钥。无法控制非fixture已绑定端口时换端口重新建夹具，不能把其他本地服务当作owned SSH。
3. hostA首次连接正向：成功建立SSH握手并记录fixture known_hosts；正向若只有认证拒绝，则仅证明host key策略/握手，不能宣称远端执行或重连写入通过。认证必须通过独立AuthorizedKeysFile与当前允许用户的临时公钥，且不用用户authorized_keys/密码；Windows权限不满足则保持该项未验收。
4. 停止owned hostA，同一个fixture端口使用hostB；known_hosts保持A记录。accept-new客户端必须拒绝hostB变化，并确认fixture副作用文件没有新增；换独立空known_hosts允许首次B，作为反向对照。既有用户known_hosts全程不读不写。
5. 恢复hostA并保留A known_hosts，若owned认证/exec确实可用，运行夹具可计数写入：发起A连接并中断owned transport，确认上层结果明确失败/不确定、没有自动重放；显式新attempt重连写入后计数只增加一次。准确区分切断SSH child（进程断连）与网络黑洞（网络故障）；本方案不改防火墙/路由，不能把前者描述成后者。
6. host key场景可优先独立完成；要验证AgentZ真实远程deploy/files/LSP/terminal旅程还需要真实POSIX目标。Windowssshd认证成功不保证`sh -c`可用；当前不借WSL默认目标或现有sshd替代。只通过SSH客户端夹具不可自动勾选AZ-045整体验收。

下一步可无需用户输入尝试fixture配置预检与owned启动；本轮授权范围明确禁止启动服务，故仅提交方案。若前台sshd因权限退出，记录失败证据，继续其他已授权工程，不先要求用户提供真实主机或更改系统环境。
