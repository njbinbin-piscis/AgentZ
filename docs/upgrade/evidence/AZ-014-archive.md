# AZ-014 可恢复源码快照

2026-10-05。新增archive领域模块、archive_snapshot/restore_snapshot宿主命令及TypeScript封装。此阶段只保存/恢复，不调用worktree remove、branch delete，不自动改绑旧session；AZ-015负责替换旧清理入口。

快照：以持久task绑定验证真实Git common-dir、worktree及分支；独立GIT_INDEX_FILE，保留HEAD ancestry；所有现存跟踪文件及非忽略、非运行时新文件以原始字节存入Git对象，write-tree/commit-tree后用refs/agentz/archives/<id>固定，schema1元数据在主项目.agentz/archives原子发布。两次内容/路径/HEAD核对及末尾绑定/分支校验拒绝捕获期间变化。原用户index和HEAD不修改。

恢复：校验schema/归属/ref/commit/tree一致性，新checkout --no-checkout再写原始blob并逐文件核对，避免换行、clean/smudge过滤改写备份；既有目的路径拒绝。固定宿主生成的新目录，不接受客户端覆盖目标根。恢复目录为独立detached checkout，不冒认原任务的可执行环境，后续正式接手/绑定另行处理。

明确支持：普通源码文件、已有Git mode、提交祖先、暂存/未暂存最终内容、删除、改名、中文/空格路径、二进制与非忽略新文件。恢复最终文件内容，不宣称恢复原来的暂存分类/系统属性。symlink、submodule、目录型跟踪条目明确拒绝；不冒称跨平台已验收。

忽略策略：未跟踪的ignored文件及.agentz/.codez/.piscis运行时文件列入omitted_paths，不纳入本阶段源码快照；require_no_omissions拒绝其清理授权。保留原目录直到这些内容有独立保留策略。旧force discard尚未替换，不把这个guard冒充已接入旧入口；AZ-015必须在清理前重新验证当前遗漏、运行状态、身份和快照，而不能相信客户端给出的空遗漏列表。

失败保留：无效ID/重复ID拒绝；stale index/lock不删除；元数据发布失败保留可达快照ref并在错误中给出ref，不删除原文件。恢复失败保留新checkout供诊断；损坏/改指的ref拒绝恢复。

验证：真实私有Git仓库案例通过，核对已提交、暂存、未暂存、删除/改名、中文新增二进制的恢复字节；原HEAD、cached diff与原始index字节未变；原目录暂时移走后仍可从快照恢复；覆盖目的目录拒绝；ignored/runtime仍在原目录且清理guard拒绝；元数据发布故障保留ref与原内容，stale index保留，ref篡改拒绝恢复。宿主cargo check通过（1m38s），前端typecheck及7项评审组件回归通过。最终领域测试与clippy结果随周期记录补充。

复盘：初版GIT_INDEX_FILE使用Windows verbatim前缀，被Git for Windows拒绝，已在该环境变量上转换为常规/UNC路径；初始add -A方案会执行Git内容过滤，因此改为原始blob+独立index-info及原始恢复写入。没有把这些修复前失败记作通过。

未验收：原生IPC点击、归档恢复UI、历史任务重新绑定、runtime journal归档、真正清理。新源码命令不等于完整丢弃安全闭环完成。

最终领域测试：10项全部通过（17.92s），clippy --all-targets -D warnings通过（3.60s），包含最后增加的末尾绑定校验。按本项源码快照/恢复范围完成，审核为Codex自检；不是独立用户/QA验收。
