#!/usr/bin/env node
// One-off, idempotent: drop the vendor "cb-" prefix from bundled agent/command ids and
// directories and set neutral `source` values. Semantic rewriting is done by hand/review.
import { readFileSync, writeFileSync, readdirSync, renameSync, existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "bundled", "preinstall");
const strip = (s) => (typeof s === "string" ? s.replace(/^cb-/, "") : s);
const renames = { agents: {}, commands: {} };

for (const [kind, file] of [["agents", "agent.json"], ["commands", "command.json"]]) {
  const base = path.join(root, kind);
  for (const id of readdirSync(base)) {
    const next = strip(id);
    if (next !== id) {
      if (existsSync(path.join(base, next))) throw new Error(`collision: ${kind}/${next}`);
      renameSync(path.join(base, id), path.join(base, next));
      renames[kind][id] = next;
    }
    const manifestPath = path.join(base, next, file);
    if (!existsSync(manifestPath)) continue;
    const j = JSON.parse(readFileSync(manifestPath, "utf8").replace(/^\uFEFF/, ""));
    j.id = strip(j.id);
    j.name = strip(j.name);
    if (kind === "commands" && !j.slash_id) j.slash_id = j.id;
    if (j.source) j.source = "agentz";
    if (/domain-specific expert agents/i.test(j.description_zh ?? "")) j.description_zh = "";
    writeFileSync(manifestPath, JSON.stringify(j, null, 2) + "\n", "utf8");
  }
}
writeFileSync(
  path.join(root, "..", "codebuddy", "legacy-id-map.json"),
  JSON.stringify(renames, null, 2) + "\n",
  "utf8",
);
console.log(`agents renamed: ${Object.keys(renames.agents).length}, commands renamed: ${Object.keys(renames.commands).length}`);
