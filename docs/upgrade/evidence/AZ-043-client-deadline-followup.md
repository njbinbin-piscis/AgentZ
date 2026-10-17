# AZ043 connect deadline followup

2026-10-10。独立复核发现原 client listen/start 无 deadline 后，补充 same-slice 防护；不改写先前实跑证据。

constructor 增加 connectTimeoutMs，默认 10000，允许 1..60000ms。listen 与 start 分别受该 deadline 约束；连接错误/超时撤销客户端监听及 pending，并最多 stopTimeoutMs 等待同 sessionId 的 stop。因此两阶段总等待最多 2×connectTimeoutMs 加清理等待，不宣称 OS 已终止。

真实 IPC promise 保留晚到 continuation：已超时 listen 的 UnlistenFn 到达后立即执行；已超时 start 到达后再次 stop 同 UUID。并非单独 Promise.race 后遗弃原结果。host 必须保留 stopped/unknown-stop UUID tombstone，才能阻止 stop 先于 delayed start 到达时新启动；该 host 防线由另一代理实现和主线程验证。

实跑第一次 9 fixtures 有 1 失败：旧 start-pending fixture 仅等待一轮 microtask，不足新增 phase helper 的异步层数，finish 尚未登记。修为 fake timers advance 0 等到真实 mock start 登记，未改变产品逻辑。补验实际 9 tests pass，37ms，总 3.54s。scoped ESLint exit 0。

新增 fixtures 证明 listen 永不响应仍有界失败，late listen unlisten；start 永不响应仍有界失败，late start 再次仅清自己的 ID。其余旧隔离/timeout/sendfail/dispose fixtures 保留。

source SHA256 `acb362b4ddbea5cf7eb8af2707463ae0897ae4109e7da896cb4f6b0fc904d509`；test SHA256 `f5dd7872dc2c8bef200daef4447522a38e9c505f6897deb25128e4c8c403e7d3`。未自行运行全量 TypeScript、Cargo、GUI/native adapter。
