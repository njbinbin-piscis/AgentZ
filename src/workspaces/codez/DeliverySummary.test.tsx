import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
const { language } = vi.hoisted(() => ({ language: { value: "en" } }));
vi.mock("react-i18next", () => ({ useTranslation: () => ({ i18n: { resolvedLanguage: language.value } }) }));
import DeliverySummary, { type DeliverySummaryProps } from "./DeliverySummary";
import type { ArtifactMetadata } from "../../services/tauri/taskArtifacts";
let renderer: ReactTestRenderer | undefined;
const scope = { project_id: "project", task_id: "task", environment_id: "env", binding_version: "a".repeat(64) };
function props(): DeliverySummaryProps { return { run: { status: "available", version: "run-v1", value: { run_id: "run", task_id: "task", environment_id: "env", attempt_id: "attempt", execution: "succeeded", delivery: "awaiting_review", updated_ms: 1 } }, artifacts: { status: "unknown" } }; }
function artifact(version: number, kind: ArtifactMetadata["kind"]): ArtifactMetadata { return { schema_version: 1, binding: scope, artifact_id: "one-artifact", version, kind, title: "Synthetic", source_relative_path: null, media_type: "text/plain", payload_sha256: "a".repeat(64), bytes: 1, created_ms: 1 }; }
function render(p = props()) { act(() => { renderer = create(<DeliverySummary {...p} />); }); return JSON.stringify(renderer?.toJSON()); }
beforeEach(() => { language.value = "en"; });
afterEach(() => { act(() => { renderer?.unmount(); }); renderer = undefined; });
it("succeeded execution is distinct from unavailable acceptance, application and publication receipts", () => {
  const out = render(); expect(out).toContain("Succeeded"); expect(out.match(/No receipt provided/g)).toHaveLength(3); expect(out).not.toContain("Accepted"); expect(out).toContain("not an atomic snapshot");
});
it("failed and missing execution never infer successful delivery", () => {
  const p = props(); if(p.run.status === "available") p.run.value = { ...p.run.value, execution: "failed" }; expect(render(p)).toContain("Failed");
  act(() => { renderer?.update(<DeliverySummary {...props()} run={{status:"missing"}} />); }); const out = JSON.stringify(renderer?.toJSON()); expect(out).toContain("Not recorded"); expect(out).not.toContain("Succeeded");
});
it("shows persisted Run accepted and rejected decisions without inventing artifact acceptance receipts",()=>{
  const p=props();if(p.run.status==="available")p.run.value={...p.run.value,delivery:"accepted"};
  const out=render(p);expect(out).toContain("Accepted");expect(out).toContain("Run delivery decision");expect(out).toContain("Artifact-version acceptance");expect(out).toContain("No receipt provided");expect(out).toContain("does not establish acceptance of a specific artifact version");
  if(p.run.status==="available")p.run.value={...p.run.value,delivery:"rejected"};act(()=>{renderer?.update(<DeliverySummary {...p}/>);});
  expect(JSON.stringify(renderer?.toJSON())).toContain("Rejected");expect(renderer!.root.findAllByType("button")).toHaveLength(0);
});
it("counts only actual page versions and distinct artifacts; validation kind is not a pass", () => {
  const p = props(); p.artifacts = { status: "available", value: { scope, store_missing: false, page: { versions: [artifact(1,"document"),artifact(2,"validation")], next_cursor: ["one-artifact",2], has_more: true } } };
  const out = render(p); expect(out).toContain("Artifacts shown"); expect(out).toContain("retained versions shown"); expect(out).toContain("more available"); expect(out).toContain("does not establish a passed validation");
  const primary = renderer!.root.findByProps({className:"delivery-summary-primary"}); expect(primary.findByType("span").children.join("")).toContain("1; retained versions shown: 2");
});
it("missing store and absent evidence remain explicit rather than fake zero successful artifacts", () => {
  const p=props();p.artifacts={status:"available",value:{scope,store_missing:true,page:{versions:[],next_cursor:null,has_more:false}}};const out=render(p);expect(out).toContain("Not recorded");expect(out).toContain("Unknown");expect(out).not.toContain("Artifacts shown");
});
it("view callbacks are the only buttons and Chinese receipts stay unprovided", () => {
  language.value="zh-CN";const review=vi.fn(),open=vi.fn();const out=render({...props(),onReviewChanges:review,onOpenArtifacts:open});expect(out).toContain("未提供回执");
  const buttons=renderer!.root.findAllByType("button");expect(buttons).toHaveLength(2);act(()=>{buttons[0].props.onClick();buttons[1].props.onClick();});expect(review).toHaveBeenCalledOnce();expect(open).toHaveBeenCalledOnce();expect(out).not.toContain("接受交付");
});
