import { act, create } from "react-test-renderer";
import { describe, expect, it, vi } from "vitest";
import MarkdownPreview from "./workspaces/codez/MarkdownPreview";
import { ChoiceField } from "./components/chat/interactiveUi/ChoiceField";

const samples = [
  "# title\n\n- [x] task\n- item\n",
  "| a | b |\n| --- | --- |\n| 1 | 2 |\n",
  "```js\nconst x = 1;\n```\n",
  "```c++\nint x;\n```\n",
  "See [file](src/app.ts:12) and <https://example.com>.\n",
  "Paragraph with `inline`.\n\n```\nplain\n```\n",
];

describe("restore render crash", () => {
  it("markdown preview does not throw on common documents", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    for (const content of samples) {
      let thrown: unknown;
      try {
        await act(async () => {
          create(<MarkdownPreview content={content} />);
        });
      } catch (e) {
        thrown = e;
      }
      expect(thrown, content).toBeUndefined();
    }
  });

  it("checkbox field does not crash when the stored value is not a list", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    for (const value of [false, 0, { bad: true }, "a"]) {
      let thrown: unknown;
      try {
        await act(async () => {
          create(
            <ChoiceField
              block={{ type: "checkbox", id: "k", options: [{ value: "a", label: "A" }] }}
              mode="checkbox"
              value={value as unknown as string[]}
              onChange={() => {}}
              disabled={false}
            />,
          );
        });
      } catch (e) {
        thrown = e;
      }
      expect(thrown, String(value)).toBeUndefined();
    }
  });
});
