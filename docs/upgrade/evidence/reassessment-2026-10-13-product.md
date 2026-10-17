# 2026-10-13 产品重新评估（AZ027–048）

审查者：Codex 子代理，源码与证据只读交叉复核，不是独立人类批准者。逐任务结论见同名JSON，共22项。未修改产品、台账、旧证据，未运行测试、Cargo、GUI、模型或真实账号；主线程运行中结果未提前计为通过。

## 范围

读取当前plan原scope/acceptance/verification、2026-10-04产品成熟度审查F06–F12与跨视图承诺；检查各任务关键源码控制路径、入口及fixture声明，非所有代码形式化审计。JSON列 inspected_files、实现状态、verified_state、severity、conclusion_proposal、remaining_work。历史pass均不代表当前pass。

37 done与M1 ADR007风险接受是既有治理记录，本文不自行推翻Gate。ADR008 GUI豁免可以表示接受风险，不能改称真实界面通过。所有参与者仍Codex，代码交叉复核不等于独立人类批准。

## 当前具体问题与实装边界

**AZ048 / P1：丢catalog可能退回旧根。** project_scope::projectless_session_directory在catalog.sqlite不存在时直接Ok(None)；chat_turn2141之后free-session解析None使用原workspace_root。若绑定DB/owned目录还在而catalog被移走，会把已绑定任务误当从未初始化legacy。应依据现有binding/初始化证据失败关闭，补丢catalog真实fixture与主turn cwd回归。当前chat_turn已新增目录解析，旧“host完全没有接线”描述过时；WorkZ仍未发现createProjectless调用，archive/restore未实现。

**AZ036 / P1：同Task操作路由未闭合。** TaskView返回observation_only、TaskFileViewer有界只读；App openProjectInCodeZ仍switchProject(dir)，不能保证editor/terminal/debug/preview保留原authority/project/session/task。必须建立操作scope再接四类面板，保留dirty和活动terminal预检。

**AZ035 / P1：聚合不完整。** runtime TaskDetails仍events Unknown、artifacts Unsupported；独立artifact API/面板存在不代表统一DTO引用已接入。atomic_snapshot=false应保留。

权限/审批/加密/技能来源/Windows AppContainer已有实质实现；不代表六类provider、reference-only全部秘密出口或所有平台沙箱。AZ031可用合成秘密推进，不需要真实账号才开工。AZ032生产nonce与真实Tauri正反测试仍缺。AZ042closure文档“每小时10次”与源码实际5分钟10次不同；真实TS/Rust服务异常未执行。AZ043/044子进程防线不证明所有debuggee/远程/breakaway树已回收。

产品层当前主要是观察能力：详情、observer预算、文本版本、被动交付、输入摘要、buffer身份、LSP/DAP/PTY生命周期。AZ037交接、AZ039版本验收回执、AZ046六扩展档案尚需独立实现/验收，不能用新增展示代替。

## 源码指纹漂移

实际SHA与plan implementation_review.sources对照发现：
- AZ035：task_details.rs领域与runtime、run.rs、budget.rs、runtime/mod.rs、project_scope.rs、lib.rs、AssistantPanel.tsx。
- AZ036：App.tsx、en/zh.ts、CodeZ index.tsx、commands/mod.rs、lib.rs。
- AZ038：artifacts.rs、task_artifacts.rs、commands/mod.rs、lib.rs。
- AZ039/AZ040：AssistantPanel.tsx。
- AZ041：CodeZ index.tsx。
- AZ044：Terminal.tsx、ide.rs、src-tauri/Cargo.toml。
- AZ047：BottomPanel.tsx、Terminal.tsx。
- AZ048：projectless.rs、project_scope.rs、lib.rs。

这些是不匹配原登记review的修改，不能由git dirty状态断言哪位agent编辑或未经任何其他复核。需重新关联最新diff、评审与实际结果；不能只更新hash绕过检查。AZ042/043本轮登记source SHA一致只说明字节一致，不代表native通过。AZ027–034/045/046无implementation_review sourcepin，不能把无比较对象当未修改。新macOS/Linux秘密provider需独立来源与平台验证。

## 更新与执行顺序

1. 将漂移、AZ048失败关闭及AZ035/036范围缺口加入现有任务remaining_work，保留原验收与历史。
2. 先修AZ048丢catalog退回，随后复核共享接线并由主线程重跑当前领域/host/client。
3. AZ036同Task操作scope→AZ037交接；AZ038文本版本限定复核；AZ039版本回执/三旅程；AZ040/047代表走查。
4. 分别保留真实Webview、TS/Rust服务故障、native DAP、PTY、可认证owned SSH、六扩展档案验收。豁免不等于通过，不虚构非Codexactor。
5. 源码review+实际结果+指纹齐备后冻结候选；本审查不发布、不创建PR、不宣布83项完成。
