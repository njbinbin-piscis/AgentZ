---
name: "things-mac"
description: "Manage Things 3 on macOS with the things CLI: add and update todos and projects, list inbox, today and upcoming, and search tasks. Use when the user asks to add or look up tasks in Things."
description_zh: "在 macOS 上通过 things 命令行管理 Things 3：添加更新待办、查看收件箱和今日任务、搜索任务"
version: "1.0.0"
source: agentz
---

# Things 3 CLI

Use `things` (run through `shell`) to read your local Things database (inbox/today/search/projects/areas/tags) and to add/update todos via the Things URL scheme.

Setup
- Install: `GOBIN=/opt/homebrew/bin go install github.com/ossianhempel/things3-cli/cmd/things@latest`
- If DB reads fail: grant **Full Disk Access** to the calling app.
- Optional: set `THINGSDB` or pass `--db` to point at your `ThingsData-*` folder.
- Optional: set `THINGS_AUTH_TOKEN` to avoid passing `--auth-token` for update ops.

Read-only (DB)
- `things inbox --limit 50`
- `things today`
- `things upcoming`
- `things search "query"`
- `things projects` / `things areas` / `things tags`

Write (URL scheme)
- Prefer safe preview: `things --dry-run add "Title"`
- Add: `things add "Title" --notes "..." --when today --deadline 2026-01-02`
- With tags: `things add "Call dentist" --tags "health,phone"`
- Checklist: `things add "Trip prep" --checklist-item "Passport" --checklist-item "Tickets"`

Modify (needs auth token)
- Get ID: `things search "milk" --limit 5`
- Title: `things update --id <UUID> --auth-token <TOKEN> "New title"`
- Complete: `things update --id <UUID> --auth-token <TOKEN> --completed`

Notes
- macOS-only.
- `--dry-run` prints the URL and does not open Things.