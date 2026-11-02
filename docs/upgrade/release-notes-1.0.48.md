# AgentZ 1.0.48 发布说明

发布日期：2026-11-03。完整变更见 `CHANGELOG.md` 的 1.0.48 条目。

## 新增

- **CodeZ 调试 Python、Go、Node.js。** 调试面板新增配置下拉和启动按钮。配置来自 `.vscode/launch.json`（允许注释和尾逗号），也来自自动识别的模板：Python 的 Flask、FastAPI、Django 和当前文件；Go 的主包、当前文件和包测试；Node.js 的包入口和当前文件。Python 用项目虚拟环境，Go 用 Delve（`dlv`），Node.js 用 js-debug，首次使用时下载一次。
- **更完整的调试会话。** 运行中增删断点立即生效，停住时自动打开文件并高亮当前行，变量可展开，监视表达式每次停住重新计算，调试控制台可在暂停帧中求值。

## 修复

- **TCP 方式的调试适配器。** Delve 和 js-debug 监听端口而不是标准输入输出。调试通道现在能连接这类适配器，并接受 js-debug 为每个调试目标开的额外连接，原有的会话隔离保持不变。

## 内核

- 内核仍为 piscis-engine `v0.8.72`。
