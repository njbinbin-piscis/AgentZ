---
name: "wedata-deploy"
description: "Upload code, deploy, sync, and monitor the WeData Agent App. Use when: (1) User wants to deploy, upload, publish, release, push code — '部署', '上传', '发布', '上线', '推送代码'; (2) User wants to view, search, or tail logs — 'view logs', 'check logs', '查日志', '看日志', '日志', 'logs.sh'; (3) User wants to sync or download code from the platform — 'sync code', 'update code', '同步代码', '下载代码', 'update.sh'; (4) User encounters a deploy or upload error; (5) User wants to preview deployment parameters (dry-run). Always use this skill for any deploy, log, or sync operation — even if the user doesn't say 'deploy' explicitly."
description_zh: "Wedata3.0 官方插件 —— Data+AI一体化平台，内置Wedata各种能力调用的Skills，通过DataAgents相关Skills可以快速开发调用Wedata引擎、数据、AI资源的数据智能体，并可自动部署到云端直接使用。"
version: "1.0.0"
source: codebuddy
source_plugin: "wedata3"
---

> **AgentZ note:** Configure external services under Settings → MCP Servers / Connectors. Use `api_connector`, `shell`, `file_read`, and `codebase_search` instead of vendor-specific MCP tool names.

# WeData Deploy

This skill is for the **WeData** template only.

## What this skill does

Help the user with any of these operations:

- **Deploy**: Upload local code and trigger a remote deployment
- **View logs**: Query the Agent's runtime logs (local or remote)
- **Sync code**: Download the latest code from the platform to local
- **Troubleshoot**: Diagnose and fix deploy/upload/sync errors

### Script responsibilities

| Script | Direction | Purpose |
|--------|-----------|---------|
| `deploy.sh` | Local → Platform | Core deploy script: packages, uploads, and triggers a remote deployment |
| `upload.sh` | Local → Platform | Upload only: packages and uploads code without triggering deployment |
| `update.sh` | Platform → Local | Sync only: downloads the latest code from the platform to your local machine |
| `logs.sh` | Local / Remote | Query logs: works for both local development and remote platform deployments |

> **Key distinction**: `upload.sh` stages code on the platform without deploying. `update.sh` is the reverse — it pulls platform code down to local. For a full deploy, use `deploy.sh -u -y` which combines upload + deploy in one step.

---

## Step 0: Identify what the user wants

Before doing anything, figure out the user's intent and jump directly to the right section:

| User says... | Go to |
|---|---|
| "deploy", "upload", "publish", "release", "部署", "上传", "发布" | → [Deploy workflow](#deploy-workflow) |
| "view logs", "check logs", "查日志", "看日志", "日志", "logs" | → [Log workflow](#log-workflow) |
| "sync", "update code", "download code", "同步代码", "下载代码" | → [Sync workflow](#sync-workflow) |
| Deploy/upload/sync failed with an error | → Read `references/troubleshooting.md` |

Don't run through all steps sequentially if the user only needs one thing.

---

## Deploy workflow

### Step 1: Verify `.env.local`

Read `.env.local` in the project root. Check these fields:

**Must be manually filled (not placeholder):**
- `LOCAL_credentials in AgentZ connector settings` — must not be `your-secret-id` or empty
- `LOCAL_credentials in AgentZ connector settings` — must not be `your-secret-key` or empty

**Auto-filled by platform (should already have real values):**
- `WEDATA_WORKSPACE_ID`, `WEDATA_APP_KEY`, `WEDATA_SDK_ENDPOINT`

If credentials in AgentZ connector settings are missing or placeholder → tell the user to run the **wedata-quickstart** skill first, or fill them manually. Do NOT proceed.

If platform fields are still placeholder → tell the user to run `./scripts/update.sh` to sync from the platform, or copy values from the WeData Console.

### Step 2: Check required CLI tools

```bash
command -v openssl && command -v curl && command -v jq && command -v base64
```

If any tool is missing:
- macOS: `brew install openssl curl jq coreutils`
- Linux: `apt-get install openssl curl jq coreutils`

### Step 3: Choose deployment mode

Infer the mode from what the user said:

- User said "deploy" or "upload and deploy" → **Option 2 (upload + deploy)** — this is the recommended default
- User said "upload only" → **Option 1**
- User said "deploy only" / "redeploy without uploading" → **Option 3**
- User said "preview" / "dry-run" / "what would happen" → **Option 4**

If the intent is ambiguous, ask once:

> Which mode?
> 1. 📦 Upload only — package and upload code, no deployment
> 2. 🚀 Upload + Deploy *(recommended)* — upload then trigger deployment
> 3. ⚡ Deploy only — use existing remote code, no new upload
> 4. 🔍 Dry-run — preview parameters only, nothing executed

**Option 1 — Upload only:**
```bash
./scripts/upload.sh
```

**Option 2 — Upload + Deploy (recommended):**
```bash
./scripts/deploy.sh -u -y
```

**Option 3 — Deploy only:**
```bash
./scripts/deploy.sh -y
```

**Option 4 — Dry-run:**
```bash
./scripts/deploy.sh -d
./scripts/upload.sh -d
```

> For advanced options (exclude files, verbose, override workspace/app key, custom endpoint), read `references/deploy-options.md`.

### Step 4: Handle the result

**On success:**
- Show the output summary (RequestId, WorkspaceId, AppKey)
- Tell the user to check deployment progress in the WeData Console
- Remind them: view logs with `./scripts/logs.sh`

**On failure:**
- Show the error clearly
- Read `references/troubleshooting.md` for diagnosis

---

## Log workflow

`logs.sh` works for **both local development and remote deployment**:
- **Local**: queries `data/logs/app.log` on your machine while the app is running locally
- **Remote**: the WeData platform also calls this script to fetch runtime logs for display in the console

For the full guide with all scenarios and options, read `references/logs-guide.md`.

### Quick reference

```bash
# Last 50 lines (default)
./scripts/logs.sh

# Real-time follow
./scripts/logs.sh -f

# Errors only
./scripts/logs.sh -l ERROR

# Search by keyword
./scripts/logs.sh -g "timeout"

# Time range
./scripts/logs.sh -s "2026-03-27 10:00:00" -u "2026-03-27 11:00:00"

# Search all rotated log files
./scripts/logs.sh -a -g "Exception"

# JSON output
./scripts/logs.sh --json -n 10
```

---

## Sync workflow

Use `update.sh` to download the latest code from the WeData platform to your local machine.

For the full guide (dry-run, backup, keep-extra, unpack), read `references/sync-guide.md`.

### Quick reference

```bash
# Sync from platform (reads config from .env.local)
./scripts/update.sh

# Preview what would change (dry-run)
./scripts/update.sh -d

# Sync with backup of modified files
./scripts/update.sh -b
```

---

## Example response structure

When helping the user deploy:

1. ✅ Verified `.env.local` — credentials in AgentZ connector settings filled; platform fields auto-filled
2. ✅ Verified CLI tools (openssl, curl, jq, base64)
3. 🚀 Executed: `./scripts/deploy.sh -u -y`
4. ✅ Success — RequestId: `xxx`
5. 📋 Check progress in WeData Console; view logs with `./scripts/logs.sh`

When helping the user view logs:

1. 📋 Read `references/logs-guide.md` for the right command
2. ▶️ Executed: `./scripts/logs.sh -l ERROR -n 50`
3. 📊 Showed results and highlighted key errors