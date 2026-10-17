# AZ-083 后代回收探针

2026-10-06，当前 Windows 主机。

AZ-030 完整内核回归记录358 passed、1 failed、1 ignored，失败在 `proc::tests::windows_appcontainer_child_is_confined_to_explicit_directory` 的后代存活 marker；同一测试二进制单独重跑通过。原始失败不能抹除。

原探针使用 `timeout /t 4 /nobreak >nul`，其等待依赖控制台输入。用隐藏 cmd、重定向 stdin 的实际探针约1138ms退出，退出码1，并报告输入重定向不支持；不能保证4秒后才写marker。这会让marker在Job关闭前出现，导致错误地推断子进程逃逸。

修改仅涉及测试夹具：批处理先写ready，等待release文件存在后才写survival marker。正向对照先放置release，证明同一AppContainer权限下能写marker；随后删除三份标记，再由root派生child。父测试等到ready并确认Job仍有活跃后代后才关闭Job，关闭后发布release，检查marker未出现。等待有5秒上限，失败不放宽断言。

生产 `proc_windows_sandbox.rs` 本轮未修改：其在root退出、超时、取消时显式 `TerminateJobObject`，再等待root及读取输出；测试探针独立检查 kill-on-close 行为。本记录不新增真实Agent会话、GUI、跨平台或发行验收。

首版握手夹具专项失败在正向对照（exit1），尚未进入回收断言。调整批处理入口为显式 `cmd /c call` 后，专项1 passed，再直接运行同一测试二进制连续三次，均1 passed（约10.5秒/次）。

完整产品内核复跑 `cargo test -p piscis-kernel --locked -- --test-threads=1 --quiet`：359 passed、0 failed、1 ignored；pool_services 25 passed。新探针没有控制台延时依赖，并保留实际子进程就绪、Job活跃进程及关闭后存活标记断言。首次失败记录仍保留。

`proc.rs` rustfmt --check通过，台账validate通过。该文件SHA-256为 `dd8df6b7a5353daf3e4d0ac6b4e1737577e9d83f542594b6344e5bcfc1ea4819`。本任务只关闭当前Windows测试夹具问题，不代表压力竞态、多用户或真实桌面验收。
