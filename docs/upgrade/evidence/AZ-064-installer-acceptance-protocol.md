# AZ-064 clean-machine installer acceptance card (prepared 2026-10-18)

This card defines evidence to collect for each platform and architecture claimed in the release scope. It is not an installation result. Run on a disposable clean test machine/account after freezing a candidate; never infer trust or runtime support from a successful bundle build. Keep the candidate workflow separate from the public `release.yml`, whose readiness prerequisite is documented in `AZ-064-2026-10-18-publication-guard.md`.

Before each run, record operator, UTC time, OS build and architecture, fresh VM/image or profile identifier, source commit and dirty-state declaration, bundle filename/size/SHA-256, application version, and the exact release channel. Record each command, exit code, and redacted log or screenshot path with SHA-256. Preserve both positive and negative results; a skipped test needs a reason and does not count as a pass.

| Phase | Required observation |
| --- | --- |
| Integrity and trust | Recompute bundle hash from the tested bytes. Verify the platform signature, certificate chain, signer identity and timestamp/notarization where applicable. Record untrusted or unavailable verdicts explicitly. |
| Fresh install | Install from the frozen bundle into a clean profile. Record package manager/installer result, installed files and version, first-launch result and time to usable window. |
| Functional smoke | Open a disposable project, create a bound task, run the agreed no-external-side-effect fixture, close/reopen and verify task, artifact and environment identity. Record logs and resource paths. |
| Upgrade | Install a supported older version with a disposable state fixture, then upgrade to the candidate. Verify settings, encrypted credentials, task/artifact binding and schema behavior. Do not use real user data. |
| Refusal/recovery | Exercise one interrupted install or downgrade/unsupported schema refusal and the documented recovery path; preserve the pre-upgrade state. Unknown or partial results require reconciliation. |
| Uninstall | Remove the application, record installer result and which user data remains by policy. Verify no orphaned app process or autostart entry; never delete data to make the check pass. |
| Restore | Use AZ-063's accepted protected backup procedure and restore to a new location, then check the restored app state. The current offline backup candidate alone is insufficient. |

Platform-specific trust receipts:

- Windows: `Get-FileHash` on the exact NSIS executable; `Get-AuthenticodeSignature` status, signer chain and timestamp. Repeat on the installed executable. Test both x64 and ARM64 packages on matching machines if both are claimed. SmartScreen bypass is not a trust pass.
- macOS: `shasum -a 256`, `codesign --verify --deep --strict --verbose=2`, `spctl --assess --type execute --verbose=4` and notarization ticket/staple validation on the exact app. Record signed Team ID and Keychain identity behavior after upgrade. Test Apple Silicon and Universal claims separately.
- Linux: record `.deb`/AppImage hashes and the actual package or repository signature scheme chosen for the release. Run install/launch/uninstall on each claimed distro/architecture; a CI-built unsigned file is not a verified signature.

Stop release expansion on an untrusted signature, install failure, environment crossover, unrecoverable migration, plaintext secret fallback, authorization bypass or an unknown side effect. Record issue ID, owner, severity and a rollback decision. The named release owner must link completed cards and the AZ-069 decision before changing any support or Gate status. Certificates, clean machines and real platform runs remain external blockers.
