# AZ036 validation follow-up independent review

2026-10-09. Read-only source review. Four app navigation keys exist in both en/zh: saveBeforeProjectSwitch, projectSwitchSaveFailed, saveBeforeProjectClose, projectCloseSaveFailed. Their messages match dirty-buffer save-first refusal and failed snapshot persistence cancellation; they do not promise that native cleanup or rollback succeeded. The names match current App callers, so ordinary en/zh users need not rely on English defaultValue. No GUI rendering or new frontend test executed by this reviewer.

chat_plan existing_binding now compares canonical PathBuf workspace.as_path() against Path::new(&project_id), preserving the former PathBuf-from-string lexical path equality without unnecessary allocation. Both operands still originate from canonical_root; registered short-circuit remains, resolver/identity behavior and Run creation policy unchanged. This is a comparison allocation/lint fix, not a permissions relaxation or new path normalization policy. No blocking defect found in this limited follow-up.

Root reported full desktop strictClippy exit1 with five existing lints; this change addresses only chat_plan cmp_owned. Four symbols/skills lints remain recorded. A subsequent command allowing named existing lints cannot be described as a full strictClippy pass; keep new task_view checks strict and report exact scope. No Cargo/test/lint run by this reviewer, and no Gate/desktop GUI acceptance inferred.

Shared chat_plan source fingerprint changes affect AZ017/AZ018/AZ020/AZ035 pins; parent must preserve prior review history and refresh to these actual sources using this limited semantic comparison review. Fingerprints:
- src/i18n/en.ts: bb8eb0541b49ed46198f7e60ffaa313dcb13272d286f9105f02b451e949d5184
- src/i18n/zh.ts: 04ff174670e9123ca34274447d7b254662a8b1189920f059ac315df5f6772a65
- src-tauri/src/commands/chat_plan.rs: 2392747b2def2b915e115159629cda076b8865ee62abcc90271a62cc5348f4a5
