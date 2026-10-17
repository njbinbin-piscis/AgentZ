# AZ-009 当前工作区候选运行入口

2026-10-05，Codex。遵循ADR-003，不新开doing；82任务、14验收、4review、1blocked。读取适用指令与前轮证据，未发现AGENTS.md，未覆盖已有源码。

上轮串行构建退出0：npm run build（Vite 6m24s，有大chunk提示），cargo build -p agentz-desktop（7m52s）。日志`.cache/upgrade-native-candidate-build.log`；旧exec session84206已结束。候选为`C:\Projects\AgentZ\target\debug\agentz-desktop.exe`，修改时间2026-10-05 16:25:43。

使用现有AGENTZ_CONFIG_DIR启动候选，隔离目录`C:\Projects\AgentZ\.cache\native-review\config`，未复制用户凭据；安装版仍运行，未关闭或覆盖。候选PID21424（后续应重新核对路径），sky返回候选窗口id5834604。调试版devUrl首次ERR_CONNECTION_REFUSED，启动仅localhost:5273的Vite服务后刷新，服务exec session66369，日志`.cache/upgrade-native-vite.log`。独立配置生成日志/空数据库/seed资源，前端运行日志可见ExtensionHostProvider无项目状态。

初次node_repl窗口筛选表达式语法错误已修正；未计为验收。元素刷新首次geometry unavailable，重新激活与观察后旧索引不可用，停止沿用索引并重新观察。候选标题已切为AgentZ，但可访问性树仅有框架；截图捕获仍报`FrameArrived timed out: timed out waiting on channel`。未打开测试项目、未创建A/B任务、未发模型请求或修改用户数据。

下一轮先检查现有候选与Vite，避免重建/重复启动。原生页面输入入口仍未完成；A/B交叉续聊、重启文件证据、评审与审批点击均未验收，相关review不关闭。不发布、部署或对外发送。
