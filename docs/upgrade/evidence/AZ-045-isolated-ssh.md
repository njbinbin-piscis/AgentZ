# AZ-045 owned localhost SSH 主机密钥握手验收

日期：2026-10-07。执行者：Codex 子代理，主线程明确授权owned隔离夹具。此证据只覆盖真实OpenSSH主机密钥握手策略，不代表AZ-045整体验收。

## 实跑范围与结果

运行 `scripts/upgrade-ssh-fixture.ps1`，工具会话38445，exit 0，结果 `passed-host-key-handshake-only`。本地Windows系统OpenSSH ssh/sshd/keygen已存在，未安装任何组件。脚本SHA-256：`30134c58291e9c0988b9eae03e89a0e090451b6e27e682a7b7e9a83bea05250f`。

独占UUID临时目录 `agentz-ssh-fixture-c4e6a097a18d4838be25fb56a8e95471`；仅在127.0.0.1:50440启动自己持有的sshd Process（A PID28984，B PID35028）。sshd配置/HostKey/PidFile/AuthorizedKeysFile全部指向夹具；AllowUsers固定不存在的fixture用户名，authorized-keys为空，不借系统用户登录。客户端独立空`-F`配置、私有UserKnownHostsFile/GlobalKnownHostsFile、临时client key、IdentitiesOnly=yes、IdentityAgent=none、BatchMode=yes、ConnectTimeout=5、StrictHostKeyChecking=accept-new；没有读取用户SSH配置或默认known_hosts。

| 验证 | 实际结果 | 能证明的范围 |
| --- | --- | --- |
| fixture配置预检 | sshd -t exit0 | 独立配置可使用 |
| hostA首次握手 | client exit255，fixture known-a确实写入A公钥；stderr显示首次加入和Permission denied | accept-new允许首次主机密钥；认证未通过 |
| 同端口切换hostB、保留known-a | client exit255，明确REMOTE HOST IDENTIFICATION HAS CHANGED；known-a仍为A公钥 | 已知主机密钥改变被拒绝，不自动替换旧信任 |
| hostB独立空known-b正向对照 | client exit255，fixture known-b确实写入B公钥 | B服务可握手；变化拒绝不是B未启动导致；认证未通过 |
| owned资源清理 | 全部9个owned PID已不存在；UUID目录已不存在 | 临时私钥/known_hosts删除、owned父进程已退出 |
| 既有系统sshd | 运行前后均Running，未对它调用start/stop/connect或改配置 | 只旁路检查状态，无服务修改 |

前三个策略检查均true：firstAcceptWritesOwnedKnownHosts、changedKeyRejected、changedKeyDidNotReplaceA；freshBPositiveControl=true。firstAAuthenticationDenied=true。脚本输出authenticatedRemoteExecutionVerified=false、reconnectWriteVerified=false。

## 进程与stderr指纹

指纹基于.NET读取到的stderr文本按UTF-8编码，不是原始管道字节。所有9项stdout为空，stdout SHA-256均为 `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。无私钥内容输出。

| 名称 | PID | Exit | stderr UTF-8 SHA-256 |
| --- | --- | --- | --- |
| keygen-host-a | 28524 | 0 | e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 |
| keygen-host-b | 23192 | 0 | e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 |
| keygen-client | 34992 | 0 | e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 |
| sshd-config-preflight | 27080 | 0 | e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 |
| first-a-accept-new | 30816 | 255 | 7cda02a88a3440cf052f7c8f95ec08fa4e5834f8d16e913323589495336dc344 |
| sshd-a-stop | 28984 | -1 | 9499d0149f5de54a247c9eef351024f2ec9e5784636dd7c79cb7f57e30817a4f |
| changed-b-rejected | 32104 | 255 | 9a69e4fe932d2717defe9d3f3a14d2027d1521b5aa4e66ee777eab5ff1291f23 |
| fresh-b-accept-new-control | 35372 | 255 | 7cda02a88a3440cf052f7c8f95ec08fa4e5834f8d16e913323589495336dc344 |
| sshd-b-stop | 35028 | -1 | 2f9e30c788dfa73fd0933af464fc7b30f73af2c722b0f6ac13392db2488ac9e1 |

两个sshd -1是脚本主动终止自身Process的结果，非握手失败。所有timedOut=false。日志明确显示A/B listener仅127.0.0.1:50440，A与B新known对照都因fixture用户名无效被拒绝认证，B旧known测试则在preauth握手后因主机密钥变化被客户端拒绝。

## 未跑与后续边界

- 未认证成功、未执行远程命令，因此不计exec、真实SSH文件写入、重连零重复写验收。
- 未执行AgentZ Rust RemoteTarget/ExtHost完整调用，只用同类型OpenSSH客户端策略参数加隔离配置。它证明底层host-key策略正反行为，不能证明每个产品入口都已统一或正确暴露错误。
- 未网络黑洞/防火墙/路由故障注入，未真实远程POSIX服务器、桌面GUI、模型、跨平台或回滚验收。
- 没有更改系统sshd服务、系统用户、权限、防火墙、用户SSH配置、用户known_hosts；临时host/client私钥已删。没有连接用户主机或默认本地22端口。
- 下一步如验证实际写入重连，需另行设计不借用户认证的owned远端执行夹具，不能把本次认证失败握手升格为通过。
