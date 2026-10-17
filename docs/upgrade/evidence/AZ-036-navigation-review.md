# AZ036 navigation result guard independent review

2026-10-09. Read applicable repository instructions search (no AGENTS.md found in repo or existing C:/, C:/Projects parent locations), AZ-036-routing-design.md and current App.tsx switchProject/openProjectInCodeZ. No production/ledger modification, build/test or GUI execution by this reviewer.

Current App.tsx SHA-256 e0b915a42ab3ba7896db4b5f0796e229577ec1388d03f6ee8f73fd82f3e12efd.

The change is present: openProjectInCodeZ now returns immediately when awaited switchProject(dir) is falsy, before setMode(codez). switchProject returns false on navigation-busy and rejected terminal confirmation, true on same-project and completed switching, and undefined after its caught exception. Thus busy/cancel/error does not change workspace mode; successful and same-project navigation retains previous CodeZ mode behavior. Await still serializes mode selection after the switch attempt. This closes the specific ignored-result defect without authorizing task routing or bypassing existing mutual exclusion. No blocking defect found in this narrow change.

Existing switchProject error behavior can still have earlier partial side effects (e.g. terminals destroyed before workspace persistence fails). This guard does not undo those preexisting actions; it prevents claiming successful CodeZ routing. Dirty-buffer prompts, managed task workspace versus authority project split, preserved WorkZ session identity, late task descriptor generation and real same-task roundtrip remain AZ036 design requirements, not delivered by this line change. No GUI acceptance or full AZ036 completion inferred.

Shared App fingerprint changed and affects earlier review manifests that pin App.tsx; root should refresh them against this limited independent guard review. No claim that all App behavior was retested, or that prior exact hashes remain valid. Product source is frozen for root validation after this review.
