# Code Sync Guide

## Overview

Two scripts handle code synchronization between the remote WeData platform and your local machine:

| Script | Direction | Use case |
|--------|-----------|----------|
| `update.sh` | Platform → Local | Download the latest code from the platform and update local files |
| `unpack.sh` | Archive → Local | Unpack a previously downloaded `.tar.gz` archive and sync local files |

---

## When to use `update.sh`

Use `update.sh` when:

- You just created a new App on the WeData platform and need to pull the initial config (`.env.local.example` with pre-filled `WEDATA_WORKSPACE_ID`, `WEDATA_APP_KEY`, etc.)
- A teammate deployed new code to the platform and you want to sync it locally
- Your local `.env.local` is missing platform fields — running `update.sh` will refresh `.env.local.example` with the latest values

### Basic usage

```bash
# Sync from platform (reads config from .env.local)
./scripts/update.sh
```

### Preview before syncing (dry-run)

```bash
# See what would change without actually modifying files
./scripts/update.sh -d
```

### Sync with backup

```bash
# Back up any files that would be modified or deleted before overwriting
./scripts/update.sh -b
```

Backup files are saved to `.backup/` in the project root.

### Keep extra local files

```bash
# Don't delete local files that don't exist on the platform
./scripts/update.sh -k
```

Useful when you have local-only files (e.g. test scripts, notes) you don't want removed.

### Save the downloaded archive

```bash
# Download and save the .tar.gz without auto-deleting it
./scripts/update.sh -o /tmp/app-backup.tar.gz
```

### All options

| Option | Description |
|--------|-------------|
| `-w <ID>` | Override Workspace ID (default: from `.env.local`) |
| `-a <KEY>` | Override App Key (default: from `.env.local`) |
| `-H <HOST>` | Override API host |
| `-r <REGION>` | Override region |
| `-b` | Backup modified/deleted files before updating |
| `-v` | Verbose — show detailed file operation list |
| `-d` | Dry-run — preview only, no actual changes |
| `-k` | Keep extra local files (don't delete anything) |
| `-o <FILE>` | Save downloaded archive to this path |

---

## When to use `unpack.sh`

Use `unpack.sh` when:

- You already have a `.tar.gz` archive (e.g. downloaded manually or saved via `update.sh -o`)
- You want to apply a specific archive to your local files without re-downloading

### Basic usage

```bash
./scripts/unpack.sh -i /path/to/app.tar.gz
```

### Preview before unpacking

```bash
./scripts/unpack.sh -i /path/to/app.tar.gz -d
```

### Unpack with backup

```bash
./scripts/unpack.sh -i /path/to/app.tar.gz -b
```

### All options

| Option | Description |
|--------|-------------|
| `-i <FILE>` | Input archive path (required) |
| `-b` | Backup modified/deleted files before updating |
| `-v` | Verbose — show detailed file operation list |
| `-d` | Dry-run — preview only, no actual changes |
| `-k` | Keep extra local files (don't delete anything) |

---

## What sync does to your local files

The sync operation performs a three-way diff:

| Case | Action |
|------|--------|
| File exists on platform but not locally | ✚ Add |
| File exists on both, content differs | ✎ Overwrite |
| File exists locally but not on platform | ✖ Delete (unless in `.gitignore` or `-k` is set) |
| File exists on both, content identical | ⏭ Skip |

> **Note**: Files listed in `.gitignore` are never deleted during sync, even if they don't exist on the platform. This protects local-only files like `data/`, `.env.local`, virtual environments, etc.

---

## Typical workflow: first-time setup after creating an App

1. Create the App on the WeData platform
2. Clone the template repo locally
3. Run `./scripts/update.sh` to pull platform config into `.env.local.example`
4. Copy: `cp .env.local.example .env.local`
5. Fill in `LOCAL_SECRET_ID` and `LOCAL_SECRET_KEY`
6. Run `./scripts/start.sh`
