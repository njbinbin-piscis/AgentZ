import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
const { locale } = vi.hoisted(() => ({ locale: { value: "en" } }));
vi.mock("react-i18next", () => ({ useTranslation: () => ({ i18n: { resolvedLanguage: locale.value } }) }));
import InputContextSummary, { type InputContextSummaryProps } from "./InputContextSummary";
let renderer: ReactTestRenderer | undefined;
const props: InputContextSummaryProps = { authorityProject: "/project", workspace: "/project/worktree", model: "model-a", modelSource: "selected", policy: "Ask", mode: "WorkZ", state: "idle" };
const render = (patch: Partial<InputContextSummaryProps> = {}) => { act(() => { renderer = create(<InputContextSummary {...props} {...patch} />); }); return JSON.stringify(renderer?.toJSON()); };
beforeEach(() => { locale.value = "en"; });
afterEach(() => { act(() => { renderer?.unmount(); }); renderer = undefined; });
it("shows project/workspace/model/state outside collapsed details and keeps isolated workspace distinct", () => {
  render({optionalCount:3}); const primary = renderer!.root.findByProps({ className: "input-context-summary-primary" });
  const text = primary.findAllByType("span").map(span => span.children.join("")).join(" "); expect(text).toContain("/project/worktree"); expect(text).toContain("model-a"); expect(text).toContain("Idle");
  expect(text).toContain("Optional context items: 3");
  expect(renderer!.root.findAllByType("button")).toHaveLength(0);
});
it("missing/free context remains unknown without inventing project or verified environment", () => {
  const out = render({ authorityProject: null, workspace: "/free/cwd", model: null, modelSource: "unknown", policy: null });
  expect(out).toContain("Unknown"); expect(out).toContain("/free/cwd"); expect(out).toContain("Source unknown"); expect(out).toContain("not host verification");
});
it("selected missing model differs from unresolved default and unknown source", () => {
  expect(render({ model: null, modelSource: "selected" })).toContain("Not selected");
  act(() => { renderer?.unmount(); }); renderer = undefined;
  expect(render({ model: null, modelSource: "default" })).toContain("Default not resolved");
});
it("policy is configured rather than granted and queued differs from awaiting approval", () => {
  expect(render({ policy: "Automatic", state: "queued" })).toContain("Configuration does not grant permission");
  expect(JSON.stringify(renderer?.toJSON())).toContain("Queued locally");
  act(() => { renderer?.update(<InputContextSummary {...props} state="approval" />); });
  expect(JSON.stringify(renderer?.toJSON())).toContain("Awaiting approval"); expect(JSON.stringify(renderer?.toJSON())).not.toContain("Queued");
});
it("Chinese labels and invalid optional counts stay explicit", () => {
  locale.value = "zh-CN"; const out = render({ state: "error", optionalCount: -1 });
  expect(out).toContain("项目"); expect(out).toContain("错误"); expect(out).toContain("配置不代表已授予权限"); expect(out).toContain("未知");
});
it("shows only the supplied action name and outcome, not tool payload", () => {
  const out = render({ latestAction: { name: "file_search", status: "done" } });
  const spans = renderer!.root.findByProps({ className: "input-context-summary-primary" }).findAllByType("span");
  expect(spans.map(span => span.children.join("")).join(" ")).toContain("Latest action: file_search");
  expect(out).toContain("Completed");
  expect(out).not.toContain("toolInput");
});
