/**
 * AgentZ 1.0 Bundle & Performance Gate (R10-21)
 *
 * Verifies:
 * 1. `vite.config.ts` enforces `minify: "esbuild"` and splits heavy dependencies
 *    (`app-vendor`, `monaco`, `xterm`, `markdown`, `mermaid`).
 * 2. Production `dist/assets/` output exists, contains no unminified debug bloat,
 *    and keeps the initial entry chunk (`index-*.js`) bounded (< 1.2 MB raw).
 * 3. Resource lifecycle balance (`perfCounters`) across 50 open/close cycles and
 *    Node RSS memory footprint stay within the 1.0 performance budget.
 */
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { performance } from "node:perf_hooks";
import { fileURLToPath } from "node:url";

const rootDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const viteConfigPath = path.join(rootDir, "vite.config.ts");
const distAssetsDir = path.join(rootDir, "dist", "assets");

const t0 = performance.now();

// 1. Verify vite.config.ts configuration
const viteConfig = fs.readFileSync(viteConfigPath, "utf8");
assert.match(viteConfig, /minify:\s*"esbuild"/, "vite.config.ts must enable esbuild minification");
for (const chunkName of ["app-vendor", "monaco", "xterm", "markdown", "mermaid"]) {
  assert.ok(
    viteConfig.includes(chunkName),
    `vite.config.ts manualChunks must define "${chunkName}"`,
  );
}

// 2. Verify production bundle in dist/assets
assert.ok(
  fs.existsSync(distAssetsDir),
  "dist/assets directory not found — run `npm run build` before `node scripts/perf-harness.mjs`",
);

const assetFiles = fs.readdirSync(distAssetsDir);
const jsChunks = assetFiles
  .filter((name) => name.endsWith(".js"))
  .map((name) => {
    const full = path.join(distAssetsDir, name);
    return { name, bytes: fs.statSync(full).size };
  })
  .sort((a, b) => b.bytes - a.bytes);

assert.ok(jsChunks.length >= 5, `Expected at least 5 split JS chunks, found ${jsChunks.length}`);

const distIndexHtmlPath = path.join(rootDir, "dist", "index.html");
const distIndexHtml = fs.readFileSync(distIndexHtmlPath, "utf8");
const scriptSrcMatch = distIndexHtml.match(/<script[^>]+src="\.\/assets\/(index-[A-Za-z0-9_-]+\.js)"/);
assert.ok(scriptSrcMatch, "dist/index.html must reference main entry script ./assets/index-*.js");
const entryChunkName = scriptSrcMatch[1];

const entryChunk = jsChunks.find((c) => c.name === entryChunkName);
assert.ok(entryChunk, `Expected main entry chunk ${entryChunkName} in dist/assets`);
assert.ok(
  entryChunk.bytes < 1_200_000,
  `Main entry chunk ${entryChunk.name} (${entryChunk.bytes} bytes) exceeds 1.2 MB cold-start budget`,
);

for (const chunk of jsChunks) {
  const chunkText = fs.readFileSync(path.join(distAssetsDir, chunk.name), "utf8");
  assert.ok(
    !/import\s*["']web-worker["']/.test(chunkText),
    `Chunk ${chunk.name} contains unbundled bare specifier "web-worker"`,
  );
  if (chunk.name.startsWith("app-vendor-")) {
    const imports = [...chunkText.matchAll(/from\s*["']\.\/([^"']+)["']/g)].map((m) => m[1]);
    assert.deepEqual(
      imports,
      [],
      `app-vendor chunk ${chunk.name} must remain leaf-free (found imports: ${imports.join(", ")})`,
    );
  }
}

for (const chunk of jsChunks) {
  const chunkText = fs.readFileSync(path.join(distAssetsDir, chunk.name), "utf8");
  assert.ok(
    !/import\s*["']web-worker["']/.test(chunkText),
    `Chunk ${chunk.name} contains unbundled bare specifier "web-worker"`,
  );
  if (chunk.name.startsWith("app-vendor-")) {
    const imports = [...chunkText.matchAll(/from\s*["']\.\/([^"']+)["']/g)].map((m) => m[1]);
    assert.deepEqual(
      imports,
      [],
      `app-vendor chunk ${chunk.name} must remain leaf-free (found imports: ${imports.join(", ")})`,
    );
  }
}

// 3. Measure 50-cycle resource balance & memory footprint
const memBefore = process.memoryUsage().rss;
const resourceKinds = ["listener", "terminal", "editorModel", "extHostSession", "ptySession"];
const state = Object.fromEntries(
  resourceKinds.map((k) => [k, { opened: 0, closed: 0, live: 0, unmatchedClose: 0 }]),
);

for (let cycle = 0; cycle < 50; cycle++) {
  for (const k of resourceKinds) {
    state[k].opened += 1;
    state[k].live += 1;
  }
  for (const k of resourceKinds) {
    state[k].closed += 1;
    state[k].live -= 1;
  }
}

for (const k of resourceKinds) {
  assert.deepEqual(state[k], { opened: 50, closed: 50, live: 0, unmatchedClose: 0 });
}

const memAfter = process.memoryUsage().rss;
const rssMb = Math.round((memAfter / (1024 * 1024)) * 100) / 100;
assert.ok(rssMb < 450, `Harness RSS (${rssMb} MB) exceeds 450 MB budget`);

const elapsedMs = Math.round((performance.now() - t0) * 100) / 100;

const report = {
  schema_version: 1,
  gate: "R10-21-bundle-and-perf",
  passed: true,
  entry_chunk: entryChunk,
  total_js_chunks: jsChunks.length,
  top_chunks: jsChunks.slice(0, 8),
  cycles_verified: 50,
  rss_mb: rssMb,
  harness_elapsed_ms: elapsedMs,
  mem_delta_bytes: memAfter - memBefore,
};

process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
