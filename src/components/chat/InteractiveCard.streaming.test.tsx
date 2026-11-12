// @vitest-environment jsdom
import { useState } from "react";
import { render, screen, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import InteractiveCard from "./InteractiveCard";
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

const stableDef: UiDefinition = {
  title: "Pick one",
  blocks: [
    {
      type: "radio",
      id: "method",
      label: "Method",
      required: true,
      options: [
        { value: "pat", label: "PAT" },
        { value: "self", label: "Self push" },
      ],
    },
  ],
  submit_label: "Go",
};

/** Simulates the chat view: streaming text updates re-render the parent while
 *  the form card sits below it. */
function StreamingParent({ reparsedDef }: { reparsedDef: boolean }) {
  const [text, setText] = useState("");
  const def = reparsedDef ? JSON.parse(JSON.stringify(stableDef)) : stableDef;
  return (
    <div>
      <div data-testid="stream">{text}</div>
      <button data-testid="tick" onClick={() => setText((t) => t + "x")}>tick</button>
      <InteractiveCard requestId="r1" uiDefinition={def} />
    </div>
  );
}

describe("InteractiveCard under streaming re-renders", () => {
  afterEach(() => cleanup());

  it("keeps the radio selection when the parent re-renders (stable definition)", async () => {
    const user = userEvent.setup();
    render(<StreamingParent reparsedDef={false} />);
    await user.click(screen.getByText("Self push"));
    const radios = screen.getAllByRole("radio") as HTMLInputElement[];
    expect(radios[1].checked).toBe(true);

    await user.click(screen.getByTestId("tick"));
    expect((screen.getAllByRole("radio") as HTMLInputElement[])[1].checked).toBe(true);
  });

  it("keeps the radio selection when the definition is re-parsed each render", async () => {
    const user = userEvent.setup();
    render(<StreamingParent reparsedDef={true} />);
    await user.click(screen.getByText("Self push"));
    const radios = screen.getAllByRole("radio") as HTMLInputElement[];
    expect(radios[1].checked).toBe(true);

    // A streaming chunk arrives: parent re-renders with a fresh (deep-equal) def.
    await user.click(screen.getByTestId("tick"));
    const after = screen.getAllByRole("radio") as HTMLInputElement[];
    expect(after[1].checked, "selection must survive a streaming re-render").toBe(true);
  });
});
