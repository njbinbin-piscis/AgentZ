# AZ-018 本地单Agent候选冻结复核

2026-10-08，两个Codex子代理独立只读复核与主线程现有39前端/88Rust/strictClippy及真实SQLite夹具证据收尾。未发现限定当前候选新增必须修复P1；停止继续扩性能scope，转当前源码构建预检。原criterion为重复事件幂等、断流补齐、其他任务隔离，本地普通单Agent/currentAttempt代码候选覆盖，不是整任务done。

剩余明确功能限制：remote/team/pool/delegated与绕过collector的直接sink未覆盖；大于5000事件/8MiB预览拒绝而非任意长历史浏览；旧v1无event不可补造；当前Attempt预览非完整task对话；静止前台丢通知只由manual/focus/online/busy刷新补齐；同流增量不重验证旧payload。同步commit成本和前端prefix复制仍存在，非SLA。真实GUI断流/重启/迟到通知、模型磁盘失败停止、中文长任务预算及备份恢复仍未跑。

当前前端production build56580在途；源码closure及离线输入预检66172在途。旧target/release/candidate-0.9.7安装器仍是先前源码，不包含本轮事件/计划改动；预检若通过只说明输入和旧产物可检查，不能证明新构建或安装验收。分发授权/签名/原生安装仍未确认；不发布、不对外发送。
