# Windows x64 candidate preparation: extension host build

2026-10-07. Only the existing dependency build command was run; no npm install, dependency download, version change, signing, or publishing was performed. No source fixes were needed.

`npm --prefix extension-host run build` completed with **exit 0** (exec session 3739). It invoked the existing esbuild script and generated:

| Output | Bytes | SHA256 |
| --- | ---: | --- |
| `extension-host/dist/host.js` | 99968 | `59aba2b5ca6fea94b9ffc83eee5d62db18bcb145767c89301c420dec3956a416` |
| `extension-host/dist/host.js.map` | 223307 | `4fd87f563ed8bb5351ac89ef1534fabc26fc8a99fcba158a31d70ffbdbcfd0a4` |

The console also reported successful generation of smoke.js and its source map. `node --check extension-host/dist/host.js` passed (exit 0). Tauri resource mappings still refer to both generated host.js and host.js.map and stage them as extension-host/host.js and extension-host/host.js.map.

This establishes a local JavaScript bundle and syntax check using existing installed dependencies. Esbuild transpilation is not a TypeScript typecheck, and the smoke executable was not run. No packaged installer, bundled Node execution, real extension activation, SSH, desktop GUI, signature, cross-platform acceptance, or final candidate acceptance was verified by this package. Checksums identify these output bytes, not trusted source provenance or signatures.

