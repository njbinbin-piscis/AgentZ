---
name: "imsg"
description: "iMessage/SMS CLI for listing chats, history, watch, and sending."
description_zh: "精选热门 AI Agent 技能合集，汇集社区高下载量技能于一处。"
version: "1.0.0"
source: codebuddy
source_plugin: "hot-skills"
---

# imsg

Use `imsg` to read and send Messages.app iMessage/SMS on macOS.

Requirements
- Messages.app signed in
- Full Disk Access for your terminal
- Automation permission to control Messages.app (for sending)

Common commands
- List chats: `imsg chats --limit 10 --json`
- History: `imsg history --chat-id 1 --limit 20 --attachments --json`
- Watch: `imsg watch --chat-id 1 --attachments`
- Send: `imsg send --to "+14155551212" --text "hi" --file /path/pic.jpg`

Notes
- `--service imessage|sms|auto` controls delivery.
- Confirm recipient + message before sending.