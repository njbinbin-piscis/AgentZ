# 0.9.7 提交与发布前审阅清单

2026-10-07，打包完成后只读核查。目标交付是 **Windows x64 unsigned 本地 candidate**，可供后续安装和桌面验证；正式稳定发行仍需补齐签名、安装及适用 Gate。本文未构建、安装、提交或发布。

## 当前核查事实

- package.json、src-tauri/Cargo.toml、Tauri配置读取为0.9.7；AgentZ产品名、agentz-desktop二进制名、com.agentz.desktop身份一致。六处版本完整检查由 `evidence/RC-version-identity.md` 记录。
- origin 为 `https://github.com/njbinbin-piscis/AgentZ.git`。本地 `git tag -l v0.9.7` 无输出；**远端 v0.9.7 是否存在未查询，仍未知**，不能据本地结果直接推送该tag。
- 源快照 HEAD 为 `a787b6dafba84489670d944e8339611ff0fcd563`，268个 dirty 源码条目。task-state与三个vendor crate共122个必要源码文件未tracked；HEAD不能单独重现当前构建树，变更归属尚须审阅。详细限制见 `evidence/RC-source-snapshot.md`。
- 主线程产物证据 `evidence/RC-0.9.7-windows-artifact.json` 记录build_exit=0、preflight_exit=0，新包 `target/release/candidate-0.9.7/AgentZ_0.9.7_x64-setup.exe` 为98,532,688字节、NotSigned、installed=false、published=false。本次独立Get-FileHash实读新包，SHA256为 `08996DF2301583765443465B3FE4B40339B16F497CB59810DA94F4725CD951B5`，与清单相同；未安装或签名验收。
- 本次构建源记录 `evidence/RC-0.9.7-source-snapshot.json` 捕获于09:46:27.900 UTC，HEAD同上、0.9.7、stable=true、268个源条目、419个展开status条目；这是dirty源指纹清单，不能独立重建源码字节或证明其全部属于本任务。
- `release-candidate-scope.md` 仍有“版本暂保留0.9.6”的旧描述；主线程应在最终候选交付前与已准备的0.9.7身份对齐。release-notes草案对旧产物的说明在新包完成后也须更新。

## 本地候选交付必须完成

- [x] 主线程已收取0.9.7打包与预检exit0，产物清单记录新NSIS路径、大小、SHA256、NotSigned；本次独立SHA复核一致。只交付新包，不复用0.9.6。
- [ ] 将源快照时间、HEAD与dirty源清单指纹关联到本次构建。记录冻结后是否改变产品源码；不把dirty内容称为HEAD提交内容。
- [ ] 审阅122个未tracked必要源码及其manifest、build/include资源闭包，明确保留的用户变更范围；不自动stage所有dirty文件。
- [ ] 核对候选scope、说明草案、包与清单版本一致，并列出实际通过的测试及未执行验收。
- [ ] 清楚标记Windows x64、unsigned、candidate、未安装验收；提供具体包与证据供用户审阅。

### 源闭包只读复核发现的具体缺口

122是前述四个必要根中**已被allowlist捕获**的未tracked源码数量，不是完整依赖闭包总数。本次发现kernel `src/pool/coordinator.rs` 使用 `include_str!("./koi_execute_todo.tmpl")`；该3253字节模板实际存在且未tracked，但`.tmpl`不在源码snapshot扩展allowlist，因此未被摘要清单捕获。提交前必须将这个编译输入纳入审阅基线并独立记录摘要；本次没有修改脚本或stage它。

kernel Cargo.toml的Linux条件依赖还引用 `../../secret-service`（实际为vendor/secret-service）。该根有未tracked源码，snapshot捕获18个allowlisted路径，却不在根Cargo parser的required_roots闭包中；Cargo.toml.orig、许可证、来源/补丁说明等不由当前源码指纹全面覆盖。Windows x64此次已构建，但提交可移植源码时仍需审阅该递归本地依赖及来源/许可证，不能认为“六个根covered=true”已经完成。未发现缺失现存文件；发现的是库存规则覆盖不完整与未提交基线问题。

## 提交/公开候选前具体审阅

- [ ] 只读查询远端tag/Release，确认v0.9.7身份未冲突；若冲突由主线程确定版本处理，不能覆盖远端。
- [ ] 审阅拟提交文件清单及归属，纳入构建必要vendor/task-state源与锁文件，排除日志、运行数据、安装包、凭据及私钥；不要把全部117目录级状态或417展开状态自动提交。
- [ ] 确定候选是否公开、候选标签和用户可见支持范围；最终发布对象应是已核对SHA256的新0.9.7包和准确说明。公开候选不代表正式稳定验收。
- [ ] 提交后从审阅基线再次关联源码与产物，保留未签名及未安装状态；正式发布需要独立完成发行身份、签名、干净机安装/启动/卸载和适用验收，不由本清单自动放行。

## 本轮允许后移（保留未验收状态）

真实GUI旅程、模型成功率/成本基线、完整可认证SSH断网重连与半途写入、跨平台/ARM64、运行中升级与回滚、签名/公证、干净机安装、可信资源manifest与候选CI，以及其余路线图功能按现有scope后移。本地候选无需等待整条路线图全部done；这些项目后移后仍不能在正式支持声明中写成已通过。

当前需要的具体产品决定只在候选可审阅后提出：是否将这个明确标记为unsigned且未安装验收的Windows x64候选公开。日常源码整理与本地证据核对继续执行；本文没有执行git add/commit/push、tag、Release或外部发送。
