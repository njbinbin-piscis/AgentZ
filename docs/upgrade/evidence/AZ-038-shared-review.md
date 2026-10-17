# AZ038 shared-source independent reviews

# AZ038 export-only independent review

2026-10-10. Current lib.rs SHA256 be80ac835872c052676752b35c6927265094a415c4921080d90c8511a57ab46d.

The new pub mod artifacts declaration is an opt-in domain export only. No initializer/global/static execution, existing environment/Run/Event/Plan/approval/recovery/details behavior, command registration or startup flow is added/altered in lib.rs. Artifacts namespace creation happens only when its explicit API is called. Existing consumers therefore retain semantic behavior while whole-lib fingerprint changes; parent must refresh prior review pins with history preserved. Subsequent artifacts implementation changes require their own independent review without changing this narrow export conclusion. No Cargo/product/ledger mutation or GUI验收 by this reviewer.


Additional narrow helper extraction: task_view_read.rs read_captured_text is crate-private and sequences existing relative(false) validation -> checked(root,rel) containment -> read_text same-handle checks. Original command retains early input validation, environment lease/fresh observation/expectedbinding and before-after scope checks; replacing its local checked/read sequence with helper is equivalent and adds no renderer authority. Future caller must supply freshly captured root under its own hostscope/lease; helper is not authorization by itself. Actual task_view_read.rs SHA256 8a38d539c9c3e3db9924169e158d348b84cd1fe6c3d03bdbf90f547489e1657a. No extra verification run.


# AZ038 scoped artifact host independent review

2026-10-10. Read-only source/registration review. Parent reports final domain6 fixtures90757 pass and domain strictClippy62880 exit0; reviewer did not rerun Cargo. Host end-to-end/GUI acceptance not inferred from those domain results.

No blocking defect found in the limited inspected adapter. Common authorized wrapper holds environment execution lease, looks up main-window opaque project scope, resolves supported existing task observation, compares full expected current PlanBinding, then executes under with_verified_scope after project identity/observation recheck, and rechecks observation afterward. Caller sends no filesystem root/payload bytes. Current authority is validated even when reading old stored binding metadata; response scope is current while metadata is historical provenance, not permission.

capture obtains bytes through same-handle, hardlink/reparse/size/UTF8-protected read_captured_text using trusted root. It rechecks observation before explicit store initialization/capture. Host constructs artifact identity by relativepath digest, version from advisory next_version and inputmetadata; renderer can supply title/kind but not bytes/digest/version. Competing captures may conflict rather than overwrite; host retry creates a new version, so this is immutable snapshots, not request-id deduplicated capture. Successful Capture returns storedmetadata only after commit.

list/read open existing readonly without artifact initialization; missing store is explicitstore_missing/empty page and missing readNone. Corrupt/incompatible storage errors propagate, not empty success. Old payload read verifies domain hash and strict UTF8 then returns passive content. No HTML execution or source path fallback. Current environment/task scope determines visibility; old different environment is not automatically included. Pagination is live bounded page, not whole-history atomic snapshot.

delete opens existing RW without CREATE, forwards accurate Deleted/AlreadyDeleted/NotFound after domain commit, and propagates DB errors. It removes captured payload/tombstones only, never live source. Missing storeNotFound and repeatAlreadyDeleted are distinguished. Post-operation observation failure can occur after successful mutation and return Err; this cannot roll back a committed capture/delete. UI must reconcile rather than claim rollback or blindly repeat capture. Held cooperative leases/scope lock narrow normal races but do not provide adversarial DB/filesystem atomicity.

mod.rs adds only pub mod task_artifacts; generate_handler adds four new scoped commands. Existing AZ017/AZ035 contracts and initialization are unchanged by registration, but shared hashes require review-history/pin refresh. New commands intentionally expose snapshot capture/delete, not write/process/tool/renderer-root permission. Existing resolver environment registry load may initialize/migrate as previously recorded, so whole host call is not globally readonly merely because artifactread uses readonlystore.

Current host test covers path-derived ID stability/case distinction and explicit delete serde only; it is not native authorized IPC/capture integration. Case-sensitive ID on Windows may create two snapshots for differently spelled path names; payload safety remains native read boundary. CodeChangeSet kind is passive text snapshot, not staged/applicable Git delta. Native scoped read tests/domainfixtures support limited candidate but full oldbinding/capture/delete UI scenarios need evidence. No GUI/fullAZ038 Gate claim.

Fingerprints:
- src-tauri/src/commands/task_artifacts.rs: d81926b188560e2fc1fd52a0014b21e8770ac92270eb377dbf960039a7c29edf
- src-tauri/src/commands/mod.rs: e3bdaa6d8a1f793d6c64c3c9a9326c5000a75f442d1378a062b519165829ad33
- src-tauri/src/lib.rs: 2516371b7cbeff732ca7747e90f00eee7addcc62848a769fbdca139b91dd3c81
