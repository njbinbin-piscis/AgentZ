# AZ-018 只读历史分页服务候选

仅新增runtime/event_replay.rs与本证据；未mod/Cargo/台账变更，未构建。服务replay(project,&freshPlanBinding,run_id,attempt_id,cursor,limit)返回PageDto{stream,events,next_cursor,has_more}，caller保留环境租约并供应fresh host身份，renderer不能供应task/env作为authority。

校验canonical项目、绑定形状与登记环境/direct指纹；只读RunStore.load取得真实task/env并匹配fresh；attempts确认历史Attempt归属（不强制当前attempt）。stream仅由权威Run+已验证attempt构造；cross stream/负cursor/未来cursor拒绝。limit严格1..200，读取limit+1并截取返回limit判has_more，next_cursor为最后返回事件；无事件保留输入cursor，初始无历史None，不伪造事件。

调用主线程新open_existing_readonly，不建库/初始化schema/repair；legacy既有v1无eventnamespace合法空history，但不代表Run从未执行。缺一半namespace、gap、未知schema、破损payload显式失败。事件kind/id非空与created_ms非负进一步检查；存储层正在补身份形状、单payload1MiB与page4MiB预算，超预算必须错误而非静默截断。

新增4个真实SQLite夹具未运行：登记环境create→running→failed→retry后旧attempt分页/新attempt/重开/DB文件bytes不变；跨task/env/attempt/cursor、limit/future拒绝；legacy空namespace读取不写与半namespace拒绝；损坏payload拒绝。无Run写仅测试准备阶段真实create/transition/retry，服务没有Run状态修改。

主线程只读接口落盘后新增第5夹具（仍未运行）：missing Run库读取错误且不创建.agentz/runs.db或父目录；readonly instance直接transition拒绝且DB bytes不变；future Run schema拒绝且bytes不变。主线程IPC已注册，仍未本代理实跑IPC/UI。

独立复核修正：权威load匹配task/env后调用readonly has_active_task校验同task所有Run/Attempt一致性；忽略其bool，健康live Run仍可历史读取。新增第6真实SQLite夹具：当前a2 queued时旧a1历史可读；破坏validenum delivery为accepted或删除current a2但保留历史a1时，replay明确拒绝并证明DB bytes不变。89628旧源码编译不可覆盖本次修改，主线程需最新targeted重跑，未在本代理Cargo。

实际rustfmt与限定diff-check exit0；需主线程导出与最新只读RunStore接口同步后统一编译/测试。未IPC、UI回放、GUI/model恢复验收，不会根据事件自动恢复工具副作用。

Final root69230 validation on latest source: events target9 tests passed0 failed, including hiddenzero/REAL sequence corruption; registered host runtime::event_replay6 tests passed0 failed, including livehealthy historical read and corruptcurrent Run/Attempt refusal without repair/DBbyte changes. No product source changed after manifest refresh. Front119 full tests, tsc0 and corrected eventservice ESLint0/target6 are recorded above. Tests do not establish native IPC/GUI/model reconnect, complete streaming transcript durability, Runidentity discovery or external-deletion detection. Original acceptance/status/Gates remain unchanged.
