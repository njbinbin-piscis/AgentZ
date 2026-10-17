# AZ-028 可信审批预览失败恢复（cycle 029）

日期：2026-10-05；Codex；主实现AZ-028。核对台账、cycle028证据及运行状态，无cargo/rustc/link构建；未发现适用AGENTS.md，保留已有修改。

可信预览IPC失败现在显示独立、按signalKey绑定的错误及Retry preview按钮。批准保持禁用，点击重试重新读取宿主快照，成功后才能使用快照digest提交。预览错误与决策投递错误分开，保留已有异步live检查以忽略切换/卸载后的旧读取结果，不恢复事件输入参数作为预览。

新增读取失败→批准禁用且无决策IPC→重试读取成功→错误清除→精确digest批准的组件回归。组件15项，前端72/72、10文件通过（3.75秒）；typecheck及定向ESLint退出0。日志 `.cache/upgrade-preview-retry-cycle-029.log`。台账validate和diff --check通过，仅已有行尾提示。未修改Rust，未重复宿主/领域库测试。

仍未验收真实原生IPC故障恢复、桌面渲染、真实模型、跨平台、回滚和OS隔离。AZ-028保持doing；未发布、部署或对外发送。
