# Advanced Deploy Options

## Overview

`deploy.sh` and `upload.sh` support a range of options for customizing the deployment. Most users only need the basic `./scripts/deploy.sh -u -y` command. Read this guide when you need more control.

---

## Exclude files from the upload package

By default, `pack.sh` respects `.gitignore` and excludes common directories like `node_modules/`, `.venv/`, `data/`, etc. Use `-e` to exclude additional paths:

```bash
# Exclude a single directory
./scripts/deploy.sh -u -y -e "docs/"

# Exclude multiple directories
./scripts/deploy.sh -u -y -e "docs/" -e "tests/" -e "notebooks/"
```

This is useful when:
- The package exceeds the 15 MB limit
- You have large test datasets or documentation you don't want deployed

---

## Verbose mode (debug output)

```bash
./scripts/deploy.sh -u -y -v
```

Shows detailed information about:
- Which files are included/excluded in the package
- API request parameters
- Response details

---

## Override workspace and app key via CLI

By default, `deploy.sh` reads `WEDATA_WORKSPACE_ID` and `WEDATA_APP_KEY` from `.env.local`. You can override them on the command line:

```bash
./scripts/deploy.sh -u -y -w <workspace-id> -a <app-key>
```

Useful when deploying to a different workspace than what's in `.env.local`.

---

## Use a custom API endpoint

```bash
./scripts/deploy.sh -u -y -H wedata.tencentcloudapi.com
```

The default endpoint is `wedata.tencentcloudapi.com`. Override this if you're using a private endpoint or a different region's endpoint.

---

## Upload a pre-built package

If you already have a `.tar.gz` package and want to upload it directly (skipping the pack step):

```bash
./scripts/upload.sh -i /path/to/my-app.tar.gz
```

---

## Upload only (no deployment)

Pack and upload code to the platform without triggering a redeployment:

```bash
./scripts/upload.sh
```

Useful when you want to stage code without immediately redeploying.

---

## Deploy only (skip upload)

Trigger a deployment using the code package already on the platform (no new upload):

```bash
./scripts/deploy.sh -y
```

Useful when the code is already uploaded and you just want to redeploy.

---

## Dry-run (preview only)

Preview what would happen without actually executing:

```bash
# Preview deploy parameters
./scripts/deploy.sh -d

# Preview upload parameters
./scripts/upload.sh -d
```

---

## Override region

```bash
./scripts/deploy.sh -u -y -r ap-chongqing
```

Default region is `ap-guangzhou`. Override when deploying to a different region.

---

## Full options reference

### `deploy.sh`

| Option | Description |
|--------|-------------|
| `-w <ID>` | Override Workspace ID |
| `-a <KEY>` | Override App Key |
| `-H <HOST>` | Override API host |
| `-r <REGION>` | Override region |
| `-u` | Upload code first, then deploy |
| `-e <PATH>` | Extra exclude pattern (can repeat) — passed to `upload.sh` |
| `-v` | Verbose output |
| `-d` | Dry-run — preview only |
| `-y` | Skip confirmation prompt |

### `upload.sh`

| Option | Description |
|--------|-------------|
| `-i <FILE>` | Use a pre-built package instead of packing |
| `-e <PATH>` | Extra exclude pattern (can repeat) |
| `-w <ID>` | Override Workspace ID |
| `-a <KEY>` | Override App Key |
| `-H <HOST>` | Override API host |
| `-r <REGION>` | Override region |
| `-v` | Verbose output |
| `-d` | Dry-run — preview only |
