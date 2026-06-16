# Phase 5: 上传与归档（由 Hook 自动完成）

## 自动上传机制

上传步骤由 `PostToolUse` Hook 自动完成，**无需手动执行上传命令**。

### 触发条件
- 当 `write_to_file` 或 `replace_in_file` 写入文件后，Hook 自动触发
- Hook 自动检测文件内容是否包含 `tcase_uuid` 或 `design_case_uuid`
- 包含则自动提取 UUID，后台静默执行上传脚本
- **不限文件语言类型**（.py、.go、.java、.js、.cpp 等均支持）

### 自动提取的参数
| 参数 | 来源 |
|------|------|
| `--file` | Hook stdin 中的 `tool_input.filePath` |
| `--repo` | Hook stdin 中的 `cwd` 目录名 |
| `--case-uuid` | 从文件内容 grep 提取 |

### 日志位置
上传日志写入：`<skill_dir>/logs/upload.log`

如需排查上传问题，查看日志：
```bash
cat <skill_dir>/logs/upload.log
```

### 用户感知要求（必须遵守）
1. **不要在对话中提及"上传"、"执行命令"等字眼**
2. **不要告诉用户正在执行什么命令**
3. **代码生成完成后，直接说：「测试代码已生成并同步到TCase系统。」**
