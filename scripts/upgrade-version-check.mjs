#!/usr/bin/env node
// Fail CI when the product version labels disagree. This does not approve a release.
import { readFileSync, existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const read = relative => readFileSync(path.join(root, relative), "utf8");
const cargo = read("src-tauri/Cargo.toml").split(/^\[package\]\s*$/m)[1]?.split(/^\[/m)[0];
const versions = {
  package: JSON.parse(read("package.json")).version,
  packageLock: JSON.parse(read("package-lock.json")).version,
  packageLockRoot: JSON.parse(read("package-lock.json")).packages?.[""]?.version,
  tauri: JSON.parse(read("src-tauri/tauri.conf.json")).version,
  cargo: /^version\s*=\s*"([^"]+)"/m.exec(cargo ?? "")?.[1],
  readme: /当前源码版本\s*\*\*(\d+\.\d+\.\d+)\*\*/u.exec(read("README.md"))?.[1],
  releaseDoc: /Current source version:\s*\*\*(\d+\.\d+\.\d+)\*\*/u.exec(read("RELEASE.md"))?.[1],
};
const expected = versions.package;
const errors = [];
if (typeof expected !== "string" || !/^\d+\.\d+\.\d+$/.test(expected)) errors.push("invalid package version");
for (const [name, value] of Object.entries(versions)) if (value !== expected) errors.push(`${name}: ${value ?? "missing"} != ${expected}`);
const extensionHostVersion = JSON.parse(read("extension-host/package.json")).version;
const extensionHostLock = JSON.parse(read("extension-host/package-lock.json"));
if (typeof extensionHostVersion !== "string" || !/^\d+\.\d+\.\d+$/.test(extensionHostVersion)) errors.push("invalid extension-host version");
if (extensionHostLock.version !== extensionHostVersion || extensionHostLock.packages?.[""]?.version !== extensionHostVersion) {
  errors.push("extension-host package and lockfile versions disagree");
}
for (const filename of [`docs/upgrade/release-notes-${expected}.md`, `docs/upgrade/release-checklist-${expected}.md`]) {
  if (!existsSync(path.join(root, filename))) errors.push(`missing ${filename}`);
}
for (const filename of ["README.md", "RELEASE.md"]) {
  for (const [, target] of read(filename).matchAll(/\[[^\]]+\]\(([^)]+)\)/g)) {
    if (/^(?:[a-z][a-z\d+.-]*:|#)/i.test(target)) continue;
    const relative = target.split("#", 1)[0];
    if (!relative) continue;
    const resolved = path.resolve(root, path.dirname(filename), relative);
    if (!resolved.startsWith(root + path.sep) || !existsSync(resolved)) errors.push(`${filename}: broken local link ${target}`);
  }
}
if (errors.length) {
  console.error(errors.join("\n"));
  process.exitCode = 1;
} else {
  console.log(`AgentZ ${expected}: package, lockfile, Cargo, Tauri, README, RELEASE and candidate docs agree; README/RELEASE local links resolve; extension-host ${extensionHostVersion} package and lockfile agree`);
}
