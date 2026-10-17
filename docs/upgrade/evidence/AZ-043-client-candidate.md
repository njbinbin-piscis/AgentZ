# AZ043 DAP client candidate

2026-10-10。限定前端会话隔离和有界请求生命周期，不代表完整调试器验收。

## 已实现

每个 one-shot `DapClient` 在监听/启动前生成 readonly crypto UUID。start/send/stop 都只携带该 sessionId；start 必须回复 running=true 且 session_id 相同。agentz:dap 只处理当前 UUID、合法 channel/string data，旧会话相同 DAP request_seq 无法完成新请求或退出新会话。

request 仅 running 状态发送，默认 10s deadline，可配置 1..60000ms。超时、发送失败、exit、dispose 均移除 pending 并清 timer。owned exit 仅通知一次。dispose 同步撤销监听/拒绝所有 pending，再等待最多默认 2s 的 owned stop；这只限制客户端等待，不证明适配器及所有子进程已终止。晚到 listen 注册立即 unlisten，晚到 start 完成再次只清理自身 ID，永不停止另一个会话。

现有 connect(command,args,cwd?,remoteDir?)、request<T>、onEvent/onLog、dispose 接口保留；新增 constructor options requestTimeoutMs/stopTimeoutMs。远程路径转换保留，此测试未执行 SSH 调试。

## 实跑

定向 Vitest：7 tests pass，50ms，总 12.04s。真实 mock invoke/listen 覆盖未响应超时、旧会话响应/退出、发送失败、owned EOF 清理、listen/start 迟到、stop 无响应有界等待、错误 start 身份。未用实现镜像代替测试。

两文件 scoped ESLint：exit 0。

源码 SHA256 `4e497d15dcb3c10c36a74841f1a2cf01d91f14f23d70c1ba376ae6fe24125878`；test SHA256 `90291c15650c730844795fd54c24d95d6e55f4fe9be004d048fb55ecc12d72cc`。

完整 TypeScript、宿主 Cargo、真实 adapter/GUI、远程连接及 OS cleanup 验收由主线程统一，本文未宣称这些通过。
