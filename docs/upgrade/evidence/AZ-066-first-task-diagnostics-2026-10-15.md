# AZ-066 first-task failure guidance candidate (2026-10-15)

CodeZ now maps recognizable missing project folders, unavailable tools, and model endpoint connection failures to concrete next steps during the first task. The project error offers the folder picker; a connection failure offers Settings. Unknown failures retain their original error without a guessed diagnosis. Both locales include the guidance.

Local checks: `npm run typecheck` passed; focused first-run Vitest passed (19 tests). This only covers offline UI logic. No clean signed install, authorized model turn, native five-user walkthrough, or independent UI acceptance ran. AZ-066 remains pending.
