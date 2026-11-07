---
name: "imsg"
description: "iMessage and SMS CLI for macOS to list chats, read history, watch for messages, and send. Use when reading or sending Messages.app conversations from the shell."
description_zh: "macOS 上的 iMessage/SMS 命令行：查看会话、历史、监听和发送消息。"
version: "1.0.0"
source: agentz
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