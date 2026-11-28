#!/usr/bin/env node
/**
 * List vscode.* namespaces / members declared by @types/vscode that are not
 * obviously present in extension-host/src/host/apiFactory.ts.
 *
 * This is a coarse inventory for scheduling — not a full TypeScript API check.
 * Exit 0 always; prints a markdown-ish report to stdout.
 */
import { readFileSync, existsSync } from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const require = createRequire(path.join(root, "package.json"));

function loadTypesVscode() {
  const candidates = [
    path.join(root, "node_modules", "@types", "vscode", "index.d.ts"),
    path.join(root, "extension-host", "node_modules", "@types", "vscode", "index.d.ts"),
  ];
  for (const p of candidates) {
    if (existsSync(p)) return { path: p, text: readFileSync(p, "utf8") };
  }
  try {
    const resolved = require.resolve("@types/vscode/index.d.ts");
    return { path: resolved, text: readFileSync(resolved, "utf8") };
  } catch {
    return null;
  }
}

const types = loadTypesVscode();
const factoryPath = path.join(root, "extension-host", "src", "host", "apiFactory.ts");
const factory = readFileSync(factoryPath, "utf8");

const namespaces = [
  "commands",
  "window",
  "workspace",
  "languages",
  "env",
  "extensions",
  "scm",
  "tasks",
  "debug",
  "tests",
  "notebooks",
  "authentication",
  "comments",
  "chat",
  "lm",
  "l10n",
];

/** @type {Map<string, string[]>} */
const declared = new Map();
if (types) {
  // Match `export namespace window { ... }` blocks at top level (brace-counting).
  for (const ns of namespaces) {
    const re = new RegExp(`export\\s+namespace\\s+${ns}\\s*\\{`);
    const m = re.exec(types.text);
    if (!m) {
      declared.set(ns, []);
      continue;
    }
    let i = m.index + m[0].length;
    let depth = 1;
    const start = i;
    while (i < types.text.length && depth > 0) {
      const ch = types.text[i++];
      if (ch === "{") depth++;
      else if (ch === "}") depth--;
    }
    const body = types.text.slice(start, i - 1);
    const members = new Set();
    for (const mm of body.matchAll(/^\s*(?:export\s+)?(?:function|const|let|var|class|enum|interface|type|namespace)\s+([A-Za-z_][\w]*)/gm)) {
      members.add(mm[1]);
    }
    // Also `export function foo` style already covered; property-like `readonly foo:`
    for (const mm of body.matchAll(/^\s*(?:readonly\s+)?([A-Za-z_][\w]*)\s*[?:]/gm)) {
      if (!["export", "function", "const", "let", "var", "class", "enum", "interface", "type", "namespace", "readonly"].includes(mm[1])) {
        members.add(mm[1]);
      }
    }
    declared.set(ns, [...members].sort());
  }
}

console.log(`# Extension API diff`);
console.log(`- apiFactory: \`${path.relative(root, factoryPath)}\``);
console.log(`- @types/vscode: ${types ? `\`${path.relative(root, types.path)}\`` : "**not installed** — run \`npm i -D @types/vscode\` to enrich"}`);
console.log("");

let missingTotal = 0;
for (const ns of namespaces) {
  const members = declared.get(ns) ?? [];
  // Heuristic presence: member name appears near the namespace in apiFactory.
  const missing = members.filter((name) => {
    if (factory.includes(`${name}:`) || factory.includes(`${name}(`) || factory.includes(`register${name}`) ) {
      // still require the namespace token nearby for register* false positives
      return !factory.includes(name);
    }
    return !new RegExp(`\\b${name}\\b`).test(factory);
  });
  missingTotal += missing.length;
  console.log(`## vscode.${ns}`);
  console.log(`- declared members scanned: ${members.length}`);
  console.log(`- likely missing in apiFactory: ${missing.length}`);
  if (missing.length) {
    console.log(`- ${missing.slice(0, 40).join(", ")}${missing.length > 40 ? `, …(+${missing.length - 40})` : ""}`);
  }
  console.log("");
}
console.log(`Total likely-missing member hits: ${missingTotal}`);
console.log(`(Heuristic only — confirm against real extension activation / soft-compat missing-API log.)`);
