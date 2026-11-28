/**
 * Declarative language / grammar / icon-theme contributions from installed
 * VS Code extensions (`contributes.languages|grammars|iconThemes`).
 *
 * Monaco registration and TextMate tokenization live in the renderer; the
 * extension host still uses a coarse extension→languageId map for documents.
 */
import { ideApi } from "../services/tauri/ide";
import type { InstalledExtension } from "../services/tauri/vsix";

export interface LanguageContribution {
  id: string;
  aliases?: string[];
  extensions?: string[];
  filenames?: string[];
  filenamePatterns?: string[];
  configuration?: string; // absolute path to language-configuration.json
  extensionId: string;
  extensionPath: string;
}

export interface GrammarContribution {
  language?: string;
  scopeName: string;
  path: string; // absolute path to .tmLanguage / .json
  injectTo?: string[];
  embeddedLanguages?: Record<string, string>;
  extensionId: string;
}

export interface IconThemeContribution {
  id: string;
  label?: string;
  path: string; // absolute path to icon theme JSON
  extensionId: string;
  extensionPath: string;
}

type Listener = () => void;

function asAbs(extensionPath: string, rel?: string): string | undefined {
  if (!rel) return undefined;
  if (/^[a-zA-Z]:[\\/]/.test(rel) || rel.startsWith("/")) return rel;
  const base = extensionPath.replace(/[/\\]+$/, "");
  const sep = base.includes("\\") ? "\\" : "/";
  return `${base}${sep}${rel.replace(/^[\\/]+/, "").replace(/\//g, sep)}`;
}

function parseContributes(ext: InstalledExtension): {
  languages: LanguageContribution[];
  grammars: GrammarContribution[];
  iconThemes: IconThemeContribution[];
} {
  const c = (ext.contributes ?? {}) as Record<string, unknown>;
  const languages: LanguageContribution[] = [];
  const grammars: GrammarContribution[] = [];
  const iconThemes: IconThemeContribution[] = [];

  const langArr = Array.isArray(c.languages) ? c.languages : [];
  for (const raw of langArr) {
    if (!raw || typeof raw !== "object") continue;
    const L = raw as Record<string, unknown>;
    const id = typeof L.id === "string" ? L.id : "";
    if (!id) continue;
    languages.push({
      id,
      aliases: Array.isArray(L.aliases) ? (L.aliases as string[]).filter((x) => typeof x === "string") : undefined,
      extensions: Array.isArray(L.extensions)
        ? (L.extensions as string[]).filter((x) => typeof x === "string")
        : undefined,
      filenames: Array.isArray(L.filenames)
        ? (L.filenames as string[]).filter((x) => typeof x === "string")
        : undefined,
      filenamePatterns: Array.isArray(L.filenamePatterns)
        ? (L.filenamePatterns as string[]).filter((x) => typeof x === "string")
        : undefined,
      configuration: asAbs(ext.extension_path, typeof L.configuration === "string" ? L.configuration : undefined),
      extensionId: ext.id,
      extensionPath: ext.extension_path,
    });
  }

  const gramArr = Array.isArray(c.grammars) ? c.grammars : [];
  for (const raw of gramArr) {
    if (!raw || typeof raw !== "object") continue;
    const G = raw as Record<string, unknown>;
    const scopeName = typeof G.scopeName === "string" ? G.scopeName : "";
    const pathRel = typeof G.path === "string" ? G.path : "";
    const abs = asAbs(ext.extension_path, pathRel);
    if (!scopeName || !abs) continue;
    grammars.push({
      language: typeof G.language === "string" ? G.language : undefined,
      scopeName,
      path: abs,
      injectTo: Array.isArray(G.injectTo) ? (G.injectTo as string[]) : undefined,
      embeddedLanguages:
        G.embeddedLanguages && typeof G.embeddedLanguages === "object"
          ? (G.embeddedLanguages as Record<string, string>)
          : undefined,
      extensionId: ext.id,
    });
  }

  const iconArr = Array.isArray(c.iconThemes) ? c.iconThemes : [];
  for (const raw of iconArr) {
    if (!raw || typeof raw !== "object") continue;
    const I = raw as Record<string, unknown>;
    const id = typeof I.id === "string" ? I.id : "";
    const pathRel = typeof I.path === "string" ? I.path : "";
    const abs = asAbs(ext.extension_path, pathRel);
    if (!id || !abs) continue;
    iconThemes.push({
      id,
      label: typeof I.label === "string" ? I.label : undefined,
      path: abs,
      extensionId: ext.id,
      extensionPath: ext.extension_path,
    });
  }

  return { languages, grammars, iconThemes };
}

class LanguageRegistry {
  private languages = new Map<string, LanguageContribution>();
  private grammars: GrammarContribution[] = [];
  private iconThemes: IconThemeContribution[] = [];
  private extToLang = new Map<string, string>(); // ".toml" → "toml"
  private filenameToLang = new Map<string, string>();
  private listeners = new Set<Listener>();
  private iconFileNames = new Map<string, string>(); // filename → icon path (file:// or convert)
  private iconFileExts = new Map<string, string>();
  private iconDefaultFile: string | undefined;
  private activeIconThemeId: string | null = null;

  subscribe = (l: Listener): (() => void) => {
    this.listeners.add(l);
    return () => this.listeners.delete(l);
  };

  private emit(): void {
    for (const l of this.listeners) l();
  }

  /** Replace contributions from the current enabled extension list. */
  ingest(extensions: InstalledExtension[]): void {
    this.languages.clear();
    this.grammars = [];
    this.iconThemes = [];
    this.extToLang.clear();
    this.filenameToLang.clear();
    for (const ext of extensions) {
      if (!ext.enabled) continue;
      const { languages, grammars, iconThemes } = parseContributes(ext);
      for (const L of languages) {
        this.languages.set(L.id, L);
        for (const e of L.extensions ?? []) {
          const key = e.startsWith(".") ? e.toLowerCase() : `.${e.toLowerCase()}`;
          if (!this.extToLang.has(key)) this.extToLang.set(key, L.id);
        }
        for (const f of L.filenames ?? []) {
          this.filenameToLang.set(f.toLowerCase(), L.id);
        }
      }
      this.grammars.push(...grammars);
      this.iconThemes.push(...iconThemes);
    }
    this.emit();
  }

  languageIdForFile(filePath: string): string | null {
    const norm = filePath.replace(/\\/g, "/");
    const base = norm.split("/").pop()?.toLowerCase() ?? "";
    if (base && this.filenameToLang.has(base)) return this.filenameToLang.get(base)!;
    const dot = base.lastIndexOf(".");
    if (dot >= 0) {
      const ext = base.slice(dot);
      if (this.extToLang.has(ext)) return this.extToLang.get(ext)!;
    }
    return null;
  }

  allLanguages(): LanguageContribution[] {
    return [...this.languages.values()];
  }

  allGrammars(): GrammarContribution[] {
    return [...this.grammars];
  }

  allIconThemes(): IconThemeContribution[] {
    return [...this.iconThemes];
  }

  getGrammarForLanguage(languageId: string): GrammarContribution | undefined {
    return this.grammars.find((g) => g.language === languageId);
  }

  /**
   * Register contributed languages with Monaco (id + aliases + configuration).
   * Safe to call repeatedly; Monaco ignores duplicate register() for same id.
   */
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  async applyToMonaco(monaco: any): Promise<void> {
    for (const L of this.languages.values()) {
      try {
        monaco.languages.register({
          id: L.id,
          aliases: L.aliases,
          extensions: L.extensions,
          filenames: L.filenames,
          filenamePatterns: L.filenamePatterns,
        });
      } catch {
        // already registered
      }
      if (L.configuration) {
        try {
          const raw = await ideApi.readFile(L.configuration);
          const conf = JSON.parse(raw.content) as Record<string, unknown>;
          monaco.languages.setLanguageConfiguration(L.id, toMonacoLanguageConfig(conf));
        } catch {
          // missing/invalid configuration — language id still registered
        }
      }
    }
  }

  async setActiveIconTheme(themeId: string | null): Promise<void> {
    this.activeIconThemeId = themeId;
    this.iconFileNames.clear();
    this.iconFileExts.clear();
    this.iconDefaultFile = undefined;
    if (!themeId) {
      this.emit();
      return;
    }
    const theme = this.iconThemes.find((t) => t.id === themeId);
    if (!theme) {
      this.emit();
      return;
    }
    try {
      const raw = await ideApi.readFile(theme.path);
      const json = JSON.parse(raw.content) as {
        iconDefinitions?: Record<string, { iconPath?: string }>;
        file?: string;
        fileExtensions?: Record<string, string>;
        fileNames?: Record<string, string>;
      };
      const defs = json.iconDefinitions ?? {};
      const resolveIcon = (defId: string | undefined): string | undefined => {
        if (!defId) return undefined;
        const rel = defs[defId]?.iconPath;
        return asAbs(theme.extensionPath, rel);
      };
      this.iconDefaultFile = resolveIcon(json.file);
      for (const [ext, defId] of Object.entries(json.fileExtensions ?? {})) {
        const p = resolveIcon(defId);
        if (p) this.iconFileExts.set(ext.toLowerCase(), p);
      }
      for (const [name, defId] of Object.entries(json.fileNames ?? {})) {
        const p = resolveIcon(defId);
        if (p) this.iconFileNames.set(name.toLowerCase(), p);
      }
    } catch {
      // ignore bad theme
    }
    this.emit();
  }

  getActiveIconThemeId(): string | null {
    return this.activeIconThemeId;
  }

  /** Absolute icon path for a file name, if an icon theme is active. */
  iconPathForFile(fileName: string): string | undefined {
    const base = fileName.replace(/\\/g, "/").split("/").pop()?.toLowerCase() ?? "";
    if (this.iconFileNames.has(base)) return this.iconFileNames.get(base);
    const dot = base.lastIndexOf(".");
    if (dot >= 0) {
      const ext = base.slice(dot + 1);
      if (this.iconFileExts.has(ext)) return this.iconFileExts.get(ext);
    }
    return this.iconDefaultFile;
  }
}

/** Convert VS Code language-configuration.json into Monaco's shape (best-effort). */
export function toMonacoLanguageConfig(conf: Record<string, unknown>): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  if (conf.comments) out.comments = conf.comments;
  if (conf.brackets) out.brackets = conf.brackets;
  if (conf.autoClosingPairs) out.autoClosingPairs = conf.autoClosingPairs;
  if (conf.surroundingPairs) out.surroundingPairs = conf.surroundingPairs;
  if (conf.wordPattern) out.wordPattern = conf.wordPattern;
  if (conf.indentationRules) out.indentationRules = conf.indentationRules;
  if (conf.folding) out.folding = conf.folding;
  if (conf.onEnterRules) out.onEnterRules = conf.onEnterRules;
  return out;
}

export const languageRegistry = new LanguageRegistry();
