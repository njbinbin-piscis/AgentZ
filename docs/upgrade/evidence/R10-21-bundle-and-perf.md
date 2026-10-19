# R10-21 Evidence: Production Bundle Chunking & Performance Gate

- **Task ID**: `R10-21` (`r10-21-bundle-and-perf`)
- **Date**: `2026-10-19`
- **Status**: `PASSED`

## 1. Summary of Changes

1. **Vite Production Minification & Manual Chunking (`vite.config.ts`)**:
   - Enabled `minify: "esbuild"` for production builds.
   - Configured `build.rollupOptions.output.manualChunks` to split heavy third-party bundles out of the main entry chunk:
     - `app-vendor`: `["react", "react-dom", "i18next", "react-i18next", "@tauri-apps/api"]`
     - `monaco`: `["monaco-editor", "@monaco-editor/react"]`
     - `xterm`: `["@xterm/xterm", "@xterm/addon-fit", "@xterm/addon-web-links"]`
     - `markdown`: `["react-markdown", "remark-gfm", "remark-math", "rehype-highlight", "rehype-katex", "rehype-raw", "rehype-sanitize", "highlight.js"]`
     - `mermaid`: `["mermaid", "elkjs", "reactflow"]`
2. **Automated Bundle & Performance Gate (`scripts/perf-harness.mjs`)**:
   - Validates `vite.config.ts` minification and `manualChunks` configuration.
   - Parses `dist/index.html` to locate the entry script (`./assets/index-*.js`) and asserts its size is `< 1.2 MB` raw (`706,750 bytes` / `685.11 kB`, `211.55 kB` gzip).
   - Executes a 50-cycle resource open/close balance check across `listener`, `terminal`, `editorModel`, `extHostSession`, and `ptySession` counters (`live === 0`, `unmatchedClose === 0`) and verifies RSS memory stays well below the `450 MB` budget (`53.62 MB`).

## 2. Verification Results

- `npx tsc --noEmit && npx vite build`: `0` TypeScript errors; `142` split JS assets generated in `dist/assets/`.
- `node scripts/perf-harness.mjs`:
  - Entry chunk: `index-CBfaEzj6.js` (`706,750 bytes` raw, `211.55 kB` gzip — well within `< 1.2 MB` cold-start budget).
  - Vendor chunk (`app-vendor-eL0o9azT.js`): `69.64 kB` (`20.87 kB` gzip).
  - Terminal chunk (`xterm-DL3g9heD.js`): `336.78 kB` (`85.75 kB` gzip).
  - Markdown chunk (`markdown-hLL2z40O.js`): `769.16 kB` (`235.57 kB` gzip).
  - Mermaid/diagram chunk (`mermaid-DV4MFrF0.js`): `904.85 kB` (`241.68 kB` gzip).
  - Monaco chunk (`monaco-wQYn9ims.js`): `3,830.98 kB` (`992.59 kB` gzip).
  - 50-cycle resource balance: `passed: true`, `rss_mb: 53.62`, `harness_elapsed_ms: 40.45`.
