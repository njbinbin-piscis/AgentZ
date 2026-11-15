import { describe, expect, it } from "vitest";
import { normalizeOptions, normalizeUiDefinition } from "./normalize";
import type { UiDefinition } from "./protocol";

describe("normalizeOptions", () => {
  it("leaves well-formed options as they are", () => {
    const options = [
      { value: "pat", label: "PAT", description: "Use a token" },
      { value: "ssh", label: "SSH" },
    ];
    expect(normalizeOptions(options)).toEqual(options);
  });

  it("turns numbers, booleans and alternate keys into string values", () => {
    expect(
      normalizeOptions([
        { value: 1, label: "One" },
        { value: true, label: "Yes" },
        { id: "x", text: "X" },
        { key: "k", title: "K" },
        { name: "only-name" },
      ]),
    ).toEqual([
      { value: "1", label: "One" },
      { value: "true", label: "Yes" },
      { id: "x", text: "X", value: "x", label: "X" },
      { key: "k", title: "K", value: "k", label: "K" },
      { name: "only-name", value: "only-name", label: "only-name" },
    ]);
  });

  it("falls back to the label, and labels a value-only option with its value", () => {
    expect(normalizeOptions([{ label: "Yes" }, { value: "n" }, "Maybe", 7])).toEqual([
      { label: "Yes", value: "Yes" },
      { value: "n", label: "n" },
      { value: "Maybe", label: "Maybe" },
      { value: "7", label: "7" },
    ]);
  });

  it("drops entries with nothing to show and makes repeated values unique", () => {
    expect(
      normalizeOptions([null, {}, { value: "" }, { value: "a", label: "A" }, { value: "a", label: "B" }, "a", "a-2"]).map(
        (o) => o.value,
      ),
    ).toEqual(["a", "a-2", "a-3", "a-2-2"]);
  });

  it("accepts a value-to-label map", () => {
    expect(normalizeOptions({ a: "Option A", b: "Option B" })).toEqual([
      { value: "a", label: "Option A" },
      { value: "b", label: "Option B" },
    ]);
  });

  it("is stable when applied to its own output", () => {
    const once = normalizeOptions([{ label: "Yes" }, 2, { value: "x", label: "X" }, { value: "x", label: "Y" }]);
    expect(normalizeOptions(once)).toEqual(once);
  });
});

describe("normalizeUiDefinition", () => {
  it("repairs nested blocks, wizard steps, defaults and the data model", () => {
    const def = {
      mode: "wizard",
      data: { size: 2, tags: "red" },
      blocks: [
        {
          type: "section",
          blocks: [{ type: "radio", id: "size", options: [{ value: 1, label: "S" }, { value: 2, label: "M" }] }],
        },
      ],
      steps: [
        {
          blocks: [
            { type: "select", id: "mode", default: "Fast", options: [{ value: "f", label: "Fast" }] },
            { type: "checkbox", id: "tags", default: 1, options: [{ value: 1, label: "red" }, "blue"] },
          ],
        },
      ],
    } as unknown as UiDefinition;

    const out = normalizeUiDefinition(def);
    expect(out.blocks[0].blocks![0].options).toEqual([
      { value: "1", label: "S" },
      { value: "2", label: "M" },
    ]);
    // A default written as the label resolves to the option's value.
    expect(out.steps![0].blocks[0].default).toBe("f");
    expect(out.steps![0].blocks[1].default).toEqual(["1"]);
    expect(out.data).toEqual({ size: "2", tags: ["1"] });
    // The input is not mutated.
    expect((def.blocks[0].blocks![0].options as unknown[])[0]).toEqual({ value: 1, label: "S" });
  });

  it("tolerates a definition without usable blocks", () => {
    expect(normalizeUiDefinition(null as unknown as UiDefinition)).toEqual({ blocks: [] });
    expect(normalizeUiDefinition({ title: "t" } as unknown as UiDefinition)).toEqual({ title: "t", blocks: [] });
  });
});
