# AZ-028 直接 CodeZ 上下文（cycle 015）

日期：2026-10-05；Codex；AZ-028 仍是唯一主实现。

已读台账/进度和 cycle 014 证据；未发现适用 AGENTS.md，开始时无 cargo/rustc。保留用户及其他路线图修改。

主 runner 现在将 canonical_root 解析后的本地根写入后续 ToolContext，与审批使用同一根，避免保留可重新指向的输入别名。仅为本地路径一致性，不代表消除了文件系统 TOCTOU 或实现 OS sandbox；远程路径仍未验收。

无注册任务的 direct CodeZ 绑定现在包含 Git 仓库根/common-dir 与 symbolic HEAD；detached HEAD 使用提交身份。等待预览后切换分支将使绑定失效。非 Git 目录按祖先 .git marker 判定，不需要调用 Git；有 marker 时 Git 失败或超时直接拒绝，不回退为普通目录。仍允许同分支内正常源码编辑；此指纹不是逐文件源码快照。

新增真实 direct Git 分支切换、非 Git 预览后出现损坏 .git 两项回归。首轮 8/9：捕获损坏 marker 被当作非 Git 授权；其时运行的是修复前构建快照，失败日志保留在 `.cache/upgrade-approval-direct-cycle-015.log`。

停止修改后重建最新源码：宿主 integration 9/9，通过日志 `.cache/upgrade-approval-direct-cycle-015-final.log`。编译 3m32s，测试 4.76s。包括真实 managed/direct Git、SQLite、审批生命周期/journal、后台与取消、数据库丢失。构建已退出。台账 validate 与 git diff --check 通过（已有行尾提示）。未重跑前端或领域测试，二者本轮源码未修改。piscis-engine 仍仅有之前 AZ-029 三个修改文件，本轮未更改固定依赖。

这些测试手工驱动 hook，未运行真实模型、原生桌面、跨平台或回滚验收。原生 protected preview、其他 harness 授权覆盖、实际账户/profile/window 资源身份仍待评估，保持 doing。AZ-079 未修复，未发布、部署或对外发送。
