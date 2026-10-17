# AZ-081 固定内核递归遍历排除目录

日期：2026-10-06（Asia/Shanghai）  
状态：Windows 当前宿主与固定依赖候选自动化验收通过；不代表 GUI、发行版或跨平台验收。

## 变更

- 在 `piscis-kernel` 导出 `EXCLUDED_SEARCH_DIRECTORY_NAMES` 与 `is_excluded_search_directory`，统一 `.git`、`node_modules`、`target`、`.cache`、`__pycache__`、`.vs` 六种目录名的排除规则，并按 ASCII 大小写不敏感比较。
- `FileSearchTool::walk_dir` 在访问者看到目录或条目之前剪枝排除目录，因此 glob/grep 不会读取其中的文件。
- 宿主 `check_search_links` 复用内核公开规则，避免预检过滤列表漂移；一般文件和非排除目录继续递归。
- 添加内核遍历单测，以及宿主对比测试：临时树中六个排除目录放置匹配文本，宿主链接预检与内核 grep 仅返回正常 `src` 文件。
- 追加固定候选补丁 `docs/upgrade/patches/AZ-081-v0.8.68.patch`，由 `scripts/upgrade-kernel-candidate.mjs` 从 immutable revision 重建；未修改相邻的 `C:\Projects\piscis-engine` 检出。

## 验证

- vendor 内核定向测试 `walk_dir_skips_excluded_directories_before_visiting_entries`：1 passed。
- 宿主定向测试 `search_preflight_and_kernel_share_excluded_directories`：1 passed。
- 最新完整 `cargo test -p piscis-kernel --locked -- --test-threads=1 --quiet`：354 passed、0 failed、1 ignored；pool integration 25 passed；doc-test 1 ignored。
- `cargo test --manifest-path src-tauri/Cargo.toml --test approval_execution --locked`：18 passed、0 failed、1 ignored。
- 固定 v0.8.68 候选上的相同内核单测：1 passed；候选从 revision `527e55496caf4437e4265e02869f0a8c764549f6` 重建并应用 AZ-029/AZ-079/AZ-081 补丁。候选目录首次离线解析生成自身 `Cargo.lock` 后完成测试；产品仓库锁文件未改用于本项验证。
- `cargo clippy -p piscis-kernel --lib --locked -- -D warnings` 通过。
- 宿主 Clippy 在严格警告模式下仅放行 AZ-080 已登记的三类历史 lint（`collapsible_if`、`cmp_owned`、`redundant_closure`），检查通过；rustfmt 与目标文件 `git diff --check` 通过。
- 台账 `node scripts/upgrade-plan.mjs validate` 通过。

## 固定候选指纹

- AZ-081 补丁 SHA-256：`5a16ffbeceaa6335b3d29bdbde2128f479e9c004e4cd3d033ec9a1402aa4cb6b`
- 完整候选归档 SHA-256：`a33465ac3d418620a195ded9b71573aa54d1baed92909f0f9b5408dc93dae92c`
- 候选 manifest 记录了 AZ-029、AZ-079、AZ-081 补丁校验值和固定基线。

## 边界

这证明当前 Windows 构建目标上的内核目录剪枝与宿主预检列表一致，不证明其它操作系统文件系统、原生 GUI、真实模型、release 包或部署行为。目录名排除是遍历策略，不是访问控制边界；授权根和链接安全由独立审批检查负责。
