# R10-02 审计证据：Git 工作树源码纳管与运行期脏文件隔离

- **日期**：2026-10-19
- **关联任务**：`R10-02`
- **核心变更**：
  1. 更新 `.gitignore`，补充 `.agentz/**`、`*.db`、`*.db-wal`、`*.db-shm`、`playwright-report/`、`test-results/` 忽略规则，防止运行期 SQLite 数据库与 E2E 临时产物污染版本库。
  2. 执行 `git rm --cached -r --ignore-unmatch .agentz` 并校验 `git status --short .agentz` 输出为空。
  3. 校验预装资源完整性 `node scripts/verify-preinstall.mjs`。

## 自动化验证记录

- `git check-ignore -v .agentz/index.db`：命中 `.gitignore` 规则
- `git status --short .agentz`：输出为空（零运行期数据库残留）
- `node scripts/verify-preinstall.mjs`：退出码 `0`（`verify-preinstall ok: skills=254 agents=233 commands=336 templates=9`）
