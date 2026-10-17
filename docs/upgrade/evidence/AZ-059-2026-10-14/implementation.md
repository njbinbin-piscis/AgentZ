# AZ-059 local loading candidate (2026-10-14)

`src/App.tsx` now loads CodeMapPanel only when the user opens the code map. Its ReactFlow and ELK graph code is separated from the initial App module by a `React.lazy` boundary. The build uses relative URLs for offline Tauri assets.

Local checks on this working tree: `npm run typecheck` passed; `npx vite build` with compression disabled passed in 4m33s; targeted ESLint passed with one pre-existing `useEffect` dependency warning at App.tsx:317; `git diff --check` passed. In that output the HTML entry `index-BoSBIgq_.js` was 2,822,923 bytes and the deferred graph `index-CcA6MEBn.js` was 3,122,049 bytes.

The exact current source then passed an explicit `npx vite build --minify esbuild --outDir target/az059-minify`. The default Vite config was restored to `minify: "esbuild"` and `npx vite build` passed again in 3m32s; its output is saved in [minified-build.log](minified-build.log). The minified HTML entry `index-Cn4e2Tmh.js` is 1,625,323 bytes, 42.4% smaller than the same-source nonminified entry. The lazy graph chunk is 1,457,074 bytes; `dist/index.html` does not preload it. Monaco (3,831,379 bytes) and Markdown are still preloaded and large. These are build-output comparisons, not a controlled before/after cold-start measurement. The earlier import-analysis failure is no longer reproduced with the new chunk graph; its precise root cause is unproven.

This change does not measure native desktop startup or satisfy AZ-059 acceptance. Keep AZ-059 blocked pending a controlled baseline comparison and offline/native functional checks; AZ-040 is also still preparing.

2026-10-14 current-tree confirmation: the production build passed again with esbuild minification after later UI changes (`agentz-reminder-build-final.log`, local temporary log). Its generated `dist/index.html` still preloads Monaco and Markdown but not the deferred graph chunk. The old parse error is no longer a current build blocker, so the ledger blocker was corrected to the remaining measurement, native-offline, and AZ-040 acceptance gaps. No cold-start figure is inferred from the build time.
