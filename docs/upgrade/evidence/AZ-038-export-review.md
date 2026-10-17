# AZ038 export-only independent review

2026-10-10. Current lib.rs SHA256 be80ac835872c052676752b35c6927265094a415c4921080d90c8511a57ab46d.

The new pub mod artifacts declaration is an opt-in domain export only. No initializer/global/static execution, existing environment/Run/Event/Plan/approval/recovery/details behavior, command registration or startup flow is added/altered in lib.rs. Artifacts namespace creation happens only when its explicit API is called. Existing consumers therefore retain semantic behavior while whole-lib fingerprint changes; parent must refresh prior review pins with history preserved. Subsequent artifacts implementation changes require their own independent review without changing this narrow export conclusion. No Cargo/product/ledger mutation or GUI验收 by this reviewer.


Additional narrow helper extraction: task_view_read.rs read_captured_text is crate-private and sequences existing relative(false) validation -> checked(root,rel) containment -> read_text same-handle checks. Original command retains early input validation, environment lease/fresh observation/expectedbinding and before-after scope checks; replacing its local checked/read sequence with helper is equivalent and adds no renderer authority. Future caller must supply freshly captured root under its own hostscope/lease; helper is not authorization by itself. Actual task_view_read.rs SHA256 8a38d539c9c3e3db9924169e158d348b84cd1fe6c3d03bdbf90f547489e1657a. No extra verification run.
