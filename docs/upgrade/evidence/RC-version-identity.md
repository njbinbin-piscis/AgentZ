# Windows x64候选版本身份：0.9.7

2026-10-07，主线程授权本地准备0.9.7版本身份。修改前 `git show-ref --verify --quiet refs/tags/v0.9.7` 退出码1，确认本地无该tag；修改后再查仍退出码1。未查询远端版本是否存在，未创建tag、commit、push、Release或CI。

仅更新 AgentZ 自身的六个值：package.json.version、package-lock.json顶层version与packages空键根version、src-tauri/Cargo.toml package version、src-tauri/tauri.conf.json.version、Cargo.lock中name=agentz-desktop条目的version；全部从0.9.6改为0.9.7。未运行npm install、Cargo更新、编译、dist重建或打包，保留既有dirty变更。

实跑六处读取检查最终全部0.9.7、退出码0。初次PowerShell JSON检查因package-lock含空键须使用ConvertFrom-Json -AsHashtable而失败；修正读取方式后通过，未因此额外改文件。

逐文件在修改前/后仅mask本任务的身份version值后计算SHA-256，五个文件masked摘要全部相等，证明除明确六处值外原文本（包括第三方依赖、锁内容与既有修改）保持不变。Cargo.lock的const-oid 0.9.6与0.10.2条目仍原样，不做全局版本替换。

在途旧版本构建和dist不因元数据修改自动成为0.9.7候选；主线程需统一生成与核对实际新版本产物、源码快照和摘要。此次版本身份检查不代表安装、签名或发布验收。
