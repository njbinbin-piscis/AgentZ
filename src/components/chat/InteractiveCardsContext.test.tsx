import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { describe, expect, it } from "vitest";
import ToolTrace, { interleaveTools, type ToolTraceItem } from "../ToolTrace";
import { parseChatUiDefinition, parseInteractiveResponse } from "./InteractiveCardsContext";

const ui = {
  title: "Pick one",
  blocks: [{ type: "text_input", id: "name", label: "Name" }],
};
const resultText = `USER_INTERACTIVE_RESPONSE_JSON:\n${JSON.stringify({ name: "Ada", __action_type__: "submit" }, null, 2)}\n\nThis is the user's final structured input.`;

describe("chat_ui history helpers", () => {
  it("recovers submitted values from a persisted tool result", () => {
    expect(parseInteractiveResponse(resultText)).toMatchObject({ name: "Ada" });
    expect(parseInteractiveResponse("Interactive UI timed out")).toBeNull();
    expect(parseInteractiveResponse(undefined)).toBeNull();
  });

  it("reads ui_definition from object or JSON-text input", () => {
    expect(parseChatUiDefinition({ ui_definition: ui })).toMatchObject({ title: "Pick one" });
    expect(parseChatUiDefinition(JSON.stringify({ ui_definition: ui }))).toMatchObject({ title: "Pick one" });
    expect(parseChatUiDefinition({ ui_definition: {} })).toBeNull();
    expect(parseChatUiDefinition("not json")).toBeNull();
  });
});

describe("chat_ui tool step in the message flow", () => {
  const render = async (item: ToolTraceItem) => {
    let root!: ReactTestRenderer;
    await act(async () => {
      root = create(<ToolTrace items={[item]} />);
    });
    return JSON.stringify(root.toJSON());
  };

  it("restored history shows a grayed read-only card, with no active buttons", async () => {
    const out = await render({
      id: "c1",
      name: "chat_ui",
      status: "done",
      input: { ui_definition: ui },
      result: resultText,
    });
    expect(out).toContain("ic-readonly");
    expect(out).toContain("Pick one");
    expect(out).toContain("Ada");
  });

  it("keeps a timed-out form where it was asked, with later output underneath", async () => {
    const segments = interleaveTools("Please choose\n\nContinuing with defaults", [
      { id: "later", name: "shell", status: "done", input: { command: "echo later" }, textOffset: 200 },
      { id: "c2", name: "chat_ui", status: "error", input: { ui_definition: ui }, result: "timed out", textOffset: 13 },
    ]);
    const formAt = segments.findIndex((segment) => segment.tools.some((tool) => tool.id === "c2"));
    const laterAt = segments.findIndex((segment) => segment.tools.some((tool) => tool.id === "later"));
    expect(formAt).toBeGreaterThanOrEqual(0);
    expect(laterAt).toBeGreaterThan(formAt);
    expect(segments.slice(formAt + 1).some((segment) => segment.text.includes("Continuing"))).toBe(true);

    let root!: ReactTestRenderer;
    await act(async () => {
      root = create(
        <ToolTrace
          items={[
            { id: "before", name: "shell", status: "done", input: { command: "echo before" } },
            { id: "c2", name: "chat_ui", status: "error", input: { ui_definition: ui }, result: "Interactive UI timed out" },
            { id: "after", name: "shell", status: "done", input: { command: "echo after" } },
          ]}
        />,
      );
    });
    const out = JSON.stringify(root.toJSON());
    expect(out.indexOf("echo before")).toBeLessThan(out.indexOf("ic-readonly"));
    expect(out.indexOf("ic-readonly")).toBeLessThan(out.indexOf("echo after"));
  });

  it("a timed-out call is read-only and says nobody answered", async () => {
    const out = await render({
      id: "c2",
      name: "chat_ui",
      status: "error",
      input: { ui_definition: ui },
      result: "Interactive UI timed out after 5 minutes with no user response.",
    });
    expect(out).toContain("ic-readonly");
    expect(out).toContain("interactiveNoAnswer");
  });
});
