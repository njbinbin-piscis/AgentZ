import { describe, expect, it } from "vitest";
import { customEditorRegistry } from "./customEditorRegistry";
import type { InstalledExtension } from "../services/tauri/vsix";

describe("customEditorRegistry", () => {
  it("matches filenamePattern selectors and prefers default priority", () => {
    customEditorRegistry.ingest([
      {
        id: "pub.pdf",
        name: "pdf",
        publisher: "pub",
        version: "1.0.0",
        display_name: "PDF",
        description: "",
        main: "out/ext.js",
        extension_path: "/ext",
        activation_events: ["onCustomEditor:pdf.preview"],
        enabled: true,
        contributes: {
          customEditors: [
            {
              viewType: "pdf.preview",
              displayName: "PDF Preview",
              priority: "default",
              selector: [{ filenamePattern: "*.pdf" }],
            },
            {
              viewType: "pdf.hex",
              displayName: "PDF Hex",
              priority: "option",
              selector: [{ filenamePattern: "*.pdf" }],
            },
          ],
        },
      } satisfies InstalledExtension,
    ]);

    const m = customEditorRegistry.match("docs/spec.pdf");
    expect(m?.viewType).toBe("pdf.preview");
    expect(customEditorRegistry.match("notes.txt")).toBeUndefined();
  });
});
