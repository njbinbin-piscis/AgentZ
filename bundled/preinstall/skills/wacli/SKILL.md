---
name: "wacli"
description: "Send WhatsApp messages or search and sync WhatsApp history with the wacli CLI. Use only when the user explicitly asks to message someone or search WhatsApp history."
description_zh: "通过 wacli 命令行发送 WhatsApp 消息或搜索、同步聊天记录，仅在用户明确要求时使用"
version: "1.0.0"
source: agentz
---

# wacli

Use `wacli` only when the user explicitly asks you to message someone on WhatsApp or to sync/search WhatsApp history.

Safety
- Require explicit recipient + message text.
- Confirm recipient + message before sending.

Auth + sync
- `wacli auth` (QR login + initial sync)
- `wacli sync --follow` (continuous sync)
- `wacli doctor`

Find chats + messages
- `wacli chats list --limit 20 --query "name or number"`
- `wacli messages search "query" --limit 20 --chat <jid>`

Send
- Text: `wacli send text --to "+14155551212" --message "Hello!"`
- File: `wacli send file --to "+14155551212" --file /path/agenda.pdf --caption "Agenda"`

Notes
- Store dir: `~/.wacli` (override with `--store`).
- Use `--json` for machine-readable output.