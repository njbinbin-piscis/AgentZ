# AZ038 artifact attack/regression matrix

2026-10-10. Read-only design/source review of developing artifacts.rs, existing readonly Run/Event/Plan stores and archive backup behavior. No Cargo/API/GUI or shared code/ledger changed. Cases below are required/proposed fixtures, not passed results.

## Early contract issue reported

Physical delete currently removes immutable version identity; capturing same task/environment/artifact/version again can replace old content under the same reference. Reported to root and author. Preserve version identity with tombstone or reject reuse by a persistent independent identity record; do not claim immutable versions while permitting this substitution. A new artifact ID/version should be explicit. Historical snapshot read is not current execution permission: current scoped host authority may retrieve old binding metadata within task/environment, but must never turn old metadata into a capability.

| Boundary | Concrete fixture | Expected observation |
|---|---|---|
| Frozen payload | capture v1; mutate/delete live source; reopen/read v1 | exact original bytes/digest/oldmetadata; no live path reread |
| Two versions | same artifact v1/v2 different content, old read after environment binding version change | both immutable; oldmetadata remains old; no fake current authority |
| Duplicate | same key exact metadata/bytes; change title/type/digest/content | Existing for identical intent, conflict for changed immutable intent |
| Delete/recreate | delete v1 then capture sameidentity differentbytes | reject reuse/tombstone; stale reference cannot silently resolve newcontent |
| Cross scope | same id/version another project/task/environment | reject or true missing; never return another scope bytes |
| Fake binding | renderer fabricates validhash/project text without hostsession | host freshscope check rejects before storecall; domain syntax is not permission |
| Blob corruption | SQL mutate payload alone/digest/bytes/metadataidentity/storageclass TEXT | error not Missing/empty; no repair and no content publication |
| Bounds | empty, exact1MiB, plus1, oversizeDBblob, metadata>16KiB, scope32MiB quota | exactbounds; preflightlength before allocation; overflow rollback |
| Concurrency | twoconnections capture sameversion differentpayload | exactlyone immutable winner; loser conflict; no duplicate silently |
| Delete failure | DELETE triggerABORT; readonlyconnection delete; commitlocked/failure | Err, captured row remains; no Deleted/falseUI success |
| Deletion success | captured payload delete, live source unchanged; repeatdelete | accurate Deleted vs NotFound; shared livefile untouched |
| Schema absence | missingDB/directory; readonlyopen/query no namespace | noCREATE/no migration, bytes unchanged; explicit Missing only |
| Incompatibility | partialschema, empty/version0/futuremetadata, view, PKmissing, nullable/wrongtype columns | failclosed, no normalization/repair, bytes unchanged |
| Identity/timestamps | invalidversion/hash/negativecreated/controlid/relativeescape | reject before mutate; no malformed record persisted |
| Snapshot backup | existing SQLite file with WAL writes; quiesced backup/reopen original+copy | preserve committed rows, metadata/payloaddigest; no nakedmainfile copy declared safe |
| Host late change | revoke scope/changeenvironment duringcapture/read/delete await | rejectpublish/commit per actual authorization boundary, no oldresponseanother task |
| Presentation | webpage/script/binary/media mismatch, artifacttextcontainsHTML | passive preview only, no scripts/network/tools automatically executed |

## Existing conventions to preserve

Readonly stores use existing-only flags and strict independent namespace metadata; absent schema is not a corrupt schema repaired by query. Immediate transactions and commit-before-success serialize mutations. Archive restore refuses existing destination and retains a failed checkout for diagnosis rather than destructive cleanup; that source/Git archive behavior is not a SQLite online backup API. SQLite backup must account for WAL/activeconnections or use proper backup mechanism/quiescence and compare rows/digests; no migration/rollback capability inferred from copying bytes.

Artifact namespace is independent from Run user_version; caller-supplied PlanBinding validation in a standalone artifact store does not prove authoritative task/environment enrollment. Host opaque scope lookup, supported existing session, held environment lease and fresh expectedbinding checks are separate prerequisites. Historicalbinding metadata may be returned as provenance without requiring it to equal currentbinding; deletion/write authorization must still be current. Do not silently recapture an outside path or grant action authority from artifact title/path/HTML.

Code ChangeSet as bounded snapshot is not full Git staging/index/merge application. Noncode Document/Report/Webpage/Validation kinds should share the same immutablepayload/identity/accuratedelete tests; a media_type is descriptive until safe renderer validation. Full GUI, concurrent filesystem capture, native link containment, provider/export execution and backup/rollback acceptance remain deferred. Parent owns final tests/review pins and tracking.
