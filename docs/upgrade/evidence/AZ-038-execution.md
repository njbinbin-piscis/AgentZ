# AZ-038 实施记录

主实现从已冻结的 AZ-036 限定候选转入统一产物清单与版本。正式验收仍为25/83，未提升Gate。

当前产品artifact数组为工具参数/结果推断的路径线索，不能用作版本、所有权或新宿主权限。领域实现采用独立artifacts.db，显式初始化、只读查询不创建或迁移；保存真实不可变payload及捕获时任务/环境/绑定版本，旧内容从数据库读取而非重读live路径。初期payload上限1MiB，任务环境容量32MiB。网页初期仅源码文本，不执行富内容。

并行分工：parallel_plan仅artifacts.rs及真实SQLite fixtures；ssh_audit独立接线设计；remote_tests独立攻击/回归矩阵；root负责台账、宿主接线及串行构建。

独立复核已发现删除后相同version身份可复用的问题，要求保留tombstone/身份，拒绝不同内容复用。修复和测试未完成，不报告通过。

AZ-036限定候选源码与独立复核已冻结：18项前端、11项宿主通过；完整strict Clippy仍有已有lint，当前allow-listed Clippy通过。新功能尚未进入旧0.9.7安装器，未执行GUI/安装/回滚或跨平台验收。

## 2026-10-10 产物领域与宿主接线检查点

新增独立版本库、四个宿主命令及注册。真实最终SQLite领域6项测试通过（session90757，0.17s），包括旧内容保留、删除墓碑不可复用、AFTER触发器假成功拒绝、只读无初始化、绑定/损坏/大小边界。领域严格Clippy通过（session62880，exit0）。宿主commands::task_测试正在编译（session38483），不能报告通过。

独立审查完成领域、宿主接口、模块导出与等价安全读取helper；所有7个受影响旧台账源码指纹已原子刷新，validate83/11通过。正式验收未提升。

后续同一主任务继续客户端严格验证和版本查看/捕获/删除交互。产物删除不修改live源文件、保留版本身份，并不宣称安全擦除。操作commit后身份复验失败可能返回错误，前端应重新查询确认，不能假称回滚或盲重捕获。当前仅文本snapshot，ChangeSet分类不等于完整Git变更应用；原生GUI和存储备份恢复验收仍待补。

## 宿主与版本管理界面验证

宿主注册后12项task相关测试通过(session38483，3.76s)；新增真实Windows代码/报告文件读取到版本库存储fixture重跑2项通过(session57616，0.10s)，覆盖旧payload、跨task/environment、删除墓碑和live文件保留。此为受控函数集成，不等于原生IPC/GUI验收。宿主allow-listed Clippy通过(session34493，2m48s)；三个已有lint类别仍显式允许，不报告完整strict通过。

WorkZ已新增受当前canonical项目/session约束的版本入口，任务切换前清旧viewer。前端修复同步双击锁、未知commit强制fresh observation+list对账屏障；最终服务7+界面6共13项实际通过(session28610)。初次TSC发现测试node:crypto缺类型，已用独立SHA固定向量和真实WebCrypto修复；修后完整TSC session35442 exit0。后续空页游标修复触发最终TSC重跑7312仍待结果。

分页末页及删除后空页契约已统一：本页为空则next_cursor=null，ordering仍按请求cursor；宿主领域增加真实SQLite删除末项后空页fixture，最终7项正在84762验证，不能提前记通过。当前源码尚未进入现有0.9.7安装器。

最终验证：域7项session84762全部通过(0.12s)、最终domain严格Clippy4216 exit0；前端13项28610通过、最终完整TypeScript7312 exit0、最终新增文件ESLint41997 exit0。宿主当前2项native集成57616通过；allow-listed hostClippy34493通过（先于最后仅分页空页领域修正）。限定源码与7份独立审查冻结为AZ-038-implementation-manifest.json/review.md；不代表真实IPC、GUI、备份恢复、完整GitChangeSet或安装发行验收。
