# AZ-029 秘密保存失败关闭

执行者：Codex。内核基线：piscis-engine `527e55496caf4437e4265e02869f0a8c764549f6`（v0.8.68）；开始前该仓库无修改，无适用 AGENTS.md。仅使用临时目录和虚构凭据，不读取用户真实配置。

本地实现：Settings::save 必须成功加载 SecretStore，每个非空秘密字段的加密错误向上传递；全部加密及序列化成功后才在同目录创建临时文件，写入、sync_all、原子替换配置。原配置不再提前截断。视觉模型 API key 补入加密/解密字段；SSH 密码/私钥和自定义 provider 继续逐项处理。SecretStore 新密钥先完整写入并同步临时文件，再以不覆盖方式发布；并发已存在则读取已有密钥，不覆盖另一个写入者。

测试范围：损坏密钥、密钥路径不可读、实际随机调用边界注入失败、第二字段/最后一个嵌套字段加密失败、Windows 禁止删除共享的真实文件锁、连续成功原子替换与 API/视觉/SSH/provider 凭据重读。比较旧文件逐字节内容，扫描虚构凭据未出现在磁盘配置，核对临时文件清理。

验证：首轮7项通过（首次编译4m17s），补齐嵌套凭据后的最终8项通过（编译40.88s，运行0.13s），日志 `.cache/upgrade-secrets-cycle-008-final.log`。随机故障为调用边界注入，不是故障化操作系统 RNG。Windows 文件锁是实际文件系统失败，不冒充其他平台的目录权限验收。

宿主候选：`cargo check -p agentz-desktop --config .cache/upgrade-kernel-local.toml` 通过（2m28s），三个内核包临时覆盖为本地修复源码；原 Cargo.lock 在 finally 中按字节恢复，SHA256核对一致。日志 `.cache/upgrade-secrets-host-cycle-008.log`。首次误用包名 agentz 导致命令失败，已纠正；该失败保留于 package-error 日志，未计为通过。这是候选源码兼容检查，不是固定远端依赖或原生产品测试。

可移交补丁：`docs/upgrade/patches/AZ-029-v0.8.68.patch`，SHA256 `effb419ebb27c1a5d852acc29269c4d7cf26fbbba511d1b5cfe29c953cfb9e1b`。在临时夹具提取固定基线三个原文件，apply --check、实际 apply、逐文件内容与修复源码比较、reverse --check 均通过；夹具 `C:\Users\zzz\AppData\Local\Temp\agentz-secret-patch-arw5svde`。未对用户目录做回退操作。

静态检查：`cargo clippy -p piscis-kernel --lib -- -D warnings` 与不带额外参数的clippy均退出101，因workspace本身warnings=deny。四个失败为 auto_select.rs manual_find、json_repair.rs collapsible_match、memory/vector.rs chunks_exact_to_as_chunks、store/db.rs redundant_closure；分别以 git hash-object --path 对照固定基线blob，全部一致。没有改这四个文件或放宽全局lint；未记录clippy通过。日志 `.cache/upgrade-secrets-clippy-cycle-008.log` 和 `.cache/upgrade-secrets-clippy-default-cycle-008.log`。所有本次构建均已退出。

验收状态：review；本地故障回归和候选宿主check通过，但固定远端依赖尚未接入，不计done。后续需要固定可分发依赖和回滚复核。视觉key新加密字段在旧版本不自动解密，不能宣称跨版本回滚验收完成；旧配置备份/迁移及释放前的兼容性验收必须覆盖此变化。

边界：读配置时的旧格式解密猜测和系统主密钥保护属于 AZ-030，秘密引用/出口属于 AZ-031/077/078；本项不声称已经实现。sync_all 后的原子替换不等于已验证断电恢复。AgentZ 仍固定远端 v0.8.68，此本地内核修改尚未进入固定依赖；后续宿主候选验证和可分发依赖必须分别记录，不把旧依赖构建当作修复验收。未发布。
