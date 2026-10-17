# AZ036 native task-file and WorkZ viewer independent review

2026-10-10. Read-only source review, no Cargo/API/GUI. Root fixture57843 underway; no result claimed here. Source freeze respected.

## Concrete must-address boundary

Current Windows read_text only checks metadata regular/reparse/size and handle final path within root. It does not reject hard-linked files. A same-volume hardlink inside workspace to an outside file can have a handle final name inside root, so lexical final-path containment alone does not exclude that alias. Reported immediately to root. If claiming link confinement consistent with sandbox, use same opened-handle link-count query and reject nNumberOfLinks>1, with a real same-volume hardlink fixture; otherwise explicitly disclose this unsupported confinement boundary. No exploit/test executed by this reviewer.

## Other reviewed behavior

Relative path rejects parent/dot/empty component, control, drive/ADS/scheme/backslash and Windows trailing-dot/whitespace alias forms. Captured host lookup uses main-window opaque scope, supported existing task binding and held environment lease. Expected full PlanBinding is checked after async resolve; with_verified_scope checks project identity and observations before/after IO. No renderer root or legacy unrestricted IDE route is forwarded. Windows text read opens root/file no-share-delete/no-share-write with final-component reparse handling, verifies root/file final handles and raw UTF8 size<=1MiB, rejects NUL/nonUTF8/nonregular and bounds read length. NonWindows text read is explicitly unsupported.

Directory listing is pathname-based recursive enumeration. It checks each reparse/ canonical containment, node1000/depth bounds and safe relative names, but checks and enumeration are not one atomic handle operation: concurrent directory replacement can race metadata/canonical/read_dir, and no hostile directory lifetime containment proof exists. Root/env scope lease is cooperative metadata protection, not adversarial filesystem snapshot. Lists can change, lose entries or reject midflight; no atomic directory snapshot, cross-platform or TOCTOU immunity claimed.

WorkZ entry is only nonteam/nonpool with matching selected project/canonical scope and current session. fileViewKey binds canonical project/session and remounts for another task; opening/closing viewer does not set project/session or create Run. Viewer scopehook/request key includes handle/project/session; mismatched-key render masks prior content before effect, cleanup increments generation, IO uses generation+operation guard. Text renders in pre, not HTML, and no Send/write/terminal/model action is exposed. Late response cannot populate visible other-task state. Listing recursively includes descendants, so UI directory labels are descriptive rather than immediate-child-only tree.

New commands module + two generate_handler list/read registrations only expose the fresh scoped read API; no existing AZ017/AZ035 handler initialization behavior changes. Shared pins must refresh to actual hashes. Full GUI interactions, scope revocation timing fixture through actual IPC, native adversarial directory races and hardlink handling remain acceptance items; helper tests do not validate entire desktop flow.

Reviewed pre-fix fingerprints:
- src-tauri/src/commands/task_view_read.rs: 0aa948b9000950db5efe705da1cef45a57d847faf513d1a081a39fe8eab6fb3b
- src-tauri/src/commands/mod.rs: ccab8c3717a612c8e7049c0aa2601d7ee34b814ea6068d2f60d29b3f5a122619
- src-tauri/src/lib.rs: a4444a81d65b643baaf6ddad9e7d738425cbe8e596c5c59efa6c14728d19835d
- src/workspaces/workz/index.tsx: 74ee339e609c7d3303c484b33c3542bc1b6198f3c1e5a69d654f9a4adfca1503
- src/workspaces/codez/TaskFileViewer.tsx: 0864228c8da7c7305c5698d805033a1c3b8aaa5f21d872f90d098ce42a5a4cf2


## Final hardlink-fix independent re-review

The preceding hardlink finding describes the earlier version and is now addressed in this frozen source. require_single_link queries GetFileInformationByHandle on the same opened file handle before reading content. Its repr(C) DWORD/FILETIME field order matches BY_HANDLE_FILE_INFORMATION including number_of_links. Failed query returns an error before reading uninitialized data; successful count must equal1, so unknown0 and multi-link reject. The normal metadata/final-path/budget checks remain unchanged and no share-write/delete permission was added.

New real same-volume fixture creates an outside original and inside alias, checks direct handle count and read rejection, preserves outside original content and removes only fixture alias. This fixture has not been run by this reviewer; root42179 rerun was compiling at review time. The specific static preexisting hardlink disclosure is closed by inspected same-handle validation. No blocking defect remains in the limited scoped Windows text-read/viewer candidate reviewed here. This does not prove adversarial concurrent object lifetime containment, atomic directory enumeration, GUI or cross-platform runtime acceptance.

Final fingerprints replace the earlier pre-fix list for parent shared-pin refresh while retaining historical finding:
- src-tauri/src/commands/task_view_read.rs: ad1c51ddb8123bf4790b36aa9d98bab3a13ef56fbd56ec87f4a64f13c67f25e8
- src-tauri/src/commands/task_view.rs: 18fbfe84c79ea825c0d4a9da8036b63436bdbc3acf80c6ecb4fedc1ae15970cf
- src-tauri/src/commands/mod.rs: ccab8c3717a612c8e7049c0aa2601d7ee34b814ea6068d2f60d29b3f5a122619
- src-tauri/src/lib.rs: a4444a81d65b643baaf6ddad9e7d738425cbe8e596c5c59efa6c14728d19835d
- src/workspaces/workz/index.tsx: 74ee339e609c7d3303c484b33c3542bc1b6198f3c1e5a69d654f9a4adfca1503
- src/workspaces/codez/TaskFileViewer.tsx: b3913a6fba9419721254bbfe5c1c60ea7deb8ecf0a1a9b6eaa11f098bf41b1f1


## Final candidate entry/display follow-up (immutable review append)

Root reports42179 actual11 host fixtures passed including hardlink,3.73s; reviewer did not rerun. WorkZ openTask now clears fileViewKey immediately after incrementing request identity, before asynchronous history/environment fetch; this removes the previous task viewer during new-task loading. Same current foreground live-task early return occurs before this clear, preserving its viewer and stream. No project/session authority change or new read permission introduced.

TaskFileViewer presentation changes only bound section height45vh with scrolling and pre-wrap/overflowWrap; content remains text children, no HTML evaluation. No behavioral/authorization regression found in these two narrow changes. Actual GUI/layout remains unverified. Final shared source pins:
- src/workspaces/workz/index.tsx: 2db994e386d59fbcc6d79a75433bc12fe644dbdafda130c04b9840153f944811
- src/workspaces/codez/TaskFileViewer.tsx: b3913a6fba9419721254bbfe5c1c60ea7deb8ecf0a1a9b6eaa11f098bf41b1f1
