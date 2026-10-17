# AZ-045 IDE 文件入口绑定候选

2026-10-07，Codex 子代理仅编辑 commands/ide.rs 与本证据；不改台账、不运行 Cargo、GUI 或真实 SSH。

ide_list_files、ide_search_files、ide_read_file、ide_write_file 和 ide_file_action 使用 is_remote 检测保留远程 scheme，再通过 bind 取得 path 与 generation，以 session.call 发送请求。畸形 remote scheme 由 bind 明确拒绝，不进入本地磁盘分支。读取保留 base64 回复校验、UTF-8/二进制结果和字节数；写入保留 base64 协议与本地创建父目录行为。

远程 rename 保留普通远程 POSIX 目标路径行为；URI 目标必须解析成功且 authority 与原 session 相同，否则拒绝，避免切换目标主机。新增一项 remote_file_binding_tests 覆盖不同 authority、畸形 URI、同 authority 大小写兼容与普通路径；尚未运行。

ide_git_discard 的远程分支从入口通过 with_session 固定世代，覆盖 tracked 探测、Git checkout 与未跟踪删除。未跟踪删除在原 scope 中 resolve/call，不重新 bind 新连接。因此探测断连时后续删除请求仍由原世代失败关闭。

追加修复：tracked 探测改为成功的 `git --literal-pathspecs ls-files -z -- path` 输出，任何命令失败直接返回错误。只有成功且空的列表才允许判定 untracked；非空列表必须以 NUL 结束，不能含空记录或与指定路径无关的记录。目录存在 tracked 子文件时仍走 tracked 恢复，避免删除整目录。字节记录比较保留 Unicode、新行和特殊字面文件名；新增 discard_tracking_tests 两项，尚未运行。

run_git_cmd_bytes 在已有 task-local scope 时读取 bound_session 并沿用原 generation，resolve 校验 authority；无 scope 时才 bind 当前远程目录并使用 session.call。远程 stdout 缺失不再默认为空，防止坏协议回复被当作 untracked。没有改变 Git 服务协议或跨命令的其它流程。

追加 Git diff：ide_git_diff 远程入口使用 with_session 包裹完整 base 树读取、原文、当前文件、统一 diff 步骤，run_git_cmd_bytes 沿用 scope generation；当前文件 resolve/call 也在原 scope，禁止中途 rebind。远程 Git 原文和 diff 命令失败直接返回错误；使用成功的 ls-tree -z 列表区分 added 文件缺少基点条目与读取失败。当前文件回复必须含合法 base64，缺失/损坏不再吞为空文本，新增 remote_diff_reply_tests 两项（尚未运行）。本地原文/当前文件/diff 逻辑保持原行为。

接口兼容性：DiffResult 形状和 IPC 参数不变，但远程断连、权限、坏协议等原先成功空文本现在返回 Err。远程已删除文件在 fs.readFile 报 ENOENT 时也显式返回 Err；目前服务只有字符串错误，不能可靠区别不存在与其它失败，本候选优先不伪造空内容，删除差异专用展示需后续明确状态协议。UTF-8 lossy 转换保持既有行为，未扩展二进制评审格式。

追加终端/监听入口：ide_terminal_create 从 bind 返回的 RemoteSession.target 不可变快照构造交互 shell argv，cwd 同样来自该 session.path；不在后续 await 读取当前 target。主线程负责 bind 捕获 target 后校验原 generation，完整 API 尚需统一编译验证。独立交互 shell 属新的传输，不代表宿主原连接存活，也不宣称桌面终端断连恢复已验收。

ide_start_watcher 检测远程 scheme 后 bind，向该 session.manager 的 watch_remote 传递完整 URI token 与解析路径。按主线程提供的 broker 契约，watch/unwatch 从 token URI 绑定 authority 和请求 generation；监听登记属于允许在同 authority 新世代恢复的控制操作，不采用文件写入副作用的禁止重放策略。stop 先拒绝畸形远程 URI，再调用 broker 清理登记；主线程负责断连清理和不同主机拒绝。

未覆盖：除 discard/diff 外其他 Git 复合命令的整体 session 范围、LSP 启停与连接复用。主线程/其它子代理继续处理远程 broker、公共 commands/remote 和 watcher 等范围。本文件 rustfmt 已运行；仍需统一编译、自动用例与真实 SSH 验收。候选不等于已完成。
