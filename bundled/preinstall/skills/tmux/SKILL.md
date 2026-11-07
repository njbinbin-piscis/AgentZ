---
name: "tmux"
description: "Remote-control tmux sessions for interactive CLIs by sending keystrokes and reading pane output. Use when a task needs an interactive TTY such as a REPL or debugger."
description_zh: "通过发送按键和读取窗格输出远程控制 tmux 会话，用于 REPL、调试器等需要交互终端的命令行程序"
version: "1.0.0"
source: agentz
---

# tmux Skill

Use tmux only when you need an interactive TTY. Prefer running long, non-interactive tasks in the background with `shell` / `process_control`.

## Quickstart (isolated socket)

```bash
SOCKET_DIR="${TMPDIR:-/tmp}/agent-tmux-sockets"
mkdir -p "$SOCKET_DIR"
SOCKET="$SOCKET_DIR/agent.sock"
SESSION=agent-python

tmux -S "$SOCKET" new -d -s "$SESSION" -n shell
tmux -S "$SOCKET" send-keys -t "$SESSION":0.0 -- 'PYTHON_BASIC_REPL=1 python3 -q' Enter
tmux -S "$SOCKET" capture-pane -p -J -t "$SESSION":0.0 -S -200
```

## Socket convention

- Default socket dir: `${TMPDIR:-/tmp}/agent-tmux-sockets`
- Default socket path: `"$SOCKET_DIR/agent.sock"`

## Targeting panes

- Target format: `session:window.pane` (defaults to `:0.0`)
- Keep names short; avoid spaces.

## Finding sessions

- List sessions: `scripts/find-sessions.sh -S "$SOCKET"`
- Scan all sockets: `scripts/find-sessions.sh --all`

## Sending input safely

- Literal sends: `tmux -S "$SOCKET" send-keys -t target -l -- "$cmd"`
- Control keys: `tmux -S "$SOCKET" send-keys -t target C-c`

## Watching output

- Capture recent: `tmux -S "$SOCKET" capture-pane -p -J -t target -S -200`
- Wait for prompts: `scripts/wait-for-text.sh -t session:0.0 -p 'pattern'`

## Cleanup

- Kill session: `tmux -S "$SOCKET" kill-session -t "$SESSION"`
- Kill all: `tmux -S "$SOCKET" kill-server`

## Helper: wait-for-text.sh

```bash
scripts/wait-for-text.sh -t session:0.0 -p 'pattern' [-F] [-T 20] [-i 0.5] [-l 2000]
```