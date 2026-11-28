/**
 * End-to-end (renderer-side) coverage for the extension contribution pipeline:
 * soft-compat host version, languages/grammars, custom editors, link routing.
 * Does not download real Open VSX packages (network); uses a synthetic package.json shape.
 */
import { describe, expect, it, beforeEach } from "vitest";
import { languageRegistry } from "./languageRegistry";
import { customEditorRegistry } from "./customEditorRegistry";
import { opensWithSystem } from "../services/linkRouter";
import { satisfies } from "./semver";
import { HOST_VSCODE_VERSION } from "./common/protocol";
import type { InstalledExtension } from "../services/tauri/vsix";

const sampleExt = (overrides: Partial<InstalledExtension> = {}): InstalledExtension => ({
  id: "example.toml-lang",
  name: "toml-lang",
  publisher: "example",
  version: "1.2.3",
  display_name: "TOML Language",
  description: "Languages + grammar + custom editor for .toml",
  main: "out/extension.js",
  extension_path: "C:\\exts\\toml-lang",
  activation_events: ["onLanguage:toml", "onCustomEditor:toml.preview"],
  enabled: true,
  contributes: {
    languages: [
      {
        id: "toml",
        aliases: ["TOML"],
        extensions: [".toml"],
        filenames: ["Cargo.toml"],
        configuration: "./language-configuration.json",
      },
    ],
    grammars: [
      {
        language: "toml",
        scopeName: "source.toml",
        path: "./syntaxes/toml.tmLanguage.json",
      },
    ],
    customEditors: [
      {
        viewType: "toml.preview",
        displayName: "TOML Preview",
        priority: "default",
        selector: [{ filenamePattern: "*.toml" }],
      },
    ],
    iconThemes: [
      {
        id: "toml-icons",
        label: "TOML Icons",
        path: "./icons/theme.json",
      },
    ],
  },
  ...overrides,
});

describe("extension ecosystem e2e (synthetic contributes)", () => {
  beforeEach(() => {
    languageRegistry.ingest([]);
    customEditorRegistry.ingest([]);
  });

  it("hosts a modern engines.vscode range under soft-compat", () => {
    expect(satisfies(HOST_VSCODE_VERSION, "^1.96.0")).toBe(true);
    expect(satisfies(HOST_VSCODE_VERSION, "^1.105.0")).toBe(true);
  });

  it("ingests languages, grammars, icons and custom editors together", () => {
    const ext = sampleExt();
    languageRegistry.ingest([ext]);
    customEditorRegistry.ingest([ext]);

    expect(languageRegistry.languageIdForFile("src/config.toml")).toBe("toml");
    expect(languageRegistry.languageIdForFile("Cargo.toml")).toBe("toml");
    expect(languageRegistry.allGrammars()[0]?.scopeName).toBe("source.toml");
    expect(languageRegistry.allIconThemes()[0]?.id).toBe("toml-icons");

    const editor = customEditorRegistry.match("docs/app.toml");
    expect(editor?.viewType).toBe("toml.preview");
    expect(opensWithSystem("docs/app.toml", { pdfPreview: true })).toBe(false);
  });

  it("ignores disabled extensions across registries", () => {
    const ext = sampleExt({ enabled: false });
    languageRegistry.ingest([ext]);
    customEditorRegistry.ingest([ext]);
    expect(languageRegistry.languageIdForFile("x.toml")).toBeNull();
    expect(customEditorRegistry.match("x.toml")).toBeUndefined();
    expect(opensWithSystem("report.xlsx", { pdfPreview: true })).toBe(true);
  });
});
