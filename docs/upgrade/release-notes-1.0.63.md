# AgentZ 1.0.63 release notes

Release date: 2026-11-08. See the 1.0.63 entry in `CHANGELOG.md`.

## Changed

- **No more silent data loss.** Every action that destroys work now asks first, says what will be lost, and keeps a recoverable copy.
  - Undoing an agent turn or the review panel's pending changes backs the current files up to `.agentz/rollback-backups/` first. If the backup fails, nothing is changed.
  - Deleting a session saves the whole conversation to `.agentz/deleted-sessions/` first.
  - Restoring a checkpoint ("restore to here") saves the conversation first and asks separately whether to revert the latest turn's file edits.
  - Deleting a file or folder in the file tree moves it to `.agentz/trash/`; rename no longer overwrites an existing target and new files no longer overwrite existing ones.
  - Git discard keeps your changes under `refs/agentz/discard/<stamp>` (recover with `git stash apply`); untracked files go to trash.
  - Dangerous agent shell commands (for example `rm -rf`, `git reset --hard`) need confirmation, and are refused when there is no way to ask.
  - Clearing notifications, clearing IM history and rolling back app changes ask first. Clearing IM history now reports failures.
- **WorkZ: the "restore task snapshot" button is removed.** It replaced all files with the pre-task state in one click. WorkZ now has the same "fork" and "restore to here" per reply as CodeZ; restore always asks and backs up first.

## Fixed

- **Plan items no longer land between finished ones.** When the agent re-sent the whole plan, a new step could be placed in the middle of completed steps. New steps now always go after the last finished step; reordering unfinished steps still works.
- WorkZ interactive forms (`chat_ui`) were re-checked: each stays in its own message, and only a form with no message of its own appears at the bottom. Covered by tests.