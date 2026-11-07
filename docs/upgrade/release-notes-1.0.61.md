# AgentZ 1.0.61 release notes

Release date: 2026-11-07. See the 1.0.61 entry in `CHANGELOG.md`.

## Fixed

- **A sub-agent that only thinks no longer ends the task.** A thinking-only reply is not the report. The loop continues and asks the model to call tools or write the conclusion.
- **The expanded sub-agent card shows the work in progress.** Each thinking round and tool call appears on the card while it runs, and the header names the current step.
- **Browser tabs stay on the surface that opened them.** CodeZ, WorkZ, and BotZ no longer share one Chromium page. The panel can list and switch its own tabs, and a screenshot poll no longer blocks navigation.
- **Command tools describe the security level that is actually on.** Strict, balanced, and dev are named in the prompt and in timeout or cancel messages. Dev mode no longer reports an AppContainer failure when the sandbox is off.
- **PowerShell scripts are written with a UTF-8 BOM.** `file_write` and `file_edit` add the BOM to `.ps1`, `.psm1`, and `.psd1`, so Windows PowerShell 5.1 does not read them as the system ANSI code page.
