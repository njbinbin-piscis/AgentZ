import { describe, expect, it } from "vitest";
import { languageRegistry, toMonacoLanguageConfig } from "./languageRegistry";
import type { InstalledExtension } from "../services/tauri/vsix";

function ext(partial: Partial<InstalledExtension> & { contributes: unknown }): InstalledExtension {
  return {
    id: partial.id ?? "pub.lang",
    name: partial.name ?? "lang",
    publisher: partial.publisher ?? "pub",
    version: partial.version ?? "1.0.0",
    display_name: partial.display_name ?? "Lang",
    description: "",
    main: null,
    extension_path: partial.extension_path ?? "C:\\exts\\lang",
    activation_events: [],
    contributes: partial.contributes,
    enabled: partial.enabled ?? true,
  };
}

describe("languageRegistry", () => {
  it("maps file extensions and filenames from contributes.languages", () => {
    languageRegistry.ingest([
      ext({
        contributes: {
          languages: [
            {
              id: "toml",
              extensions: [".toml"],
              filenames: ["Cargo.toml"],
              aliases: ["TOML"],
            },
          ],
          grammars: [
            {
              language: "toml",
              scopeName: "source.toml",
              path: "./syntaxes/toml.tmLanguage.json",
            },
          ],
          iconThemes: [{ id: "vs-seti", path: "./icons/vs-seti-icon-theme.json", label: "Seti" }],
        },
      }),
    ]);

    expect(languageRegistry.languageIdForFile("src/foo.toml")).toBe("toml");
    expect(languageRegistry.languageIdForFile("Cargo.toml")).toBe("toml");
    expect(languageRegistry.allGrammars()[0]?.scopeName).toBe("source.toml");
    expect(languageRegistry.allGrammars()[0]?.path.replace(/\\/g, "/")).toContain("syntaxes/toml.tmLanguage.json");
    expect(languageRegistry.allIconThemes()).toHaveLength(1);
  });

  it("ignores disabled extensions", () => {
    languageRegistry.ingest([
      ext({
        enabled: false,
        contributes: { languages: [{ id: "zig", extensions: [".zig"] }] },
      }),
    ]);
    expect(languageRegistry.languageIdForFile("main.zig")).toBeNull();
  });

  it("converts language-configuration comments for Monaco", () => {
    const conf = toMonacoLanguageConfig({
      comments: { lineComment: "//", blockComment: ["/*", "*/"] },
      brackets: [["{", "}"]],
    });
    expect(conf.comments).toEqual({ lineComment: "//", blockComment: ["/*", "*/"] });
    expect(conf.brackets).toEqual([["{", "}"]]);
  });
});
