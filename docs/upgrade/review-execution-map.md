# 工程审查发现与现有路线图执行映射

2026-10-09。依据《AgentZ 产品成熟度与工程审查》F01–F12和当前83项台账制定本轮对应执行安排，继续使用已有任务ID、验收标准与依赖，不另建重复路线图。本文件是版本执行视图，不改变plan.json状态或Gate。读取时台账为24 done、11 preparing、5 review、42 pending、1 blocked；数字仅代表任务验收，不能代表成熟度或发布许可。动态状态以脚本台账为准。

## 发现映射与当前证据边界

| 发现 | 已有对应任务 | 已交付或候选依据 | 必须保留的缺口 |
|---|---|---|---|
| F01 环境串用 P0 | AZ007–009、017、036–037 | 007/008 done；009 review；任务登记、后端可信解析，见evidence/AZ-007-registry.md及AZ008/009台账证据 | A/B切换、运行中切换和重启原生旅程未替代；009不能因核心夹具通过关闭 |
| F02 评审不完整 P0 | AZ011、013、038–039 | 011 done、013 review；完整变更集/二进制/错误反馈候选，见evidence/AZ-013-parallel-review.md | 双视图原生评审交付验收，晚回包与异常结果；不能只以分支提交diff证明完整改动 |
| F03 journal根错位 P1 | AZ012、035、079 | 012/079 done；统一环境journal与完整Undo；035 preparing，见evidence/AZ-035-host-readonly-integration.md | 035统一源只读候选不等于产物清单已接；产物Unsupported、事件Unknown明确展示 |
| F04 清理无恢复 P1 | AZ014–015、019、021–022、063 | 014/015 done；快照/登记对象/分步失败；019/021 preparing | 活跃工具真正结束再清理、真实恢复/回滚；父进程锁空闲不能推断子进程或远程副作用停止 |
| F05 创建合并隐式副作用 P1 | AZ010、016、041 | 010/016 done；明确起点和事务预检，见evidence/AZ-016-merge-transaction.md | 原生脏缓冲/外部写冲突及环境切换验收；不把候选依赖放行当M1 Gate通过 |
| F06 隔离宣传与执行边界 P1 | AZ027–028、034、040、044、054–055、080–083 | 027/028/034/080–083 done；受限Windows策略、权限绑定及路径守卫 | worktree仍不是通用系统沙箱；remote/team/desktop/外发权限需分别实现验收，准确披露 |
| F07 Webview CSP/隔离 P1 | AZ032、046、056 | 032 review；资源/消息作用域候选，见evidence/AZ-032-parallel-review.md | 原生Tauri能够渲染/不能触宿主DOM、IPC、其它面板正反验收已后移，不能标done |
| F08 凭据加密失败降级 P1 | AZ029–031、061、077–078 | 029/030 done；Windows密钥保护与加密失败关闭；077/078 review | secret reference和所有脱敏出口031 pending；macOS/Linux真实凭据环境未替代 |
| F09 非耐久运行/续聊清计划 P1 | AZ017–026、028、035、070–074 | 017–021/023–024/035 preparing；Run、事件、计划、取消、预算候选。见AZ020 implementation-review及AZ024 reservation-core-review | 019可信owner证明、021取消确认、022恢复契约、025外部动作对账、024宿主全请求预算仍缺；不自动恢复执行 |
| F10 大组件/分散状态 P2 | AZ035–041、047、059–060 | 035共享详情DTO/两视图被动摘要候选；existing sources独立版本，atomic_snapshot=false | 交接/产物/脏缓冲/加载与长任务资源治理后续；不为拆组件重写所有既有路径 |
| F11 质量门禁不能证明旅程 P1 | AZ003–004、056–058、062、075 | 003 done；04 blocked；隔离夹具、独立review、源码SHA和实际执行证据已建立 | 真实模型成本/成功率、native E2E、长时性能、跨平台资源仍需单独验收，无数据不编造SLA |
| F12 发布和文档治理 P1–P2 | AZ005–006、026、063–069 | 005/006 done；064 preparing，见evidence/current-release-scope.md与本地产物预检 | 最新源码与已有0.9.7包不同；签名/安装卸载/数据恢复/更新/分发/稳定版决策未完成 |

上表证据按现有台账关联，原审查的行号属于2026-10-04基线，不能拿旧行号作当前缺陷仍存在的证明。状态done表示既定任务证据通过，不保证所有相邻用户旅程已验。正式review/done与milestone Gate仍执行原依赖；ADR006的限定implementation_review只允许指定下游开始代码，不放行验收。

## 本轮版本优先执行包

| 顺序/追踪ID | 原子工作与文件归属 | 可运行验证/完成条件 | 失败与复盘记录 |
|---|---|---|---|
| 1 / AZ021 | 主线程chat_turn/durable_run接线；内核代理worker receipt；host取消service独立review。修复晚Stop旗标绕过drain，统一requested/confirmed决定终态和lifecycle；未覆盖工具/remote不得由空registry推断全资源结束 | 实际核心receipt、host取消与真实本地shell/code_run故障夹具；同Attempt、队列、超时、延迟清理、guard/slot保持、Cancelled必须有确认，未确认Interrupted | AZ-021-host-cancellation-review.md列P2；失败保留原日志/源码SHA，修复再测，不把cancel=true等同已回收 |
| 2 / AZ035 | 完整Plan只读opener、纯聚合/host授权IPC、共享前端服务与两视图摘要；各独占文件，根统一接线 | 已有plan16/host4实际通过由根登记；前端严格DTO与late-response/focus pending复测。无CREATE/repair、Cancelled items不丢、unknown不变0 | AZ-035-panel-interaction-review.md焦点在途刷新丢失必须补coalesced pending fixture；fixture hook返回mock导致cleanup异常已辨明，保留失败及修复证据 |
| 3 / AZ019 | 限定本地显式对账核心与host owner方案收敛；旧无owner记录保持Unverifiable | SQLite CAS状态+事件同提交；OS跨进程锁/epoch/queue原子登记仅在实现并实跑后可消费；工具子进程未containment不能ConfirmedGone | AZ-019-owner-lock-review.md保留MSRV/锁库/Windows文件生命周期和副作用证明边界；不以PID/TTL猜死 |
| 4 / AZ023–024 | 冻结已review重试与预算核心，确认源码/隔离补丁/source manifest闭环 | 真实loopback HTTP矩阵及预算SQLite累计/单次permit/未知保留/rollback实际结果；候选scope准确 | 不在新版本前临时接全模型/成员费用硬限；observer与reservation核心不能宣传host收费控制已执行 |
| 5 / AZ064（不提升Gate） | 停止版本范围扩张；刷新源码manifest、vendor补丁重建、版本identity与candidate notes；根统一前端/ext-host/Rust/NSIS构建 | 日志exit、实际产物路径/大小/SHA、离线预检、包内/包外清单对应同一源码快照；失败先修再重跑 | 旧installer不能证明新源码已打包；未签名未安装候选保持candidate，不创建发布/推送/分发许可 |
| 6 / AZ067/069后续验收 | 只把可审阅包与准确限制交付给用户；同步release notes与未验收列表 | 本地相对稳定候选可审阅；正式安装/GUI/真实模型/恢复由资源窗口另验 | 83任务不会为提前候选发版全部强行done；正式稳定发布需原Gate和明确发行动作 |

AZ021当前正确性修复是主实现；AZ035纯前端/host有限文件、证据与独立review可并行。构建由主线程统一，不能为加快速度同时覆盖同一shared源或构建目标。若源码冻结后出现可复现P0/P1/P2正确性缺陷，修复并更新指纹/证据，再重建；不能用“后移”隐藏已知失败。

## 下一轮明确后移范围

后移是版本scope说明，不擅自将台账pending改成deferred或解除正式依赖。每项仍记录原ID及具体原因：

- 原生GUI/core E2E、长时稳定、干净机器安装与恢复：AZ009、013、032、056–058、062–064。AZ004于2026-10-09首次12项×3真实synthetic内核基线已交付，完整host/GUI旅程、账单与跨平台仍另验；未执行不通过。
- 全任务host/wire/成员资源硬限和价格来源：AZ024、071；辅助标题、memory/skill、delegate等未全覆盖，未知用量/价格不得当0释放。
- Remote全部工具取消/恢复、团队池/多Agent自动化：AZ045、070–076；局部generation或receipt候选不等于全旅程完成。
- 跨视图编辑交接/交付产物、非仓库工作目录、文档研究、连接器、桌面资源租约：AZ036–039、048–055；按核心依赖及授权范围逐包推进，不混入当前稳定化包。
- macOS/Linux凭据与核心旅程、公证/可信更新/Beta正式发行：AZ058、065、068–069、077–078；资源与真实验收未具备，Windowsunsigned候选不代替。

## 沿现脚本追踪与复盘

唯一状态源为docs/upgrade/plan.json，所有主线程更新走scripts/upgrade-plan.mjs。复用字段id/status/owner/dependencies/code_dependencies/next_action/evidence/implementation_review/completion_summary及history，不引入另一套完成百分比。每个原子包在原任务追加：触发问题、授权文件范围、实现SHA、实际命令/exit/通过与ignored数、失败日志、独立review、未覆盖入口、下一动作与需资源验收。进度展示通过脚本summary/render；正式Gate只能由原gate命令和相应验收证据决定。

代码依赖复核要有独立reviewer、明确scope/verification、evidence及source manifest；共享文件变更使pin过期时原子刷新全部相关review，不绕哈希校验。结束本轮时复盘三点：哪些原发现已有可靠证据解除，哪些只是候选实现，哪些必须留下一轮实际验收。候选包、任务done、产品成熟度、对外发行四者分别报告。

## 三个执行批次

批次1：关闭已复现取消缺陷并冻结本地可靠性。主线程已修晚Stop旗标决策，通用tool receipt仍在实施；必须以最新源实跑与独立复核确认，不能只据“已修改”验收。合并本轮AZ035焦点刷新、严格DTO/只读发现补验，限定本地单Agent范围。

批次2：一次最新Windows候选构建、manifest与升级风险闭环。停止新增功能，根统一构建并写实际产物SHA/源码身份；记录旧包与新包的功能差异、数据schema/备份恢复风险、unsigned及未安装限制。本批只准备本地可审阅候选，不publish、push、tag或对外分发。

批次3：补后移的真实GUI/model/安装回滚与跨平台验收。沿原ID安排实际资源与故障窗口，记录成功率/成本与原生交互，不用核心夹具、文档、Windows构建或延期决定替代其他平台与恢复验收。原Gate保持独立。
