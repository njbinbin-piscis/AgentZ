// ExtHostWebviews: backs vscode.window.createWebviewPanel. HTML is pushed to the
// renderer which hosts it in a sandboxed iframe; messages flow both ways.
// Also hosts registerCustomEditorProvider / $resolveCustomEditor.

import { IRPCProtocol } from "../common/proxyIdentifier";
import type { UriComponents } from "../common/dto";
import { ExtHostWebviewsShape, MainContext, MainThreadWebviewsShape } from "../common/protocol";
import { randomUUID } from "node:crypto";
import * as fs from "fs";
import { asWebviewResourceUri, normalizeWebviewResourcePath } from "./webviewResourceUri";
import { Disposable, EventEmitter, Uri, ViewColumn } from "./types-impl";

type CustomEditorProvider = {
  openCustomDocument?: (
    uri: Uri,
    openContext: { backupId?: string },
    token: { isCancellationRequested: boolean },
  ) => Promise<unknown> | unknown;
  resolveCustomEditor?: (
    document: unknown,
    webviewPanel: WebviewPanel,
    token: { isCancellationRequested: boolean },
  ) => Promise<void> | void;
  resolveCustomTextEditor?: (
    document: unknown,
    webviewPanel: WebviewPanel,
    token: { isCancellationRequested: boolean },
  ) => Promise<void> | void;
};

class Webview {
  private _html = "";
  readonly onDidReceiveMessageEmitter = new EventEmitter<unknown>();
  readonly onDidReceiveMessage = this.onDidReceiveMessageEmitter.event;
  options: { enableScripts?: boolean; retainContextWhenHidden?: boolean; localResourceRoots?: unknown[] } = {};
  readonly resourceToken = randomUUID();
  resourceRoots: string[] = [];
  constructor(public readonly handle: string, private readonly proxy: MainThreadWebviewsShape) {}
  get html(): string {
    return this._html;
  }
  set html(value: string) {
    this._html = value;
    this.proxy.$setHtml(this.handle, value);
  }
  postMessage(message: unknown): Promise<boolean> {
    return this.proxy.$postMessage(this.handle, message);
  }
  asWebviewUri(uri: { toString(): string; scheme?: string; fsPath?: string }): string {
    return asWebviewResourceUri(uri, this.resourceToken);
  }
  cspSource = process.platform === "win32"
    ? "http://agentz-resource.localhost"
    : "agentz-resource:";
}

class WebviewPanel {
  readonly webview: Webview;
  readonly onDidDisposeEmitter = new EventEmitter<void>();
  readonly onDidDispose = this.onDidDisposeEmitter.event;
  readonly onDidChangeViewStateEmitter = new EventEmitter<unknown>();
  readonly onDidChangeViewState = this.onDidChangeViewStateEmitter.event;
  active = true;
  visible = true;
  private disposed = false;
  constructor(
    public readonly handle: string,
    public readonly viewType: string,
    public title: string,
    public readonly viewColumn: ViewColumn,
    webview: Webview,
    private readonly proxy: MainThreadWebviewsShape,
  ) {
    this.webview = webview;
  }
  reveal(): void {
    this.proxy.$createWebviewPanel({
      handle: this.handle,
      resourceToken: this.webview.resourceToken,
      resourceRoots: this.webview.resourceRoots,
      viewType: this.viewType,
      title: this.title,
      viewColumn: this.viewColumn,
      options: this.webview.options,
    });
  }
  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.proxy.$dispose(this.handle);
    this.onDidDisposeEmitter.fire();
  }
}

export class ExtHostWebviews implements ExtHostWebviewsShape {
  private readonly proxy: MainThreadWebviewsShape;
  private readonly panels = new Map<string, WebviewPanel>();
  private readonly customEditors = new Map<string, { provider: CustomEditorProvider; extensionPath: string }>();
  private seq = 0;

  constructor(rpc: IRPCProtocol) {
    this.proxy = rpc.getProxy(MainContext.MainThreadWebviews);
  }

  createWebviewPanel(
    viewType: string,
    title: string,
    showOptions: number | { viewColumn: number },
    options?: { enableScripts?: boolean; retainContextWhenHidden?: boolean; localResourceRoots?: Array<{ fsPath?: string }> },
    extensionPath = "",
    editorResource?: string,
  ): WebviewPanel {
    const handle = `webview-${this.seq++}`;
    const viewColumn = typeof showOptions === "number" ? showOptions : showOptions.viewColumn;
    const webview = new Webview(handle, this.proxy);
    webview.options = options ?? {};
    webview.resourceRoots = options?.localResourceRoots === undefined
      ? [normalizeWebviewResourcePath(extensionPath)]
      : options.localResourceRoots.flatMap((root) => root?.fsPath ? [normalizeWebviewResourcePath(root.fsPath)] : []);
    const panel = new WebviewPanel(handle, viewType, title, viewColumn as ViewColumn, webview, this.proxy);
    this.panels.set(handle, panel);
    this.proxy.$createWebviewPanel({
      handle,
      resourceToken: webview.resourceToken,
      resourceRoots: webview.resourceRoots,
      viewType,
      title,
      viewColumn,
      options: webview.options,
      editorResource,
    });
    return panel;
  }

  registerCustomEditorProvider(
    viewType: string,
    provider: CustomEditorProvider,
    _options?: unknown,
    extensionPath = "",
  ): Disposable {
    this.customEditors.set(viewType, { provider, extensionPath });
    return new Disposable(() => {
      const cur = this.customEditors.get(viewType);
      if (cur?.provider === provider) this.customEditors.delete(viewType);
    });
  }

  async $resolveCustomEditor(
    viewType: string,
    resource: UriComponents,
    title: string,
  ): Promise<string | null> {
    const entry = this.customEditors.get(viewType);
    if (!entry) return null;
    const uri = Uri.from(resource);
    const panel = this.createWebviewPanel(
      viewType,
      title || uri.fsPath.split(/[/\\]/).pop() || viewType,
      ViewColumn.One,
      { enableScripts: true },
      entry.extensionPath,
      uri.fsPath,
    );
    const token = { isCancellationRequested: false };
    try {
      if (typeof entry.provider.resolveCustomTextEditor === "function") {
        const content =
          uri.scheme === "file" && uri.fsPath && fs.existsSync(uri.fsPath)
            ? fs.readFileSync(uri.fsPath, "utf8")
            : "";
        const document = {
          uri,
          fileName: uri.fsPath,
          getText: () => content,
          languageId: "plaintext",
          version: 1,
          isDirty: false,
          isUntitled: false,
          lineCount: content.split(/\r?\n/).length,
        };
        await entry.provider.resolveCustomTextEditor(document, panel, token);
      } else if (typeof entry.provider.resolveCustomEditor === "function") {
        const document =
          typeof entry.provider.openCustomDocument === "function"
            ? await entry.provider.openCustomDocument(uri, {}, token)
            : { uri };
        await entry.provider.resolveCustomEditor(document, panel, token);
      }
    } catch (err) {
      panel.dispose();
      throw err;
    }
    return panel.handle;
  }

  $onMessage(handle: string, message: unknown): void {
    this.panels.get(handle)?.webview.onDidReceiveMessageEmitter.fire(message);
  }

  $onDidDispose(handle: string): void {
    const panel = this.panels.get(handle);
    if (panel) {
      this.panels.delete(handle);
      panel.onDidDisposeEmitter.fire();
    }
  }

  registerWebviewViewProvider(): Disposable {
    return new Disposable(() => undefined);
  }
}
