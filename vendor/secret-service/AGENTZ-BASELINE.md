# AgentZ Secret Service dependency

Source: crates.io `secret-service` 5.1.0, upstream commit `7e8f960c767d34d961d82913a2eb0ce3bce09701`, retrieved from the local Cargo registry. Original archive SHA-256: `9a62d7f86047af0077255a29494136b9aaaf697c76ff70b8e49cded4e2623c14`. Upstream MIT and Apache licenses are preserved.

AgentZ changes:

- Add opt-in `agentz-no-prompts`; both asynchronous and blocking prompt helpers return `Error::Prompt` before building a prompt proxy or invoking Prompt. The Linux kernel dependency explicitly enables this feature and the pure Rust encrypted-session backend.
- Entropy failure during DH key generation or AES IV generation returns `Error::Crypto` instead of panicking.
- Add an independent workspace boundary; normalized dependency metadata otherwise remains upstream.

AgentZ does not use unlock, create-collection, session-only collection fallback, or plaintext session fallback. This source change requires Linux typechecking and actual service acceptance; Windows tests alone do not validate it. Validation and limitations are recorded in `docs/upgrade/evidence/AZ-078-secret-service-adapter.md`.
