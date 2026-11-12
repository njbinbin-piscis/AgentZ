import { act, create, type ReactTestInstance } from "react-test-renderer";
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
        { value: "pat", label: "PAT" },
        { value: "self", label: "Self push" },
        { value: "ssh", label: "Fix SSH" },
      ],
    },
    {
      type: "text_input",
      id: "token",
      label: "Token",
      show_when: { field: "method", equals: "pat" },
    },
  ],
  submit_label: "Go",
};

function radios(root: ReactTestInstance): ReactTestInstance[] {
  return root.findAll(
    (n) => n.type === "input" && (n.props as { type?: string }).type === "radio",
  );
}

describe("InteractiveCard radio selection", () => {
  it("selects a radio option on click and keeps it selected", async () => {
    let renderer!: ReturnType<typeof create>;
    await act(async () => {
      renderer = create(<InteractiveCard requestId="r1" uiDefinition={def} />);
    });

    const inputs = radios(renderer.root);
    expect(inputs.length).toBe(3);
    expect(inputs.every((i) => i.props.checked === false)).toBe(true);

    await act(async () => {
      inputs[1].props.onChange({ target: { checked: true } });
    });

    const after = radios(renderer.root);
    expect(after[0].props.checked).toBe(false);
    expect(after[1].props.checked).toBe(true);
    expect(after[2].props.checked).toBe(false);

    // The conditional field for "pat" must stay hidden; selecting "pat" shows it.
    await act(async () => {
      after[0].props.onChange({ target: { checked: true } });
    });
    const final = radios(renderer.root);
    expect(final[0].props.checked).toBe(true);
    expect(final[1].props.checked).toBe(false);
  });
});
