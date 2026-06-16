# Troubleshooting

## Configuration errors

### Missing or placeholder AK/SK

**Symptom**: API call fails with authentication error, or `.env.local` still has `your-secret-id` / `your-secret-key`.

**Solution**:
1. Go to https://console.cloud.tencent.com/cam/capi
2. Create or copy your `SecretId` and `SecretKey`
3. Fill them in `.env.local`:
   ```
   LOCAL_SECRET_ID=<your-tencent-cloud-secret-id>
   LOCAL_SECRET_KEY=<your-tencent-cloud-secret-key>
   ```

---

### Platform fields are still placeholder values

**Symptom**: `WEDATA_WORKSPACE_ID`, `WEDATA_APP_KEY`, or `WEDATA_SDK_ENDPOINT` still show `your-workspace-id` / `your-app-key`.

**Cause**: These fields are auto-filled by the WeData platform when the App is created. If they're still placeholders, the platform hasn't synced yet.

**Solution**: Run `update.sh` to pull the latest config from the platform:

```bash
./scripts/update.sh
```

Then copy the updated values into `.env.local`:

```bash
cp .env.local.example .env.local
# Then re-fill LOCAL_SECRET_ID and LOCAL_SECRET_KEY
```

Alternatively, copy the values manually from the WeData Console → App Settings.

---

## Upload errors

### Package too large (>15 MB)

**Symptom**: Upload fails with a size limit error.

**Solution**: Exclude unnecessary directories:

```bash
./scripts/deploy.sh -u -y -e "docs/" -e "tests/" -e "data/"
```

Also check that `.gitignore` properly excludes large directories like `node_modules/`, `.venv/`, `__pycache__/`.

---

### `base64` command not found

**Symptom**: `upload.sh` fails with `command not found: base64`.

**Solution**:
- macOS: `base64` is built-in, should always be available
- Linux: `apt-get install coreutils` or `yum install coreutils`

---

## Deploy errors

### Missing CLI tools

**Symptom**: Script fails with `command not found: openssl` / `curl` / `jq`.

**Solution**:
- macOS: `brew install openssl curl jq`
- Ubuntu/Debian: `apt-get install openssl curl jq`
- CentOS/RHEL: `yum install openssl curl jq`

---

### API authentication error (AuthFailure)

**Symptom**: API returns `AuthFailure` or `InvalidSecretId`.

**Possible causes**:
- Wrong `SecretId` / `SecretKey` — verify at https://console.cloud.tencent.com/cam/capi
- Using a temporary key (`LOCAL_TOKEN`) that has expired — generate a new one
- Key has been disabled or deleted

---

### Network error / connection timeout

**Symptom**: `curl` fails with connection refused or timeout.

**Solution**:
1. Check internet connectivity
2. Verify the API endpoint is reachable: `curl -I https://wedata.tencentcloudapi.com`
3. If behind a corporate proxy, set `HTTP_PROXY` / `HTTPS_PROXY` environment variables
4. If using a custom endpoint (`-H`), verify it's correct

---

### `UnsupportedRegion` error

**Symptom**: API returns `UnsupportedRegion`.

**Cause**: The region in `.env.local` (`WEDATA_REGION`) doesn't match the region where the App was created.

**Solution**: Check the App's region in the WeData Console and update `WEDATA_REGION` in `.env.local` accordingly.

---

## Sync errors (`update.sh`)

### Download fails

**Symptom**: `update.sh` fails during the download step.

**Solution**:
1. Verify AK/SK are correct
2. Verify `WEDATA_WORKSPACE_ID` and `WEDATA_APP_KEY` are set
3. Run with verbose mode: `./scripts/update.sh -v`

---

### Local files unexpectedly deleted after sync

**Symptom**: Local files you added were deleted after running `update.sh`.

**Cause**: Files not in `.gitignore` and not on the platform are deleted during sync.

**Solution**: Add local-only files to `.gitignore` before running `update.sh`. Or use `-k` to keep all extra local files:

```bash
./scripts/update.sh -k
```

---

## Log errors

### "Log directory does not exist"

**Symptom**: `logs.sh` reports the log directory doesn't exist.

**Cause**: The app hasn't been started yet, or `data/logs/` was deleted.

**Solution**: Start the app first: `./scripts/start.sh`

---

### No log output after filtering

**Symptom**: `logs.sh -l ERROR` returns nothing.

**Cause**: No errors in the recent log window, or the filter is too narrow.

**Solution**: Remove filters to see all logs, then narrow down:

```bash
# See all recent logs
./scripts/logs.sh -n 100

# Then add filters
./scripts/logs.sh -n 100 -l ERROR
```
