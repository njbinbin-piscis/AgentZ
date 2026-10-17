# AZ-018 历史元数据健壮性与只读 session 授权接线

2026-10-08，主线程集成，task-state 与宿主元数据模块由两个 Codex 子代理独占实施；第三个代理只读复核。继续原 AZ-018 主任务，不提升正式完成数或 Gate。

Run load 与同任务健康检查拒绝负 updated_ms；全部当前与历史 Attempt 拒绝负 created_ms/updated_ms 和 created_ms > updated_ms。发现或读取健康当前 Attempt 不能掩盖同任务旧 Attempt 的损坏。保留 delivery 决策单独推进 Run 时间的合法行为，不要求 Run 与当前 Attempt updated_ms 相等，不进行数据修复。

existing_binding 改接专用 session_metadata::read_existing：只打开可信 canonical project 的既有 .agentz/piscis.db，READ_ONLY、单事务快照，严格读取授权字段类型，先检查 state_frame_json 字节长度不超过 1 MiB，再读取 JSON。缺库、缺 session、缺必需列、错误类型/未支持 source、损坏 WorkZ metadata 明确拒绝；不调用正常 kernel 初始化或迁移入口，不读取模型 Settings，不创建 session。沿用 CodeZ/legacy CodeZ/plain WorkZ 支持范围，opaque scope、环境租约、fresh 环境解析与查询前 scope 复验仍保留。

未注册 WorkZ 的 host free-project 路径核对改为 free_sessions_path（纯 join）及 try_exists/canonical 比较，避免 workz_overview 创建 free/scratch 目录或列出聊天会话的副作用。未存在的 free-dir 视为不匹配，普通 WorkZ 当前项目仍继续原绑定校验；IO 错误显式拒绝。

兼容限制：上游 session 数据库没有本模块可鉴定的 schema-version 契约，采用必需列与严格字段校验，不宣称自动检测未来 schema 或迁移旧库。CodeZ 保留无关 frame 内容不作为 WorkZ JSON 解释的行为，但其字段类型及长度也严格检查。SQLite WAL/共享缓存文件层面的操作不以 DB 主文件字节核对证明完全 filesystem-immutable。完整聊天流、GUI/模型断流、跨平台/安装/回滚验收仍未完成，现有 0.9.7 安装器不包含本轮新源码。

实际检查结果由主线程完成后追加；代理只运行格式及 diff 检查，不将夹具编写等同测试通过。

独立复核发现另一保留边界：环境登记 agentz_task_state::load 的既有 open(create=false) 仍使用读写连接，version0 的既有 task-environments.db 可以初始化 schema；后续 workspace/binding resolver 也复用此入口。本包仅承诺 session 授权查询与 Run/events 正文 READ_ONLY，不将整个 fresh binding 解析描述为无写入。环境登记专用只读查询及其 resolver 传递另包处理，避免本轮扩大共享迁移语义。

最终主线程实际验证：session17089 task-state 全量 80 项测试通过、0 失败（包括新增时间戳损坏与正常交付两个 fixture），all-targets 严格 Clippy exit0。session62366 registered desktop lib session_metadata 3 项、chat_plan 3 项、event_replay 6 项测试全部通过；编译与整个命令链 exit0。未修改前端，未重复此前通过的前端全量验证。只读独立代理复核：合法 CodeZ/plain WorkZ/交付时间行为限定通过，缺 free-dir 短路不创建目录，原 scope/binding 限制保留；旧缺列或错误类型/过大 frame 必须显式恢复。
