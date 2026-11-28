/**
 * TextMate → Monaco tokens provider using vscode-textmate + vscode-oniguruma.
 * Grammars come from {@link languageRegistry}.
 */
import type { GrammarContribution } from "./languageRegistry";
import { languageRegistry } from "./languageRegistry";
import { ideApi } from "../services/tauri/ide";

type TextMateMod = typeof import("vscode-textmate");
type OnigurumaMod = typeof import("vscode-oniguruma");

let textmate: TextMateMod | null = null;
let oniguruma: OnigurumaMod | null = null;
let onigReady = false;
let registry: import("vscode-textmate").Registry | null = null;
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const monacoDisposables: { dispose: () => void }[] = [];

async function ensureOnig(): Promise<void> {
  if (onigReady && textmate && oniguruma) return;
  textmate = await import("vscode-textmate");
  oniguruma = await import("vscode-oniguruma");
  const wasmUrl = new URL("../../node_modules/vscode-oniguruma/release/onig.wasm", import.meta.url);
  const wasmRes = await fetch(wasmUrl);
  await oniguruma.loadWASM(await wasmRes.arrayBuffer());
  onigReady = true;
}

function buildRegistry(): import("vscode-textmate").Registry {
  if (!textmate || !oniguruma) throw new Error("textmate engines not loaded");
  const grammarByScope = new Map<string, GrammarContribution>();
  for (const g of languageRegistry.allGrammars()) {
    grammarByScope.set(g.scopeName, g);
  }
  return new textmate.Registry({
    onigLib: Promise.resolve({
      createOnigScanner: (sources: string[]) => oniguruma!.createOnigScanner(sources),
      createOnigString: (s: string) => oniguruma!.createOnigString(s),
    }),
    loadGrammar: async (scopeName: string) => {
      const g = grammarByScope.get(scopeName);
      if (!g || !textmate) return null;
      try {
        const raw = await ideApi.readFile(g.path);
        return textmate.parseRawGrammar(raw.content, g.path);
      } catch {
        return null;
      }
    },
    getInjections: (scopeName: string) => {
      const inject: string[] = [];
      for (const g of languageRegistry.allGrammars()) {
        if (g.injectTo?.includes(scopeName)) inject.push(g.scopeName);
      }
      return inject;
    },
  });
}

/**
 * Register TokensProvider for every contributed grammar that names a language.
 * Returns a dispose function.
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export async function applyTextMateToMonaco(monaco: any): Promise<() => void> {
  await ensureOnig();
  registry = buildRegistry();
  if (!registry || !textmate) return () => undefined;

  for (const d of monacoDisposables) d.dispose();
  monacoDisposables.length = 0;

  const byLang = new Map<string, GrammarContribution>();
  for (const g of languageRegistry.allGrammars()) {
    if (g.language && !byLang.has(g.language)) byLang.set(g.language, g);
  }

  for (const [languageId, contrib] of byLang) {
    try {
      const grammar = await registry.loadGrammar(contrib.scopeName);
      if (!grammar) continue;
      const INITIAL = textmate.INITIAL;
      const disposable = monaco.languages.setTokensProvider(languageId, {
        getInitialState: () => INITIAL,
        tokenize: (line: string, state: import("vscode-textmate").StateStack) => {
          const result = grammar.tokenizeLine(line, state);
          return {
            tokens: result.tokens.map((t) => ({
              startIndex: t.startIndex,
              scopes: t.scopes.join(" "),
            })),
            endState: result.ruleStack,
          };
        },
      });
      monacoDisposables.push(disposable);
    } catch {
      // skip broken grammars
    }
  }

  return () => {
    for (const d of monacoDisposables) d.dispose();
    monacoDisposables.length = 0;
  };
}
