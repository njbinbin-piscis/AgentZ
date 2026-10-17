# AZ-009 原生核对入口尝试

2026-10-05，Codex。读取revision3台账、ADR-003与revision-3-progress：82任务、14验收、4 review、1 blocked。遵循当前周期不新开doing的收束，未修改实现或覆盖已有工作。

应用computer-use技能，完整读取SKILL.md及guidance/confirmations，通过node_repl导入@oai/sky。list_apps返回唯一AgentZ窗口，应用id com.agentz.desktop，进程路径为 C:\Users\zzz\AppData\Local\AgentZ\agentz-desktop.exe。该安装二进制修改时间2026-10-04 12:47:58；工作区target/debug候选修改时间2026-10-05 11:41:21。未将安装版本当作本轮代码验收对象。

取得返回窗口后get_window_state失败：`FrameArrived timed out: timed out waiting on channel`。按技能恢复规则重新list_windows并筛选AgentZ，未获得唯一可用窗口，终止输入。未发送点击、键盘、文本；未关闭、重启或覆盖用户应用。未验证A/B续聊、重启恢复或原生评审文件列表，也未签署任何Gate。

本次是原生入口失败证据，不是验收通过。后续先取得工作区候选版本可捕获的原生窗口，再执行AZ-009与AZ-013；工具恢复前保持review，不把相邻表面写回AZ-028。
