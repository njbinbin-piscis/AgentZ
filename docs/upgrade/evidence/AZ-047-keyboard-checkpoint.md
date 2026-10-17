# AZ047 keyboard dock candidate checkpoint

2026-10-10. Single main task follows frozen AZ044. Reviewed candidates AZ040/AZ041 are code dependencies only; original acceptance/M1 remain unmet. Read current BottomPanel: console buttons lacked tab semantics, roving focus and arrow/Home/End navigation. Implemented native button tab roles, selected state, stable useId controls/labelled active panel, roving tabIndex and visible-tab keyboard navigation with focus refs. Ordinary/modified keys preserve browser behavior; extension-tab removal retains existing terminal fallback. No terminal/backend source changed.

Two helper cases cover wrap/endpoints/empty/removed IDs and ordinary keys, run50315 pending. Full component/native accessibility and Chinese/English keyboard journey not yet verified. This is a narrow dock candidate, not complete AZ047 or desktop acceptance. Remaining focus fallback when an extension disappears and keyboard operation across other workspaces/error recovery remain open.

## Actual controlled component check
50315 two helper tests passed;72759 full TypeScript exit0. Added actual BottomPanel ReactTestRenderer fixture with mocked extension/native subviews: arrow wrapping calls tab change and focuses target; controlled rerender updates selected/tabIndex/panel labels, language mock changes labels, modified arrow remains untouched.65776 three tests passed,3.69s. Final TypeScript43132 and scopedlint63796 pending. Language mocks only verify labels consumer wiring, not actual English/Chinese translation or screenreader/native focus journey. Independent four-source review requested; original full AZ047 scope remains pending.


## Delayed terminal focus guard
Independent dock review found existing60ms terminal activation stole keyboard tab focus. Added same-dock tablist focus guard to both initial post-create and delayed visible terminal focus paths; standalone and unrelated dock focus do not suppress existing behavior. Added actual TerminalPanel controlled-timer fixture with mocked focused dock ownership.16686 combined15 tests passed (2 navigation,1 dock,6 terminal component,6 lease),3.90s. Final TypeScript74723 andlint28988 pending; new exact-source independent follow-up underway. AZ044 pinned Terminal source changed deliberately; all affected records will refresh atomically after review, no ledger mutation while fingerprints stale. NativeDOM/screenreader/bilingual realjourneys remain unrun.


Final actual74723 full TypeScript exit0;28988 lint exit0 with2 existing Terminal FastRefresh warnings.15 controlled tests passed; no native acceptance.
