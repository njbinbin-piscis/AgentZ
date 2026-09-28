// Minimal ambient types for Monaco's dependency-free URI module.
// The package ships no declarations for this deep ESM path, but the runtime
// module is plain TypeScript output with no DOM dependencies — safe to use from
// Node (tests) and the browser (app). Keeping the surface we actually use.
declare module "monaco-editor/esm/vs/base/common/uri.js" {
  export interface UriComponents {
    scheme: string;
    authority?: string;
    path?: string;
    query?: string;
    fragment?: string;
  }

  export class URI {
    static parse(value: string, _strict?: boolean): URI;
    static file(path: string): URI;
    static from(components: UriComponents): URI;
    static joinPath(uri: URI, ...pathFragment: string[]): URI;
    readonly scheme: string;
    readonly authority: string;
    readonly path: string;
    readonly query: string;
    readonly fragment: string;
    toString(skipEncoding?: boolean): string;
    toJSON(): UriComponents;
    readonly fsPath: string;
  }
}
