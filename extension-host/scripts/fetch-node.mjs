// Downloads the pinned Node runtime used by agentz-server.
//
//   node scripts/fetch-node.mjs            -> host platform only (bundled sidecar)
//   node scripts/fetch-node.mjs --remote   -> also linux-x64 / linux-arm64 tarballs
//                                             uploaded to SSH / container targets
//
// Output: ../src-tauri/resources/node/
//   <platform>-<arch>/node(.exe)          extracted runtime for the local host
//   node-v<ver>-linux-<arch>.tar.xz       untouched archives for remote deploy
import { mkdirSync, existsSync, writeFileSync, copyFileSync, rmSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { tmpdir } from "node:os";

export const NODE_VERSION = "22.11.0";

const here = dirname(fileURLToPath(import.meta.url));
const outDir = join(here, "..", "..", "src-tauri", "resources", "node");
mkdirSync(outDir, { recursive: true });

async function download(url, dest) {
  if (existsSync(dest)) return;
  console.log(`[fetch-node] ${url}`);
  const res = await fetch(url);
  if (!res.ok) throw new Error(`download failed ${res.status}: ${url}`);
  writeFileSync(dest, Buffer.from(await res.arrayBuffer()));
}

async function fetchLocal() {
  const plat = process.platform === "win32" ? "win" : process.platform;
  const arch = process.arch;
  const target = join(outDir, `${process.platform}-${arch}`);
  const exe = process.platform === "win32" ? "node.exe" : "node";
  if (existsSync(join(target, exe))) return;
  mkdirSync(target, { recursive: true });
  const base = `https://nodejs.org/dist/v${NODE_VERSION}`;
  if (process.platform === "win32") {
    await download(`${base}/win-${arch}/node.exe`, join(target, exe));
    return;
  }
  const name = `node-v${NODE_VERSION}-${plat}-${arch}`;
  const archive = join(tmpdir(), `${name}.tar.gz`);
  await download(`${base}/${name}.tar.gz`, archive);
  const extractDir = join(tmpdir(), `${name}-x`);
  rmSync(extractDir, { recursive: true, force: true });
  mkdirSync(extractDir, { recursive: true });
  execFileSync("tar", ["-xzf", archive, "-C", extractDir]);
  copyFileSync(join(extractDir, name, "bin", "node"), join(target, exe));
}

async function fetchRemote() {
  for (const arch of ["x64", "arm64"]) {
    const name = `node-v${NODE_VERSION}-linux-${arch}.tar.xz`;
    await download(`https://nodejs.org/dist/v${NODE_VERSION}/${name}`, join(outDir, name));
  }
}

await fetchLocal();
if (process.argv.includes("--remote")) await fetchRemote();
console.log(`[fetch-node] done -> ${outDir}`);
