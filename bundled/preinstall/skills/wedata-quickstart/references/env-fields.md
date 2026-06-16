# WeData Quickstart — Environment Fields Reference

This file provides a complete description of all `.env.local` configuration fields.

> **Key principle**: The WeData platform automatically populates all platform-related fields into `.env.local.example` when an App is created.
> After running `cp .env.local.example .env.local`, **only AK/SK needs to be filled manually** — all other fields are already set.

---

## Fields That Must Be Filled Manually

| Field | Required | Description |
|-------|----------|-------------|
| `LOCAL_SECRET_ID` | ✅ Required | Tencent Cloud SecretId — obtain from [CAM Console](https://console.cloud.tencent.com/cam/capi) |
| `LOCAL_SECRET_KEY` | ✅ Required | Tencent Cloud SecretKey — paired with SecretId |
| `LOCAL_TOKEN` | ❌ Optional | Temporary credential token — **only fill when using temporary keys**, leave blank for permanent keys |

---

## Platform Auto-Filled Fields (No Manual Configuration Needed)

The following fields are automatically written into `.env.local.example` by the WeData platform when the App is created, and synced via `upload.sh` / `update.sh`.

| Field | Description |
|-------|-------------|
| `WEDATA_WORKSPACE_ID` | Workspace ID |
| `WEDATA_APP_KEY` | App Key assigned by the platform |
| `WEDATA_REGION` | Deployment region (e.g. `ap-chongqing`) |
| `WEDATA_LLM_ENDPOINT` | LLM model gateway URL (used by `AsyncWedataOpenAI`) |
| `WEDATA_MCP_ENDPOINT` | MCP Server base URL |
| `WEDATA_SDK_ENDPOINT` | WeData SDK API endpoint (TC3-HMAC-SHA256 signing) |
| `MLFLOW_TRACKING_URI` | MLflow remote tracking URL (auto-filled after associating an experiment) |
| `MLFLOW_EXPERIMENT_ID` | MLflow experiment ID (auto-filled after associating an experiment) |

---

## Optional Local Custom Fields

| Field | Default | Description |
|-------|---------|-------------|
| `WEDATA_APP_PORT` | `8000` | Local service port — change if 8000 is already in use |

---

## Field Format Rules

```bash
# ✅ Correct: no quotes, no spaces
LOCAL_SECRET_ID=<your-tencent-cloud-secret-id>
LOCAL_SECRET_KEY=<your-tencent-cloud-secret-key>

# ❌ Wrong: leading space
LOCAL_SECRET_ID= <your-secret-id>
LOCAL_SECRET_KEY=xxx  

# ❌ Wrong: quoted value (quotes become part of the value)
LOCAL_SECRET_ID="<your-secret-id>"
```

---

## About MLflow

- `MLFLOW_TRACKING_URI` and `MLFLOW_EXPERIMENT_ID` are auto-filled by the platform (direct connection mode)
- **No local MLflow Server needs to be started manually**
- If these fields are empty or the remote server is unreachable, the Agent still runs normally — only the Trace recording feature is unavailable
