# AZ-042 语言服务恢复候选

2026-10-06。主实现任务，preparing；M1 Gate仍未通过。

入口核查：桌面LSP转发到vendored piscis-ide-tools。agent工具的manager已在死进程后按需respawn，但新client没有重放其他已打开文档；stdio诊断缓存退出后仍可被旧引用读取。Monaco WebSocket client断开时原来只清pending，保留诊断；旧socket延迟回调可能清空新socket或污染诊断。WebSocket桥仅服务单一连接，恢复还需重新申请桥端口，不能只复用旧port。

本批修复前端socket归属检查：旧消息/error/close不能操作替代socket；当前socket断开清诊断并向现有订阅广播空数组，清除陈旧marker；握手前断开拒绝connect。保留订阅用于后续恢复。新增崩溃→重连→旧回调→新诊断夹具。

实际验证：`npm test -- src/services/tauri/lsp.test.ts`，13 passed、0 failed；包含新增归属/陈旧诊断用例。TypeScript noEmit检查另行运行，尚未读到结果。

本机可调用rust-analyzer 1.99.0（b940084d 2026-09-28）及typescript-language-server 6.0.1。版本查询只证明程序可运行，未执行真实服务异常旅程。下一步实现健康状态与有界重启、桥端口重建、文档重同步及诊断世代校验；以独立测试项目验证TS/Rust崩溃恢复，不能杀现有用户语言服务或把模拟WebSocket当作桌面验收。

后续增加同一LspClient在新桥端口reconnect，保留现有provider/诊断订阅，重放尚打开文档的最新未保存文本并重置LSP版本为1；关闭文档不重放，离线编辑更新本地文本快照。当前Session pool仍未自动调用恢复，不能宣称自动重启已实现。

最终前端LSP+Session池专项19 passed、0 failed（14+5）；新增新端口、最新未保存文本、关闭文件排除夹具通过。旧tsc会话3167已不可读取，不能声称通过；重新核查进程后启动noEmit验证，结果待确认。Rust stdio客户端、真实TS/Rust进程异常验收仍未实施。

Session池现接入断线自动恢复：保留同一client及provider引用，重新向宿主申请桥端口；100/200/400ms退避，每个Session累计最多3次，避免连续崩溃无限重启。首次握手失败不启动后台恢复；失败Session移除时关闭client。最后引用释放阻止延迟重启，恢复中宿主start返回后再次检查disposed，不能让已释放Session重新连上。

最终专项18018：20 passed、0 failed（14+6）；新增断线新端口恢复、共享client身份保持、最后消费者释放后不再重启通过。重试耗尽/多次连续崩溃、状态UI、真实进程旅程仍未验证。noEmit第二会话27844也已不可读取；当前77674检查输出与exit保存到.cache/az042-tsc.log及az042-tsc-exit.txt，以免会话失效丢证据，结果仍待读取。

持久noEmit记录明确失败（exit 2）：新增测试使用Array.at而项目target不支持。已改为数组索引，保留既有TypeScript目标不变；58042重跑日志/exit正在更新，未读取最终结果。

push诊断增加文档版本过滤：服务器提供version且本地有版本时，仅接受匹配当前版本的诊断；没有version的服务仍兼容，不能宣称能判断其所有迟到诊断。新增旧版本1/当前版本2断言；55801前端专项21 passed、0 failed（15+6）。补充Session最多3次失败重启后停止的夹具，87557最终专项结果待读取。真实TS/Rust异常流程、stdio缓存失效及健康状态呈现仍待完成。

87557最终22 passed、0 failed；持久noEmit重跑exit 0、空错误日志，类型检查通过。

本批修改vendored stdio客户端：读循环结束清诊断缓存并唤醒等待者；已死亡client不返回缓存/pull结果。新增两个显式ignored真实服务夹具，使用独立临时Cargo/TS项目、仅kill其持有的Child，再由实际manager重新spawn、sync_file和请求documentSymbol，检查旧client诊断失效。模拟旧诊断用于验证缓存，不把该注入视作服务器真实诊断生成。34680基础LSP构建仍在运行，最新夹具尚未执行；后续必须显式--ignored执行才可计真实服务异常证据。Monaco桌面恢复和打开多文档重放仍不能用这些stdio夹具替代。

2026-10-07人工继续：34680最终基础2 passed、真实用例2 ignored。显式51948真实异常测试：Rust passed，TS failed，初始化错误为临时项目缺少TypeScript安装，不能把版本查询当作可用工作区。夹具改为将仓库已安装TypeScript 5.9.3复制到独立临时项目，不安装/下载依赖、不修改用户项目。68449重跑构建中，最后补充的固定manifest相对依赖路径还需确认实际测试源。

同时实现stdio文档重放：manager保留已死亡client直到替代client成功，重新didOpen已同步文档并将版本重置1，仅迁移文档文本/语言，不迁移旧诊断；重放失败保留旧文档证据供下一次调用重试。真实夹具新增替代client返回前已重放的断言。该变更尚未得到完整重跑结论，不宣称真实TS恢复通过。

68449旧cwd路径失败后14775确认依赖路径修复，但TS在kill后等待EOF超时：Windows npm.cmd包装进程退出后Node后代仍持有stdout。Rust真实重放继续通过。真实夹具改为Windows仅对持有Child的PID执行taskkill /T /F，模拟完整服务进程树退出；30893重跑中。此修复不证明生产stop已经能结束完整进程树，需继续核查生产生命周期与本轮夹具残留。

30893：TS真实恢复通过；Rust夹具taskkill状态失败。改为Rust沿用此前验证的Child.kill，TS仅终止fixture拥有的wrapper进程树。最终21307显式ignored执行：2 passed、0 failed、0 ignored，真实TS与Rust均完成退出→manager重启→文档自动didOpen→documentSymbol恢复，旧client不返回诊断。读取CIM中含agentz-lsp-recovery临时路径的Node进程为空；此检查只覆盖带fixture路径的进程，不声称全系统无孤儿进程。

生产Windows包装进程与子进程的可靠树退出仍待修复；上述测试TS用了显式taskkill注入完整退出，不能冒充生产stop已验收。健康状态呈现、stdio过期诊断版本校验、多文档真实恢复和Monaco桌面旅程仍待完成。台账83任务/11里程碑validate通过，AZ-042继续preparing。

2026-10-07后续生产启动修复：识别已知Windows npm TypeScript .cmd布局（全局npm目录、项目node_modules/.bin），定位真实cli.mjs与Node，stdio与Monaco桥均直接启动Node。未知包装布局保持原解析行为，不能声称所有Windows工具树退出已解决；桥入口现在也按项目本地优先解析服务。真实夹具移除TS专用taskkill，统一仅对拥有的Child调用kill。

49297最终显式真实服务异常恢复2 passed、0 failed、0 ignored，TS使用新生产启动路径并通过Child.kill→EOF→manager重启→文档重放→符号恢复；Rust亦通过。严格piscis-ide-tools全target Clippy61960正在运行；此验证不含真实Monaco WebSocket桥、未知npm/pnpm包装布局、跨平台或发行验收。

61960严格Clippy最终通过。后续stdio push诊断增加文档版本校验，有version时拒绝与当前文档版本不符的结果；文件内容更新时清除该URI旧缓存。无version服务仍不能可靠判定其所有迟到结果。

17362最新显式真实TS/Rust恢复2 passed、0 failed、0 ignored，均覆盖两个已打开文档在重启后未手动重开即可查询符号，并检查旧版本诊断注入被拒绝。注入证明本地过滤，不冒充服务主动发送迟到诊断。71593最新严格Clippy运行中。

健康呈现下一入口为CodeEditor.tsx共享Session effect：已有tab generation守卫，当前服务不可用只console.debug；需要订阅连接/恢复耗尽状态并随tab释放清理，避免旧tab状态污染。真实Monaco窗口验收仍保持未执行。

本批LspClient提供idle/connecting/ready/recovering/unavailable健康订阅，Session退避重连与耗尽同步更新状态。CodeEditor按当前tab generation订阅并展示连接中/恢复中/不可用提示，释放时先退订；不显示底层命令或内部重试计数。45067基础前端22通过；补充恢复成功ready与耗尽unavailable断言后76412最终专项待读。75381最新noEmit输出/exit仍在.cache/az042-tsc日志中更新，不能拿上轮exit0当本轮结论。真实桌面提示布局与tab切换旅程未运行。

76412健康专项22通过，75381持久noEmit exit0通过。后续诊断pull增加socket/文档版本检查，编辑期间迟到pull不返回旧结果，编辑主动清旧缓存与marker；9359专项23通过。stdio同样拒绝pull等待期间文档版本变化后的结果，83124严格全target Clippy通过。

回顾恢复失败路径发现握手失败留下OPEN socket/已拒绝connectPromise，可能阻止真正重连；已统一失败清理，关闭旧socket、清连接/pending并发布不可用事件。新增失败握手后新socket成功夹具，20182最终专项待确认。81579 noEmit运行源可能早于最后失败清理改动，最终类型检查需重新固定结果。AZ-042-source-manifest.json已创建候选文件hash，最新修改后需要刷新；该清单不证明发行或固定上游等价。

## 候选交付阶段结论

20182最终前端24 passed、0 failed；46040最终noEmit持久exit0、错误日志为空。候选源hash已刷新并核对。代码阶段已交付有界桥重建、未保存文档重放、健康状态提示、旧socket/版本/pull结果拒绝、stdio重启/双文档重放及已知Windows TS包装层修复。实际证据分别为前端模拟、严格Rust Clippy、Windows真实TS/Rust stdio异常旅程，不混为桌面端到端验收。

AZ-042保留preparing等待M1 Gate及Monaco窗口、提示布局/切换与未知wrapper/跨平台补验；不计已验收，不开放其依赖任务。当前没有继续运行的本任务构建。下一主要实现切换到依赖AZ-008已完成的独立AZ-045远程断连与恢复候选。
