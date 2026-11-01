# Contributing to AgentZ

**English** | [简体中文](CONTRIBUTING.zh-CN.md)

Thanks for your interest in AgentZ. This guide covers how to report problems, set up a build, and get a change merged.

## Reporting issues

- Search [existing issues](https://github.com/njbinbin-piscis/AgentZ/issues) first.
- Include the AgentZ version (from the installer name or the GitHub release you installed), your OS, the model provider, and steps to reproduce.
- Logs live in `logs/` under the config directory (see [Configuration and data](README.md#configuration-and-data)). Remove API keys, tokens and private file contents before attaching anything.
- Do **not** report security vulnerabilities in public issues. See [SECURITY.md](SECURITY.md).

## Development setup

Prerequisites, Linux packages and the main commands are in the [Development](README.md#development) section of the README. In short:

```bash
npm install
npm run tauri dev
```

The kernel is compiled from `vendor/piscis-engine` (wired in through `[patch]` in the root `Cargo.toml`). Kernel changes must also land in the upstream [piscis-engine](https://github.com/njbinbin-piscis/piscis-engine) repository, and the pinned tag in `Cargo.toml` must be bumped, so the vendored copy and upstream stay identical.

## Before opening a pull request

Run the same checks CI runs (`.github/workflows/ci.yml`):

```bash
node scripts/upgrade-version-check.mjs
npm run lint
npm run typecheck
npm test
npm run test:e2e
npm run build
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

If you touch bundled skills or templates under `bundled/preinstall/`, also run `npm run verify:preinstall` and `npm run lint:preinstall -- --strict`.

## Pull request guidelines

- Keep a pull request focused on one change. Split unrelated fixes.
- Add or update tests for behaviour changes.
- Match the style of the surrounding code. The Rust workspace treats warnings as errors.
- User-visible strings go through i18n: add keys to both `src/i18n/en.ts` and `src/i18n/zh.ts`.
- Add an entry under the upcoming version in [CHANGELOG.md](CHANGELOG.md) for user-visible changes ([Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format).
- Write commit messages in the imperative mood with a short prefix where it helps, e.g. `fix: ...`, `feat: ...`, `docs: ...`.
- Never commit API keys, tokens or other secrets.

## Documentation

English documents are canonical. A Chinese translation sits next to the English file as `<name>.zh-CN.md`, with a language switcher under the title. When you change an English document that has a translation, update the translation in the same pull request, or note in the pull request that it needs updating. If the two disagree, the English version wins.

Release records under `docs/upgrade/` (release notes, checklists, evidence) are historical and stay in their original language.

## Releases

Releases are cut by maintainers by pushing a `v*` tag; see [RELEASE.md](RELEASE.md) and the Development section of the README for the version-sync checklist.

## License

By contributing, you agree that your contributions are licensed under the [Apache License 2.0](LICENSE), the license of this repository.
