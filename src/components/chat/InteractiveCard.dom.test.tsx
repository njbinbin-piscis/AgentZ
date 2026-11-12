// @vitest-environment jsdom
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
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

const def: UiDefinition = {
  title: "Pick one",
  blocks: [
    {
      type: "radio",
      id: "method",
      label: "Method",
      required: true,
      options: [
        { value: "pat", label: "PAT", description: "Use a token" },
        { value: "self", label: "Self push", description: "Push yourself" },
        { value: "ssh", label: "Fix SSH", description: "Repair keys" },
      ],
    },
  ],
  submit_label: "Go",
};

describe("InteractiveCard radio (real DOM)", () => {
  it("selects an option when the user clicks its label", async () => {
    const user = userEvent.setup();
    render(<InteractiveCard requestId="r1" uiDefinition={def} />);

    const radios = screen.getAllByRole("radio") as HTMLInputElement[];
    expect(radios.length).toBe(3);
    expect(radios.every((r) => !r.checked)).toBe(true);

    // Click the label text, exactly like a user would.
    await user.click(screen.getByText("Self push"));
    expect(radios[1].checked).toBe(true);
    expect(radios[0].checked).toBe(false);

    // Click the radio input itself.
    await user.click(radios[2]);
    expect(radios[2].checked).toBe(true);
    expect(radios[1].checked).toBe(false);
  });
});
