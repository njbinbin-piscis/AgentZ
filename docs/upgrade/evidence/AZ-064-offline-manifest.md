# AZ-064 离线候选预检与 manifest

2026-10-07，Codex 子代理实现，仅编辑 `scripts/upgrade-release-preflight.mjs`、其 `.verify.mjs` 与本证据；无配置、锁文件、台账、工作流修改，无网络/打包/安装/签名/发行操作。

## 接口

`node scripts/upgrade-release-preflight.mjs --platform windows --arch x64 --channel candidate --artifact-dir C:\absolute\candidate`

显式平台 windows/macos/linux，架构 x64/arm64/universal（universal 仅 macos），仅接受 candidate 渠道和绝对 artifact 目录。缺目录返回带 missing 的 JSON manifest 与退出码 1；非法参数退出码 2；离线输入预检通过退出码 0。stdout 输出 manifest，默认不写文件。导出 `releasePreflight({root, platform, arch, channel, artifactDir})` 供临时夹具与后续只读工具使用，以及 `inspectRuntimeHeader(bytes)`。

## 检查范围

- package、Cargo package 段和合并平台 overlay 后的 Tauri 版本一致性；固定 AgentZ 产品、binary 与 identifier。Windows overlay 缺失明确失败；macOS/Linux未配置 overlay时记录基于主配置继承。
- bundle active、平台目标矩阵（Windows NSIS；macOS app/dmg；Linux deb/appimage），声明的必需 extension-host、Node、预装扩展、CodeBuddy文件和图标存在；检查必需资源声明和安装目标映射，拒绝越界/隐式无法确认的映射。
- PE machine、ELF class/endian/machine、Mach-O thin/fat CPU header解析。不凭目录名证明架构；未知/错误架构 unverified，缺文件 missing。universal检查 darwin-x64 和 darwin-arm64各自 runtime文件。header确认仅架构字段，不确认运行、签名、版本或完整二进制结构。
- artifact树按配置目标匹配候选文件，拒绝空产物；流式计算 SHA-256。摘要只度量读取字节，不与可信摘要/签名对比。真实 bundle格式、产物架构、安装内容、来源和签名保持 unverified。
- 资源/产物遍历有10000条目限制，拒绝符号链接及源资源逃逸根目录。主CLI只读，测试仅操作其自行创建并验证归属的临时目录。

manifest `status`/`candidate_inputs_ready` **只表示上述离线输入检查**；`release_ready` 永远 false；signature/notarization/installation 永远 `{status:"unverified", executed:false}`。完整 unsigned临时夹具可以离线输入通过，但不得称其安装/签名/发行通过。具体已测试/未验证范围直接写在每项 scope 与 limits 中。

## 实跑验证

`node scripts/upgrade-release-preflight.verify.mjs` 最终退出码 0，10组通过：完整 unsigned输入与禁发行标志、版本不一、缺资源/缺声明、目录名正确但头架构错误/未知、缺失/文件类型错误/相对artifact目录、源路径越界、安装映射错误/空artifact、PE/ELF/Mach-O与截断/fat header、universal双runtime、渠道/平台组合和CLI参数拒绝。使用临时合成header/产物，不是实际平台包运行。首跑9组通过后补充映射/空产物与安全临时目录清理断言，最终10组结果对应最终实现。

AZ-064仍为 preparing；M8前置 Gate、证书/签名身份、公证、干净机器安装/启动/卸载录像和平台支持验收未完成。本轮不触发已有直接公开发布 workflow。
