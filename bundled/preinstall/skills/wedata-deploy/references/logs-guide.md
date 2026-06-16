# Logs Guide

## Overview

`logs.sh` serves two purposes:

1. **Local development**: Query logs from the locally running Agent service (`data/logs/app.log`)
2. **Remote platform**: The WeData platform calls this script remotely to fetch runtime logs and display them in the console

Both use cases share the same script and the same options.

---

## Log file location

| File | Description |
|------|-------------|
| `data/logs/app.log` | Current log file (default) |
| `data/logs/app.log.1`, `.2`, ... | Rotated backup logs (older entries) |
| `data/logs/evaluate.log` | Agent quality evaluation logs |

---

## Common scenarios

### Just started the app — check if it's running

```bash
./scripts/logs.sh -n 20
```

Shows the last 20 log lines. Look for startup messages or errors.

### Watch logs in real time (like `tail -f`)

```bash
./scripts/logs.sh -f
```

Streams new log lines as they appear. Press `Ctrl+C` to stop.

Combine with filters:

```bash
# Real-time, errors only
./scripts/logs.sh -f -l ERROR

# Real-time, filter by keyword
./scripts/logs.sh -f -g "tool_call"
```

### Find errors

```bash
# Last 50 ERROR lines (default count)
./scripts/logs.sh -l ERROR

# Last 20 ERROR lines
./scripts/logs.sh -l ERROR -n 20

# Errors containing a specific keyword
./scripts/logs.sh -l ERROR -g "timeout"
```

### Search by keyword

```bash
# Find all lines mentioning a tool name
./scripts/logs.sh -g "translate"

# Find lines with a specific request ID or trace
./scripts/logs.sh -g "req-abc123"

# Regex is supported
./scripts/logs.sh -g "Exception|Error"
```

### Filter by time range

```bash
# Logs after a specific time
./scripts/logs.sh -s "2026-03-27 10:00:00"

# Logs within a time window
./scripts/logs.sh -s "2026-03-27 10:00:00" -u "2026-03-27 11:00:00"

# Date only (expands to 00:00:00 / 23:59:59 automatically)
./scripts/logs.sh -s "2026-03-27"
```

### Search across all rotated log files

```bash
# Search all log files (including app.log.1, app.log.2, ...)
./scripts/logs.sh -a -g "Exception"

# All files, errors only
./scripts/logs.sh -a -l ERROR
```

### Get structured JSON output (for programmatic use)

```bash
./scripts/logs.sh --json -n 10
```

Output format:

```json
[
  {"timestamp": "2026-03-27 12:25:04,270", "level": "INFO", "logger": "app", "message": "Agent started"},
  {"timestamp": "2026-03-27 12:25:05,100", "level": "ERROR", "logger": "tool", "message": "Tool call failed"}
]
```

---

## All options reference

| Option | Description | Default |
|--------|-------------|---------|
| `-n <N>` | Show last N lines | `50` |
| `-f` | Follow (real-time stream) | off |
| `-s <TIME>` | Show logs after this time | — |
| `-u <TIME>` | Show logs before this time | — |
| `-g <PATTERN>` | Filter by keyword or regex | — |
| `-l <LEVEL>` | Filter by log level: `DEBUG` / `INFO` / `WARNING` / `ERROR` / `CRITICAL` | — |
| `-p <FILE>` | Use a custom log file path | `data/logs/app.log` |
| `-a` | Search all rotated log files | off |
| `--json` | Output as JSON array | off |
| `-h` | Show help | — |

---

## Log format

Each log line follows this format:

```
YYYY-MM-DD HH:MM:SS,mmm [LEVEL] logger - message
```

Example:

```
2026-03-27 12:25:04,270 [INFO] app - Agent service started on port 8000
2026-03-27 12:25:10,512 [ERROR] tool - Tool call failed: connection timeout
```

---

## Troubleshooting

**"Log directory does not exist"**
The app has not been started yet, or `data/logs/` was deleted. Run `./scripts/start.sh` first.

**"No matching log records found"**
Your filter is too narrow. Try removing `-l` or `-g` to see all logs, then narrow down.

**Real-time follow stops showing output**
The app may have crashed. Check with `./scripts/logs.sh -l ERROR -n 20`.

---

## Note on remote usage

The WeData platform also calls `logs.sh` remotely to fetch runtime logs for display in the console. This is the same script — no separate setup is needed. When the app is deployed to the platform, the platform invokes this script with appropriate parameters automatically.
