# AZ-064 签名、公证与安装证据契约候选

日期：2026-10-07。仅交付离线证据契约与解析器。没有运行签名/公证工具、安装器、下载、发布、证书读取或秘密读取，没有更新台账。

## 交付

- `docs/upgrade/contracts/signing-evidence.schema.json`：schema_version 1、平台windows/macos/linux与arch x64/arm64、artifact预期/观察SHA-256、签名预期/实际identity、tool raw result与SHA-256及绑定artifact/identity、执行时间、证书有效窗口和时间戳状态。
- `scripts/upgrade-signing-evidence.mjs`：Node无依赖离线解析器，导出validateSigningEvidence/rawHash；CLI仅读取指定JSON和bundled schema、向stdout输出诊断。不执行任何外部程序、不读取artifact/录屏内容、不访问网络、不改文件。
- `scripts/upgrade-signing-evidence.verify.mjs`：纯内存synthetic契约夹具，无真实证书、工具或安装。

签名与安装各自独立status：签名错误不会把已有安装记录抹掉，安装成功声明也不会使签名通过。macOS契约独立要求notarization和staple两份执行记录。这是本项目候选发行证据政策，不是关于某平台全部安装格式的合规声明；windows/macos要求有效时间戳，Linux记录仍需明确签名身份和信任材料。

installation记录clean_machine、非秘密machine_id、platform/arch及install/launch/uninstall三步。每步执行布尔值、结果、UTC时间、artifact SHA-256、raw result/hash、录像SHA-256独立记录；没有录像或未执行不得作为完整证据。录像哈希仅来自输入，解析器不打开录像，后续人工/native验收需检查录像和实际文件。

## 状态与真实性边界

| 输出 | 含义 |
| --- | --- |
| schema_valid | 输入形状符合严格schema；不是产品通过 |
| missing | 缺工具、证书/信任材料或预期签名身份，禁止ready |
| unverified | 未执行或未确认摘要/有效性/安装步骤 |
| invalid | identity、artifact或raw摘要不一致；tool非零、签名/时间窗口无效；平台或步骤时序错误 |
| candidate | bundle或任何tool/install receipt来源是fixture/candidate，不允许提升ready |
| evidence-ready-for-review | actual执行**声明**的证据字段完整且一致，只是可交给独立native审查 |
| acceptance_passed=false | 所有输入恒定为false，离线解析器无法授予真实签名/安装验收 |

Bundle与每份tool/install receipt均有provenance。即使bundle标actual，内部fixture/mock receipt也只能candidate。executed严格boolean，字符串`"true"`拒绝；未经执行的mock只candidate/unverified，不得用模拟payload登记实际验收。字段extra拒绝，不能夹带`acceptance_passed:true`。

expected_identity应来自已确认发行身份，优先使用固定证书/公钥指纹或明确team/publisher identifier，不能只凭易重名的显示名称。actual_identity与tool receipt.identity必须严格一致；null预期身份明确missing，不猜身份/证书。tool_result中的raw_result按UTF-8字符串精确SHA-256匹配，receipt artifact hash必须匹配manifest预期digest；观察digest必须也匹配。JSON输入本身仍可被伪造，解析器无法证明provenance/executed真实性，也不解释任意原生工具raw output的语义，必须由独立capture/原生工具验收验证。完整字段不能替代此真实性检查。

所有时间使用canonical UTC ISO8601含毫秒，例如2026-10-07T01:00:00.000Z；拒绝非法日历日期、倒置证书窗口、签名/参考时间超窗口、timestamp晚于verification、capture晚于record和install→launch→uninstall倒序。证书有效性字段表示签名时有效，允许由已验证时间戳表达其签名时证据；未验证可选timestamp不能用于绕过有效窗口。

## 使用与退出码

`node scripts/upgrade-signing-evidence.mjs <evidence.json>`：0仅表示完整actual声明可审查（acceptance仍false）；2 schema错误；3字段矛盾/无效；4 actual缺失/未验证；5 candidate/fixture。不要把exit0当作安装器签名、安装或公证真正通过。

实际签名/公证工具应由后续独立受控capture产生证据，原始日志及录像需关联该artifact、平台和架构。当前没有此capture工具或受信执行证明，契约不能解锁M8发行gate。

## 本轮实跑

运行 `node scripts/upgrade-signing-evidence.verify.mjs`，26项纯内存契约夹具全部通过，exit0；native signing/installations executed=0。

覆盖预期identity与捕获identity不符、artifact digest与tool绑定不符、raw结果摘要篡改、缺工具/证书/预期identity、未执行mock、非零exit、无效timestamp/证书窗口、Windows时间戳禁止跳过、签名状态与安装状态分离、安装平台/日志/录像/时序错误、macOS缺公证、非法日期、truthy执行标志、未知字段/arch。每个夹具断言acceptance_passed=false、review_ready=false、输入未被修改。不能把26项夹具通过描述为26项真实平台签名/安装验收通过。

剩余实际条件：发行预期身份、证书或签名服务、各平台原生验证机器、干净安装/启动/卸载录像、真实公证/staple输出。均未确认/未运行；无需在本轮向用户索取秘密。
