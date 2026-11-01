# Crash and Memory Growth Investigation (2026-09-28)

**English** | [简体中文](crash-memory-investigation.zh-CN.md)

> Status (2026-11-02, AgentZ 1.0.40): the fixes below are still present in code. Confirmed locations: watcher batching in `src-tauri/src/index_worker.rs` (`MAX_PENDING = 1024`, 200 ms coalescing); terminal log caps in `src-tauri/src/terminal_log.rs` (5000 lines, `MAX_BYTES` 2 MiB, `MAX_LINE_BYTES` 64 KiB); extension output caps in `src/extensions/extensionUiStore.ts` (`MAX_OUTPUT_CHARS = 512 * 1024`, `MAX_LOG_LINE_CHARS = 4096`); 10 MiB editor preview via `bounded_read` in `src-tauri/src/commands/ide.rs`; per-entry 20 000-file cap in `src-tauri/src/commands/codebase.rs` and `graph.rs`; search results clamped to 50 in `codebase.rs`; `--software-rendering` in `src-tauri/src/main.rs`; panic hook in `src-tauri/src/lib.rs`. The Windows long-run retest and the Linux manual reproduction described at the end remain open; the original black-screen/OOM root cause is still not proven.

## Code issues confirmed and fixed

- The file watcher created a native thread per change, and each thread opened SQLite and updated the index. It now uses one background worker thread per watcher, coalesces duplicate paths over 200 ms and keeps at most 1024 pending paths; on overflow it does a single full rebuild so that no file is silently missed. The worker exits when the watcher is released (a batch in progress finishes first). Start/stop share one lock to avoid duplicate creation.
- The backend terminal log only had a 5000-line limit, so output without newlines and very long lines could still grow without bound. Added a 2 MiB total and 64 KiB per-line limit that keeps the most recent content and truncates on UTF-8 boundaries.
- Frontend extension OutputChannels used unbounded string concatenation. Each channel now keeps the most recent 512 Ki UTF-16 code units; host/debug logs also cap each entry at 4096 code units while keeping the existing entry-count limit.
- When the editor's LSP start-up completed after the component had unmounted, it re-registered global providers and the WebSocket. Added a lifecycle sequence check, release of pending connections, and disconnect on failure paths too. Fixed listener leaks where the async subscriptions to the app-close event and the workspace browser event resolved after cleanup.
- Incremental indexing read the whole file before checking its size. It now checks for a regular file and its size before reading, and uses a bounded read so a file that grows after the check is still capped. The graph index uses the same read guard; the editor single-file preview is limited to 10 MiB and returns an error beyond that.
- The 20 000-file cap during index traversal was only checked on entering a directory, so a single large directory could exceed it. The cap is now checked per entry. Code search keeps only the best candidates within the top 50 instead of accumulating every matching snippet.
- The graph index previously copied the whole file-name index for every processed file. It now builds one shared read-only lookup table per rebuild, reducing repeated allocation and CPU use when opening large projects.

These are resource issues that can be confirmed from code and regression tests; without a Windows process heap dump, we cannot claim they are the sole cause of every black screen/OOM.

## Local Linux evidence

The local machine is Ubuntu 24.04.1 on VMware Virtual Platform, with system WebKitGTK 2.52.6.

Launching the system-installed `/usr/bin/agentz-desktop` produced `VMware: No 3D enabled`, `Could not get DRI3 device` and `failed to create dri2 screen`. That launch exited with 137; no Rust panic or kernel OOM record was captured. Exit code 137 alone does not prove OOM, nor does it on its own prove that the graphics errors caused the exit.

Comparison with `WEBKIT_DISABLE_DMABUF_RENDERER=1 LIBGL_ALWAYS_SOFTWARE=1`: neither the 35-second nor the subsequent 60-second observation exited early, and the EGL/DRI errors no longer appeared. The 60-second test monitored only the main process; RSS rose from about 182 MiB and levelled off around 235 MiB. This does not represent the whole WebView process tree or long-term memory stability. The existing workspace state pointed at this project, but the test did not drive the folder picker dialog, so the user's original "open project" action has not been fully reproduced.

The official WebKit issue tracker documents disabling the DMA-BUF path when it is unavailable:
https://bugs.webkit.org/show_bug.cgi?id=291332

Newly built Linux binaries support this on demand:

```sh
./agentz-desktop --software-rendering
```

Older installed versions can run the same comparison directly:

```sh
WEBKIT_DISABLE_DMABUF_RENDERER=1 LIBGL_ALWAYS_SOFTWARE=1 \
  RUST_BACKTRACE=1 /usr/bin/agentz-desktop > /tmp/agentz-linux.log 2>&1
```

This affects only that process and does not change the system graphics configuration. Hardware rendering has not been disabled by default for all Linux users. Rust panic logging (location and backtrace) was added; native WebKit crashes and SIGKILL still require system logs or process dumps.

## Regression verification and follow-up retests

Automated coverage: coalescing of duplicate file events, a bounded queue after overflowing with 100 000 paths, releasing callbacks when stopping the worker, continuous multi-byte terminal output without newlines, total-byte/line limits on logs, file-read boundaries, and extension output truncation.

Windows needs a rebuild followed by a long-running retest that separates memory growth in `agentz-desktop.exe`, `msedgewebview2.exe` and the extension host; sustained terminal output, extension log output and rapid file/project switching should each be tested separately. Linux needs repeated manual opening of the original project, editing and project switching in software-rendering mode; the current comparison only supports the rendering-compatibility hypothesis and does not yet show the original crash is fully resolved.

Final check results (at the time of the investigation): `cargo test -p agentz-desktop --lib --bins`: 87 passed, 1 ignored; `npm test`: 21 passed; `npm run typecheck` and `npm run build` passed. ESLint reported 0 errors on the modified frontend files, with pre-existing unused-disable / Hook dependency warnings; Vite reported a large-chunk notice. No new Windows installer was built or installed, and the locally installed system version was not replaced.
