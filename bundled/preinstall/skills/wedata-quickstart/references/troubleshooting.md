# WeData Quickstart — Troubleshooting Guide

Use this file to find solutions when the user encounters startup, configuration, or dependency errors.

---

## 1. Script Permission Denied

### Error
```
bash: ./scripts/start.sh: Permission denied
zsh: permission denied: ./scripts/start.sh
```

### Fix
```bash
chmod +x scripts/start.sh scripts/stop.sh
./scripts/start.sh
```

---

## 2. Port Already in Use

### Error
```
OSError: [Errno 48] Address already in use
Address already in use: ('0.0.0.0', 8000)
```

### Fix
```bash
# Find the process using the port
lsof -i :8000

# Option 1: Stop the existing WeData service
./scripts/stop.sh

# Option 2: Change the port (in .env.local)
WEDATA_APP_PORT=8001
```

---

## 3. pip install Dependency Conflict

### Error
```
ERROR: pip's dependency resolver does not currently take into account all the packages that are installed.
Could not find a version that satisfies the requirement xxx
```

### Fix

**Option 1 (Recommended): Use a virtual environment to isolate dependencies**
```bash
python3 -m venv .venv
source .venv/bin/activate   # macOS/Linux
# .venv\Scripts\activate    # Windows
pip install -r requirements.txt
```

**Option 2: Upgrade pip and retry**
```bash
pip install --upgrade pip
pip install -r requirements.txt
```

> 💡 Using a virtual environment is the recommended approach — it completely isolates the project's dependencies from your global Python environment and prevents conflicts.

---

## 4. Wrong Python Version

### Error
```
SyntaxError: invalid syntax  # usually Python 2 running Python 3 code
ModuleNotFoundError: No module named 'xxx'  # version incompatibility
python3 --version shows below 3.10
```

### Fix
```bash
# Install Python 3.10+ on macOS
brew install python@3.10
python3 --version

# Verify the correct python3 is being used
which python3
```

---

## 5. AK/SK Format Error / Authentication Failure

### Error
```
AuthFailure: SecretId is invalid
InvalidParameter: SecretId format error
```

### Common Causes and Fixes
- **Leading/trailing whitespace**: Check `LOCAL_SECRET_ID` and `LOCAL_SECRET_KEY` in `.env.local` — ensure no extra spaces
- **Using temporary keys without TOKEN**: Temporary keys require `LOCAL_TOKEN` to be filled as well
- **Expired or disabled key**: Go to [Tencent Cloud Console](https://console.cloud.tencent.com/cam/capi) to generate a new key pair

```bash
# Correct format (no quotes, no spaces)
LOCAL_SECRET_ID=<your-tencent-cloud-secret-id>
LOCAL_SECRET_KEY=<your-tencent-cloud-secret-key>
```

---

## 6. Platform Fields Empty in `.env.local`

### Error
```
WEDATA_WORKSPACE_ID is not set
WEDATA_APP_KEY is not set
```

### Cause
These fields are automatically populated by the WeData platform into `.env.local.example` when an App is created. If they are empty, it means:
1. The App has not been created on the platform yet, or
2. `upload.sh` / `update.sh` has not been run to sync the config

### Fix
- Go to the WeData platform and create an App, then run `./scripts/update.sh` to sync the latest config
- Re-run `cp .env.local.example .env.local`

---

## 7. Service Unresponsive After start.sh

### Debugging Steps
```bash
# 1. Check the logs
tail -f data/logs/app.log

# 2. Verify the process is running
ps aux | grep app.py

# 3. Try running in foreground to see errors directly
python app.py
```

---

## 8. MLflow Connection Failure (Does Not Affect Core Functionality)

### Error
```
MlflowException: Could not connect to tracking server
```

### Notes
MLflow connection failure **does not affect normal Agent operation** — only the Trace recording feature is unavailable.
- If `MLFLOW_TRACKING_URI` is not set, MLflow is automatically skipped
- If it is set but the connection fails, check network connectivity or contact the platform administrator
