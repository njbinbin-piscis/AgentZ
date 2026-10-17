import assert from "node:assert/strict";
import { renderReport, validateReport } from "./upgrade-report.mjs";

const report = {
  schema_version: 1,
  task_id: "research-task",
  question: "What changed?",
  conclusion: "The local fixture changed.",
  findings: [{ claim: "One source supports the conclusion.", source_ids: ["S1"] }],
  sources: [{ id: "S1", title: "Local fixture", locator: "workspace:reports/fixture.md" }],
  limitations: ["This is a synthetic local example, not an external fact check."],
};

assert.deepEqual(validateReport(report), []);
const markdown = renderReport(report);
assert.match(markdown, /## Question/);
assert.match(markdown, /## Sources/);
assert.match(markdown, /\[S1\]/);
assert.ok(validateReport({ ...report, findings: [{ claim: "Unsupported", source_ids: ["S2"] }] }).some(error => error.includes("known source")));
assert.ok(validateReport({ ...report, sources: [{ ...report.sources[0], locator: "workspace:../secret" }] }).some(error => error.includes("supported locator")));
assert.ok(validateReport({ ...report, limitations: [] }).some(error => error.includes("limitations")));
assert.throws(() => renderReport({ ...report, sources: [] }));
const injected = renderReport({ ...report, question: "[click](https://other.example)", findings: [{ claim: "**unsupported emphasis**", source_ids: ["S1"] }] });
assert.match(injected, /\\\[click\\\]\\\(https:\/\/other\\\.example\\\)/);
assert.match(injected, /\\\*\\\*unsupported emphasis\\\*\\\*/);
console.log("report contract checks passed");
