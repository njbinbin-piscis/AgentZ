// @vitest-environment jsdom
import { render, screen, cleanup, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import InteractiveCard from "./InteractiveCard";
import ToolTrace, { type ToolTraceItem } from "../ToolTrace";
import { InteractiveCardsContext } from "./InteractiveCardsContext";
import { respondInteractiveUi } from "../../services/tauri/interactive";
import type { UiDefinition } from "./interactiveUi/protocol";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string, opts?: { defaultValue?: string }) => opts?.defaultValue ?? key }),
}));

vi.mock("../../services/tauri/interactive", () => ({
  respondInteractiveUi: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("../../workspaces/codez/Markdown", () => ({
  default: ({ content }: { content: string }) => <div>{content}</div>,
}));

vi.mock("./InteractiveCard.css", () => ({}));

const respond = vi.mocked(respondInteractiveUi);

beforeEach(() => respond.mockClear());
afterEach(() => cleanup());

/** A required single-choice question whose options are written the way agents get them wrong. */
function requiredRadio(options: unknown): UiDefinition {
  return {
    title: "Pick one",
    blocks: [{ type: "radio", id: "choice", label: "Choice", required: true, options }],
    submit_label: "Go",
  } as unknown as UiDefinition;
}

async function pickSecondAndSubmit(def: UiDefinition) {
  const user = userEvent.setup();
  render(<InteractiveCard requestId="r1" uiDefinition={def} />);
  const radios = screen.getAllByRole("radio") as HTMLInputElement[];
  await user.click(radios[1]);
  expect(radios[1].checked, "the clicked option must become selected").toBe(true);
  expect(
    radios.filter((r) => r.checked).length,
    "exactly one option of a single-choice question is selected",
  ).toBe(1);
  await user.click(screen.getByRole("button", { name: "Go" }));
  expect(screen.queryByText("Required"), "a selected required field must validate").toBeNull();
  expect(respond).toHaveBeenCalledTimes(1);
  return respond.mock.calls[0][1] as Record<string, unknown>;
}

describe("InteractiveCard with malformed option lists", () => {
  it("numeric option values can be selected and submitted", async () => {
    const payload = await pickSecondAndSubmit(
      requiredRadio([
        { value: 1, label: "One" },
        { value: 2, label: "Two" },
      ]),
    );
    expect(payload.choice).toBe("2");
  });

  it("options with only a label submit the label", async () => {
    const payload = await pickSecondAndSubmit(requiredRadio([{ label: "Yes" }, { label: "No" }]));
    expect(payload.choice).toBe("No");
  });

  it("options keyed by id instead of value submit the id", async () => {
    const payload = await pickSecondAndSubmit(
      requiredRadio([
        { id: "script", label: "Use the script" },
        { id: "install", label: "Install it" },
      ]),
    );
    expect(payload.choice).toBe("install");
  });

  it("bare string options submit the string", async () => {
    const payload = await pickSecondAndSubmit(requiredRadio(["Apple", "Banana"]));
    expect(payload.choice).toBe("Banana");
  });

  it("options sharing a value stay individually selectable", async () => {
    const payload = await pickSecondAndSubmit(
      requiredRadio([
        { value: "a", label: "First" },
        { value: "a", label: "Second" },
      ]),
    );
    expect(payload.choice).toBe("a-2");
  });

  it("a numeric default preselects its option", () => {
    const def = requiredRadio([
      { value: 1, label: "One" },
      { value: 2, label: "Two" },
    ]);
    (def.blocks[0] as { default?: unknown }).default = 2;
    render(<InteractiveCard requestId="r1" uiDefinition={def} />);
    const radios = screen.getAllByRole("radio") as HTMLInputElement[];
    expect(radios.map((r) => r.checked)).toEqual([false, true]);
  });

  it("checkbox options with numeric values toggle independently", async () => {
    const user = userEvent.setup();
    const def = {
      blocks: [
        {
          type: "checkbox",
          id: "picks",
          required: true,
          options: [
            { value: 1, label: "One" },
            { value: 2, label: "Two" },
            { value: 3, label: "Three" },
          ],
        },
      ],
      submit_label: "Go",
    } as unknown as UiDefinition;
    render(<InteractiveCard requestId="r1" uiDefinition={def} />);
    const boxes = screen.getAllByRole("checkbox") as HTMLInputElement[];
    await user.click(boxes[0]);
    await user.click(boxes[2]);
    expect(boxes.map((b) => b.checked)).toEqual([true, false, true]);
    await user.click(screen.getByRole("button", { name: "Go" }));
    expect((respond.mock.calls[0][1] as Record<string, unknown>).picks).toEqual(["1", "3"]);
  });

  it("works through the message-flow path (chat_ui tool step with a live card)", async () => {
    const user = userEvent.setup();
    const uiDefinition = requiredRadio([{ label: "Yes" }, { label: "No" }]);
    const item: ToolTraceItem = {
      id: "call-1",
      name: "chat_ui",
      status: "running",
      input: { ui_definition: uiDefinition },
    };
    const onSubmitted = vi.fn();
    const view = render(
      <InteractiveCardsContext.Provider
        value={{
          cards: { "call-1": { requestId: "call-1", uiDefinition } },
          onSubmitted,
          onActionSent: () => {},
        }}
      >
        <ToolTrace items={[item]} />
      </InteractiveCardsContext.Provider>,
    );
    const card = view.container.querySelector<HTMLElement>(".agentz-chat-ui-inline")!;
    expect(card, "the live form must render inline").toBeTruthy();
    const radios = within(card).getAllByRole("radio") as HTMLInputElement[];
    await user.click(within(card).getByText("No"));
    expect(radios[1].checked).toBe(true);
    await user.click(within(card).getByRole("button", { name: "Go" }));
    expect(respond).toHaveBeenCalledWith("call-1", expect.objectContaining({ choice: "No" }));
    expect(onSubmitted).toHaveBeenCalledWith("call-1", expect.objectContaining({ choice: "No" }));
  });
});

describe("InteractiveCard wizard", () => {
  const wizard: UiDefinition = {
    mode: "wizard",
    submit_label: "Finish",
    blocks: [],
    steps: [
      {
        id: "s1",
        label: "Language",
        blocks: [
          {
            type: "radio",
            id: "lang",
            required: true,
            options: [
              { value: "rust", label: "Rust" },
              { value: "go", label: "Go" },
            ],
          },
        ],
      },
      {
        id: "s2",
        label: "Name",
        blocks: [{ type: "text_input", id: "name", label: "Project name", required: true }],
      },
    ],
  };

  it("keeps earlier answers when moving forward and back, and submits every step", async () => {
    const user = userEvent.setup();
    render(<InteractiveCard requestId="w1" uiDefinition={wizard} />);

    await user.click(screen.getByText("Go"));
    await user.click(screen.getByRole("button", { name: "Next" }));
    await user.type(screen.getByRole("textbox"), "demo");

    await user.click(screen.getByRole("button", { name: "Back" }));
    const radios = screen.getAllByRole("radio") as HTMLInputElement[];
    expect(radios.map((r) => r.checked), "step 1 answer must survive the round trip").toEqual([false, true]);

    await user.click(screen.getByRole("button", { name: "Next" }));
    expect((screen.getByRole("textbox") as HTMLInputElement).value, "step 2 answer must survive too").toBe("demo");

    await user.click(screen.getByRole("button", { name: "Finish" }));
    expect(respond).toHaveBeenCalledTimes(1);
    expect(respond.mock.calls[0][1]).toEqual(expect.objectContaining({ lang: "go", name: "demo" }));
  });

  it("does not let the user advance past an unanswered required step", async () => {
    const user = userEvent.setup();
    render(<InteractiveCard requestId="w2" uiDefinition={wizard} />);
    await user.click(screen.getByRole("button", { name: "Next" }));
    expect(screen.getAllByRole("radio").length, "still on step 1").toBe(2);
    expect(screen.getByText("Required")).toBeTruthy();
  });
});
