# AgentZ 1.0.7 发布说明

发布日期：2026-10-21。版本号与 `CHANGELOG.md` 的 1.0.7 条目一致。

AgentZ 1.0.7 修复了 Windows 沙箱（`shell` 工具）在云盘映射/同步目录（如坚果云 Nutstore、OneDrive 等）中报 `NTSTATUS 0xC0000022`（拒绝访问）以及无法调用未授予 `ALL APPLICATION PACKAGES` 权限的开发工具链（如 `node`、`python`、`cargo`）的问题；同时全面修复了全局字体缩放（`Aa`）下各类弹出框、右键菜单、顶栏浮层与模态框越界裁剪的问题。

## 修复

- **Windows 沙箱兼容云盘同步目录与开发工具链**：
  - 在 `NtCreateFile` 逐级相对打开句柄时补齐 `FILE_OPEN_FOR_BACKUP_INTENT`（`0x00004000`），并在后台云盘同步守护进程持有工作区根目录 `DELETE` 共享句柄导致 `0xC0000022` / `0xC0000043` 时自动回退至 `FILE_SHARE_DELETE`。
  - 通过 `GetFileInformationByHandleEx(..., FileAttributeTagInfo, ...)` 区分良性云端同步重解析标签（`IO_REPARSE_TAG_CLOUD*`、`IO_REPARSE_TAG_PROJFS`、`IO_REPARSE_TAG_DEDUP`、`IO_REPARSE_TAG_WOF`）与恶意重定向重解析点（符号链接、目录联接 Junction），并在云盘微过滤器拒绝 `FILE_OPEN_REPARSE_POINT` 时安全探测后重试。
  - 新增只读 DACL 工作区句柄链锁定校验（`verify_and_pin_readonly_workspace`）与开发工具链 `S-1-15-2-1` 权限检测（`command_requires_unprivileged_toolchain_fallback`）：当网盘共享目录拒绝 `WRITE_DAC`（`0xC0000022`）或命令调用了未包含 `ALL APPLICATION PACKAGES` ACE 的开发工具（如官方 MSI 安装的 `node.exe`）时，在完整验证目录树无符号链接、无多硬链接、无空 DACL 并锁定句柄链的前提下，使用受控 Windows Job Object 执行命令。
- **全局 UI 缩放（`zoom`）下全量弹出层、右键菜单与模态框防越界裁剪**：
  - 新增 `src/utils/popupPosition.ts`（`computeFixedPopupPosition` 与 `computeContextMenuPosition`），统一对 `getBoundingClientRect()` 和鼠标坐标执行 `zoom` 归一化，并在靠近窗口右侧或底部边缘时自动改用 `right` / `bottom` 锚定与动态 `maxHeight` 收敛。
  - 覆盖 `DropdownSelect`、`DropdownMultiSelect`、`TagMultiSelect`、`NotificationInbox`、`ChatComposer`（`@` / `/` 补全弹层）、顶栏 `Aa` / 工作区 / 快速上手浮层、全量右键菜单（文件树、编辑器标签页、终端、扩展面板）及全量模态对话框。
