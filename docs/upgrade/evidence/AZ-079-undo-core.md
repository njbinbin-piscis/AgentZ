# AZ-079 Undo All 核心与补丁复核

2026-10-05，Codex。核对当前台账、cycle032及构建，未发现适用AGENTS.md，保留AZ-029原三文件修改；主实现仅AZ-079，GUI按用户要求跳过但不计通过。

首轮3/3回归通过。补齐已undone快照不得参与恢复后，最终cargo test -p piscis-kernel --lib undo_tests 4/4通过（0.08秒，331项未运行），日志.cache/upgrade-undo-cycle-033.log；session82477退出0。验证真实临时文件字节、先新建后编辑恢复不存在、失败记录/其他session隔离、已撤销排除与重复撤销不重放。当前FileJournal没有独立单快照Undo API，未声称运行不存在入口；按回合撤销接口未改。

独立补丁docs/upgrade/patches/AZ-079-v0.8.68.patch，SHA256 25fa90ddd5018349666688d1b51f3cc323c98413bd2edad007e183ee30886414。提取固定527e554基线至.cache临时夹具。首次夹具未建立独立Git仓库，git apply跳过非当前目录路径，字节及规范化内容比较失败。建立独立Git仓库后重新apply --check、实际apply、reverse --check通过；规范化CRLF为LF后源码一致，逐字节比较仍未通过。前次失败保留，不归因于单纯行尾差异。

尚未完成AgentZ宿主候选实际hook集成、可分发固定依赖接入及产品Undo验收。AgentZ仍固定远端v0.8.68，不能将本地内核回归当作生产已修复。AZ-079保持doing，后续进行候选宿主回归；未发布、部署或对外发送。
