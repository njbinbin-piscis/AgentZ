# 工作流升级 Windows 本地候选构建

本次仅生成本地未签名0.9.7工程候选，纳入已冻结的AZ036导航/任务文件查看、AZ038产物版本、AZ040输入/预算展示和AZ039交付观察摘要。完整原生桌面、安装/卸载/回滚、签名、公证、跨平台验收未执行；不发布、部署或对外发送。

当前源码仍含已有未提交工作，冻结指纹为AZ-064-workflows-source-snapshot.json，不宣称clean Git HEAD复现。旧contract候选安装器保留于原独立目录。

Session47800已启动extension-host生产构建，然后在成功时运行前端TypeScript/Vite。尚未报告两者完成，尚未启动NSIS。仅在前端构建成功及源指纹核对后冻结dist/发行输入，再执行单次offline/locked NSIS构建，归档到新的独立目录并进行离线产物检查。
## 构建检查点 2026-10-10
extension-host 生产构建实际退出成功（host.js 与 smoke.js 已产出）。同一 Session 47800 的前端 TypeScript 已进入 Vite transforming，尚未完成；当前 Node 构建进程仍存在并消耗 CPU，因此不重复启动构建。台账 validate 实际通过：83 tasks / 11 milestones。

只读 AZ041 审查发现既有编辑器外部修改/删除和异步保存期间 dirty 状态可能丢失，详见 AZ-041-next-scope.md。这属于当前工程候选已知风险，尚未修复或验收；不将本次包认定为稳定发布。源码冻结仍保持，待构建结束后处理，旧候选归档保留。
## 前端完成与 NSIS 启动
Session 47800 实际退出0：extension-host 和 TypeScript/Vite 生产构建完成，Vite 4156 modules / 6m5s；大 chunk 和 dialog 静动态导入警告保留。Session11300 实际退出0：348份冻结源指纹未变，并冻结2094份dist/extension-host/资源/配置输入至 AZ-064-workflows-build-inputs.json。

随后仅启动一次 Session40602：npx tauri build --bundles nsis --config target/az064-workflows-tauri.json --ci --no-sign -- --offline --locked。覆盖配置跳过重复frontend构建，明确不签名。NSIS/Cargo尚在运行，没有新安装器完成证据或摘要。源码继续冻结，禁止并行Cargo或代码编辑。
## Rust release 完成
Session40602 返回 Finished release profile optimized in 56m48s，已生成 agentz-desktop.exe，随后实际进入 x64 makensis。该 session 尚未退出，旧 bundle/nsis 安装器时间戳仍属于上一构建，因此暂不将其复制或算作本轮产物。新归档与源/输入再次核对须等待 NSIS 成功结束。
## 本次安装器完成与冻结解除
Session40602实际退出0，NSIS完成一个bundle。Session16806实际退出0，重新核对348源码+2094发行输入全部未变，归档到 target/release/candidate-0.9.7-workflows-20261010/AgentZ_0.9.7_x64-setup.exe。实际98,766,742 bytes，SHA256 e4bd973f1f0da2bc6eabaeecdbc9e0690b2e8c20b35e6aba860e83e21d284223，Authenticode NotSigned。

Session59546离线candidate预检实际退出0/status verified，release_ready false。未安装/GUI/卸载/回滚/跨平台验收，无发布。AZ041具体风险仍存在。此前contract归档未覆盖。归档后源码冻结解除，主实现转AZ041未保存缓冲区保护；新修改不包含于本归档。
