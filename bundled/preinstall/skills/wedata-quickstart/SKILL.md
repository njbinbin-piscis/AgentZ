---
name: "wedata-quickstart"
description: "Smart guide for setting up, configuring, starting, and troubleshooting the WeData Agent local development environment. Trigger when the user says 'how do I run this', 'startup error', 'start.sh permission denied', 'pip install failed', 'how to fill AK/SK', 'port already in use', 'how to configure .env.local', 'just cloned the repo', 'can't start', 'local development', 'quickstart', 'how to run', 'environment setup', 'credentials config', 'SECRET_ID', 'SECRET_KEY'. Also applies when the user encounters any startup/configuration/dependency errors. When the user already has a working environment and just wants to use MCP tools, let wedata-local-mcp-example handle it instead. When in doubt, use this skill."
description_zh: "Wedata3.0 官方插件 —— Data+AI一体化平台，内置Wedata各种能力调用的Skills，通过DataAgents相关Skills可以快速开发调用Wedata引擎、数据、AI资源的数据智能体，并可自动部署到云端直接使用。"
version: "1.0.0"
source: codebuddy
source_plugin: "wedata3"
---

> **AgentZ note:** Configure external services under Settings → MCP Servers / Connectors. Use `api_connector`, `shell`, `file_read`, and `codebase_search` instead of vendor-specific MCP tool names.

# WeData Quickstart

This skill is for the **WeData** template project only.

## Reference Files

- `references/env-fields.md` — Full description of all `.env.local` config fields (read when the user asks about specific field meanings)
- `references/troubleshooting.md` — Common error diagnosis handbook (read when the user encounters errors)

---

## First: Assess the User's Current State

**Before taking any action, evaluate which stage the user is at** to avoid repeating already-completed steps:

| User Situation | Jump To |
|----------------|---------|
| Encountered a specific error (permission, port, pip, credentials in AgentZ connector settings auth failure, etc.) | Go directly to the "Troubleshooting" section |
| `.env.local` already exists, just credentials in AgentZ connector settings not filled | Go directly to "Step 3: Fill in credentials in AgentZ connector settings" |
| Environment and config are OK, just unsure how to start | Go directly to "Step 4: Install Dependencies and Start" |
| Just cloned, nothing set up yet | Start the full flow from "Step 1: Check Runtime Environment" |

---

## Full Setup Flow (From Scratch)

### Step 1: Check Runtime Environment

```bash
python3 --version   # requires 3.10+
pip --version
```

- If `python3` is missing or below 3.10, help the user install it first:
  ```bash
  # macOS
  brew install python
  ```
  Windows users: download from [python.org](https://www.python.org/downloads/), make sure to check "Add to PATH" during installation.

- If `pip` is missing, resolve that before continuing.

### Step 2: Create `.env.local`

Check whether `.env.local` already exists in the project root:

- **Already exists** → do NOT overwrite it, proceed to Step 3
- **Does not exist** → create it from the template:
  ```bash
  cp .env.local.example .env.local
  ```

> `.env.local` is the local development config file. When it exists, the app runs in local mode. It is git-ignored and will not be committed.

### Step 3: Fill in credentials in AgentZ connector settings

**Read the current `.env.local` file** and check the fill status of the following fields:

A field is considered **unfilled** if:
- It is missing entirely, or its value is empty (e.g. `LOCAL_credentials in AgentZ connector settings=`)
- Its value is still a placeholder (contains `<your`, `your_`, `xxx`, `TODO`, or matches `.env.local.example`)

Display status (❌ unfilled / ✅ filled):

> 🔧 **Configuration Check**
>
> **Only credentials in AgentZ connector settings needs to be filled manually.** All other platform fields are automatically populated by the WeData platform when the App is created.
>
> - {❌/✅} `LOCAL_credentials in AgentZ connector settings`: Tencent Cloud SecretId
> - {❌/✅} `LOCAL_credentials in AgentZ connector settings`: Tencent Cloud SecretKey
> - `LOCAL_TOKEN`: *(optional)* Temporary credential token — only needed when using temporary keys, leave blank for permanent keys
>
> ✅ Other platform fields (`WEDATA_WORKSPACE_ID`, `WEDATA_APP_KEY`, `WEDATA_REGION`, etc.): auto-filled by the platform, no action needed.

If credentials in AgentZ connector settings is not filled, guide the user to obtain credentials:
- Console URL: `https://console.cloud.tencent.com/cam/capi`
- Recommended: use a sub-account with least-privilege principle

If all required fields are filled:
> ✅ **All required fields are configured. You're ready to start!**

> Need details on all fields? See `references/env-fields.md`

### Step 4: Create Virtual Environment and Install Dependencies

Using a virtual environment is **strongly recommended** — it isolates project dependencies and avoids polluting your global Python environment.

```bash
# Create a virtual environment (only needed once)
python3 -m venv .venv

# Activate the virtual environment
source .venv/bin/activate        # macOS/Linux
# .venv\Scripts\activate         # Windows

# Install dependencies
pip install -r requirements.txt
```

> 💡 **You need to activate the virtual environment every time you open a new terminal session** before running the project.
> You can tell it's active when you see `(.venv)` at the start of your terminal prompt.

```bash
# Start the service (background daemon mode)
./scripts/start.sh
```

After startup:
- Access URL: `http://localhost:8000` (or the `WEDATA_APP_PORT` configured in `.env.local`)
- Log path: `data/logs/app.log`

**Service management commands:**

| Command | Description |
|---------|-------------|
| `./scripts/start.sh` | Start the service (background mode) |
| `./scripts/stop.sh` | Stop the service |
| `python app.py` | Run in foreground (debug mode, see errors directly) |

---

## Troubleshooting

When errors occur, read `references/troubleshooting.md` for solutions.

Quick reference:

| Error | See |
|-------|-----|
| `Permission denied: ./scripts/start.sh` | troubleshooting.md §1 |
| `Address already in use: 8000` | troubleshooting.md §2 |
| `pip install` dependency conflict | troubleshooting.md §3 |
| Wrong Python version | troubleshooting.md §4 |
| `AuthFailure` / credentials in AgentZ connector settings authentication failure | troubleshooting.md §5 |
| Platform fields (`WEDATA_APP_KEY`, etc.) are empty | troubleshooting.md §6 |
| Service unresponsive after startup | troubleshooting.md §7 |
| MLflow connection failure | troubleshooting.md §8 |

---

## Next Steps

Once the environment is running:

1. **Discover available tools** — Use the **discover-tools** skill to find MCP Servers in your workspace
2. **Test locally** — Visit `http://localhost:8000` and chat with the Agent

---

## Example Response Format

When helping the user, prefer a concise checklist format:

```
1. ✅ Checked python3 (3.11.x) and pip — requirements met
2. ✅ .env.local already exists — skipped creation
3. 🔧 Checking credentials in AgentZ connector settings configuration:
   - ❌ LOCAL_credentials in AgentZ connector settings: not filled
   - ❌ LOCAL_credentials in AgentZ connector settings: not filled
   - ✅ Other platform fields: auto-filled by platform
   ⚠️ Please fill in credentials in AgentZ connector settings before proceeding
4. ⏳ Waiting for user to fill in credentials in AgentZ connector settings...
```