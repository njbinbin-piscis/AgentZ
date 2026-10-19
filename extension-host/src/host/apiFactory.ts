// Builds the `vscode` module object handed to each extension's require('vscode').
// Mirrors VS Code's createApiFactoryAndRegisterActors: namespaces backed by the
// ExtHost* services, plus the concrete types and enums.

import * as fs from "fs";
import * as path from "path";
import { Services } from "./services";
import * as types from "./types-impl";
import { ExtensionDescriptionDto } from "../common/dto";
import { HOST_VSCODE_VERSION } from "../common/protocol";

export interface ExtensionContext {
  subscriptions: { dispose(): unknown }[];
  extensionPath: string;
  extensionUri: types.Uri;
  globalState: Memento;
  workspaceState: Memento;
  asAbsolutePath(relative: string): string;
  storageUri?: types.Uri;
  globalStorageUri: types.Uri;
  logUri: types.Uri;
  extensionMode: number;
  secrets: { get(k: string): Promise<string | undefined>; store(k: string, v: string): Promise<void>; delete(k: string): Promise<void> };
}

export class Memento {
  private data: Record<string, unknown> = {};

  constructor(private readonly storageFilePath?: string) {
    if (this.storageFilePath) {
      try {
        if (fs.existsSync(this.storageFilePath)) {
          const raw = fs.readFileSync(this.storageFilePath, "utf8");
          const parsed = JSON.parse(raw);
          if (parsed && typeof parsed === "object") {
            this.data = parsed as Record<string, unknown>;
          }
        }
      } catch {
        // Ignore corrupt/unreadable state file
      }
    }
  }

  get<T>(key: string, defaultValue?: T): T | undefined {
    return key in this.data ? (this.data[key] as T) : defaultValue;
  }
  update(key: string, value: unknown): Promise<void> {
    if (value === undefined) delete this.data[key];
    else this.data[key] = value;
    if (this.storageFilePath) {
      try {
        fs.mkdirSync(path.dirname(this.storageFilePath), { recursive: true });
        fs.writeFileSync(this.storageFilePath, JSON.stringify(this.data, null, 2), "utf8");
      } catch {
        // Best-effort persistence
      }
    }
    return Promise.resolve();
  }
  keys(): string[] {
    return Object.keys(this.data);
  }
  setKeysForSync(): void {
    /* no-op */
  }
}

export function createApiFactory(services: Services) {
  const s = services;

  const commands = {
    registerCommand: (id: string, cb: (...a: unknown[]) => unknown, thisArg?: unknown) => s.commands.registerCommand(id, cb, thisArg),
    registerTextEditorCommand: (id: string, cb: (...a: unknown[]) => unknown, thisArg?: unknown) => s.commands.registerCommand(id, cb, thisArg),
    executeCommand: <T>(id: string, ...args: unknown[]) => s.commands.executeCommand<T>(id, ...args),
    getCommands: () => s.commands.getCommands(),
  };

  const wrapEditor = (document: unknown) => ({
    document,
    selection: new types.Selection(0, 0, 0, 0),
    selections: [new types.Selection(0, 0, 0, 0)],
    options: { tabSize: 2, insertSpaces: true },
    viewColumn: types.ViewColumn.One,
    edit: () => Promise.resolve(true),
    insertSnippet: () => Promise.resolve(true),
    setDecorations: () => undefined,
    revealRange: () => undefined,
    show: () => undefined,
    hide: () => undefined,
  });

  const window = {
    showInformationMessage: (m: string, ...r: unknown[]) => s.window.showInformationMessage(m, ...r),
    showWarningMessage: (m: string, ...r: unknown[]) => s.window.showWarningMessage(m, ...r),
    showErrorMessage: (m: string, ...r: unknown[]) => s.window.showErrorMessage(m, ...r),
    createStatusBarItem: (
      a?: string | types.StatusBarAlignment,
      b?: types.StatusBarAlignment | number,
      c?: number,
    ) => s.window.createStatusBarItem(a, b, c),
    createOutputChannel: (name: string, options?: string | { log?: boolean }) =>
      s.window.createOutputChannel(name, options),
    showQuickPick: (items: unknown, options?: { placeHolder?: string; title?: string; canPickMany?: boolean }) =>
      s.window.showQuickPick(items, options),
    showInputBox: (options?: {
      prompt?: string;
      title?: string;
      value?: string;
      placeHolder?: string;
      password?: boolean;
    }) => s.window.showInputBox(options),
    createQuickPick: () => s.window.createQuickPick(),
    createInputBox: () => s.window.createInputBox(),
    createTerminal: (o?: string | { name?: string; cwd?: string | { fsPath?: string } }) =>
      s.window.createTerminal(o),
    get terminals() {
      return s.window.terminals;
    },
    get activeTerminal() {
      return s.window.activeTerminal;
    },
    onDidOpenTerminal: s.window.onDidOpenTerminalEmitter.event,
    onDidCloseTerminal: s.window.onDidCloseTerminalEmitter.event,
    onDidChangeActiveTerminal: s.window.onDidChangeActiveTerminalEmitter.event,
    onDidChangeTerminalState: s.window.onDidChangeTerminalStateEmitter.event,
    setStatusBarMessage: (t: string, hideAfter?: number | Thenable<unknown>) =>
      s.window.setStatusBarMessage(t, hideAfter),
    withProgress: <R>(
      o: unknown,
      t: (p: { report: (v: unknown) => void }, token: unknown) => Thenable<R>,
    ) => s.window.withProgress(o, t),
    registerTreeDataProvider: (viewId: string, provider: any) =>
      s.treeViews.registerTreeDataProvider(viewId, provider),
    createTreeView: (viewId: string, options: any) => s.treeViews.createTreeView(viewId, options),
    createWebviewPanel: (viewType: string, title: string, showOptions: any, options?: any) =>
      s.webviews.createWebviewPanel(viewType, title, showOptions, options),
    registerWebviewViewProvider: () => s.webviews.registerWebviewViewProvider(),
    createTextEditorDecorationType: () => new types.Disposable(() => undefined),
    get activeTextEditor() {
      const doc = s.documents.getActiveDocument();
      return doc ? wrapEditor(doc) : undefined;
    },
    get visibleTextEditors() {
      return s.documents.all().map((document) => wrapEditor(document));
    },
    onDidChangeActiveTextEditor: (l: (e: unknown) => unknown) =>
      s.documents.onDidChangeActiveEmitter.event((doc) => l(doc ? wrapEditor(doc) : undefined)),
    onDidChangeVisibleTextEditors: new types.EventEmitter<unknown[]>().event,
    onDidChangeTextEditorSelection: new types.EventEmitter<unknown>().event,
    onDidChangeTextEditorVisibleRanges: new types.EventEmitter<unknown>().event,
    onDidChangeWindowState: new types.EventEmitter<{ focused: boolean }>().event,
    state: { focused: true },
    showTextDocument: async (uriOrDoc: unknown) => {
      if (!uriOrDoc) return undefined;
      const targetUri =
        typeof uriOrDoc === "object" && uriOrDoc !== null && "uri" in uriOrDoc
          ? (uriOrDoc as { uri: types.Uri }).uri
          : (uriOrDoc as types.Uri);
      if (targetUri) {
        await s.commands.executeCommand("vscode.open", targetUri.toString());
      }
      return undefined;
    },
  };

  const workspace = {
    get workspaceFolders() {
      return s.workspace.workspaceFolders;
    },
    get rootPath() {
      return s.workspace.rootPath;
    },
    get name() {
      return s.workspace.workspaceFolders[0]?.name;
    },
    get textDocuments() {
      return s.documents.all();
    },
    get isTrusted() {
      return true;
    },
    onDidGrantWorkspaceTrust: new types.EventEmitter<void>().event,
    getConfiguration: (section?: string) => s.workspace.getConfiguration(section),
    getWorkspaceFolder: (uri: types.Uri) => s.workspace.getWorkspaceFolder(uri),
    asRelativePath: (pathOrUri: string | types.Uri) => {
      const p = typeof pathOrUri === "string" ? pathOrUri : pathOrUri.fsPath;
      const root = s.workspace.rootPath;
      if (root && p.startsWith(root)) {
        return p.slice(root.length).replace(/^[/\\]+/, "").replace(/\\/g, "/");
      }
      return p;
    },
    findFiles: (
      include: string | types.RelativePattern,
      exclude?: string | types.RelativePattern | null,
      maxResults?: number,
    ) => s.workspace.findFiles(include, exclude, maxResults),
    openTextDocument: async (uriOrPath?: types.Uri | string | { language?: string; content?: string }) => {
      if (!uriOrPath || (typeof uriOrPath === "object" && !("scheme" in uriOrPath))) {
        const opts = (uriOrPath ?? {}) as { language?: string; content?: string };
        const untitledUri = types.Uri.parse(`untitled:Untitled-${Date.now()}`);
        s.documents.$acceptModelOpened({
          uri: untitledUri.toJSON(),
          languageId: opts.language ?? "plaintext",
          versionId: 1,
          lines: (opts.content ?? "").split(/\r?\n/),
          eol: "\n",
        });
        return s.documents.getDocument(untitledUri.toJSON());
      }
      const uri = typeof uriOrPath === "string" ? types.Uri.file(uriOrPath) : uriOrPath;
      const existing = s.documents.getDocument(uri.toJSON());
      if (existing) return existing;
      try {
        if (uri.scheme === "file" && uri.fsPath && fs.existsSync(uri.fsPath)) {
          const content = await fs.promises.readFile(uri.fsPath, "utf8");
          const ext = path.extname(uri.fsPath).toLowerCase();
          const langMap: Record<string, string> = {
            ".ts": "typescript",
            ".tsx": "typescriptreact",
            ".js": "javascript",
            ".jsx": "javascriptreact",
            ".json": "json",
            ".md": "markdown",
            ".rs": "rust",
            ".py": "python",
          };
          s.documents.$acceptModelOpened({
            uri: uri.toJSON(),
            languageId: langMap[ext] ?? "plaintext",
            versionId: 1,
            lines: content.split(/\r?\n/),
            eol: "\n",
          });
          return s.documents.getDocument(uri.toJSON());
        }
      } catch {
        // ignore
      }
      return existing;
    },
    createFileSystemWatcher: (
      globPattern: string | types.RelativePattern,
      ignoreCreateEvents?: boolean,
      ignoreChangeEvents?: boolean,
      ignoreDeleteEvents?: boolean,
    ) =>
      s.workspace.registerFileSystemWatcher(
        globPattern,
        ignoreCreateEvents,
        ignoreChangeEvents,
        ignoreDeleteEvents,
      ),
    registerTaskProvider: (type: string, provider: any) => s.tasks.registerTaskProvider(type, provider),
    registerNotebookSerializer: (viewType: string, serializer: any) =>
      s.notebook.registerNotebookSerializer(viewType, serializer),
    registerTextDocumentContentProvider: () => new types.Disposable(() => undefined),
    applyEdit: () => s.workspace.applyEdit(),
    saveAll: () => Promise.resolve(true),
    onDidChangeConfiguration: s.workspace.onDidChangeConfigurationEmitter.event,
    onDidChangeWorkspaceFolders: s.workspace.onDidChangeWorkspaceFoldersEmitter.event,
    onDidOpenTextDocument: s.documents.onDidOpenEmitter.event,
    onDidCloseTextDocument: s.documents.onDidCloseEmitter.event,
    onDidChangeTextDocument: s.documents.onDidChangeEmitter.event,
    onDidSaveTextDocument: new types.EventEmitter<unknown>().event,
    onWillSaveTextDocument: new types.EventEmitter<unknown>().event,
    fs: s.workspace.createFileSystem(),
  };

  const languages = {
    registerCompletionItemProvider: (sel: any, provider: any, ...triggers: string[]) => s.languageFeatures.registerCompletionItemProvider(sel, provider, triggers),
    registerHoverProvider: (sel: any, provider: any) => s.languageFeatures.registerHoverProvider(sel, provider),
    registerDefinitionProvider: (sel: any, provider: any) => s.languageFeatures.registerDefinitionProvider(sel, provider),
    registerReferenceProvider: (sel: any, provider: any) => s.languageFeatures.registerReferenceProvider(sel, provider),
    registerDocumentHighlightProvider: (sel: any, provider: any) => s.languageFeatures.registerDocumentHighlightProvider(sel, provider),
    registerDocumentSymbolProvider: (sel: any, provider: any) => s.languageFeatures.registerDocumentSymbolProvider(sel, provider),
    registerDocumentFormattingEditProvider: (sel: any, provider: any) => s.languageFeatures.registerDocumentFormattingEditProvider(sel, provider),
    registerCodeActionsProvider: (sel: any, provider: any) => s.languageFeatures.registerCodeActionsProvider(sel, provider),
    registerSignatureHelpProvider: (sel: any, provider: any, ...triggers: string[]) => s.languageFeatures.registerSignatureHelpProvider(sel, provider, triggers),
    createDiagnosticCollection: (name?: string) => s.workspace.createDiagnosticCollection(name),
    getLanguages: () => Promise.resolve<string[]>([]),
    setTextDocumentLanguage: () => Promise.resolve(undefined),
  };

  const scm = {
    createSourceControl: (id: string, label: string, rootUri?: types.Uri) => s.scm.createSourceControl(id, label, rootUri),
  };

  const tasks = {
    registerTaskProvider: (type: string, provider: any) => s.tasks.registerTaskProvider(type, provider),
    executeTask: (task: unknown) => s.tasks.executeTask(task),
    onDidStartTask: new types.EventEmitter<unknown>().event,
    onDidEndTask: new types.EventEmitter<unknown>().event,
  };

  const debug = {
    registerDebugConfigurationProvider: (type: string, provider: any) => s.debug.registerDebugConfigurationProvider(type, provider),
    registerDebugAdapterDescriptorFactory: () => s.debug.registerDebugAdapterDescriptorFactory(),
    startDebugging: (folder: unknown, config: any) => s.debug.startDebugging(folder, config),
    onDidStartDebugSession: s.debug.onDidStartDebugSessionEmitter.event,
    onDidTerminateDebugSession: s.debug.onDidTerminateDebugSessionEmitter.event,
    onDidReceiveDebugSessionCustomEvent: s.debug.onDidReceiveDebugSessionCustomEvent.event,
    get activeDebugSession() {
      return undefined;
    },
    breakpoints: [],
  };

  const tests = {
    createTestController: (id: string, label: string) => s.testing.createTestController(id, label),
  };

  const notebooks = {
    registerNotebookSerializer: (viewType: string, serializer: any) => s.notebook.registerNotebookSerializer(viewType, serializer),
  };

  const env = {
    appName: "AgentZ",
    appRoot: typeof process !== "undefined" ? process.cwd() : "",
    appHost: "desktop",
    uriScheme: "agentz",
    language: "zh-cn",
    machineId: "agentz-machine",
    sessionId: `agentz-${Date.now()}`,
    isNewAppInstall: false,
    isTelemetryEnabled: false,
    onDidChangeTelemetryEnabled: new types.EventEmitter<boolean>().event,
    remoteName: undefined,
    shell: typeof process !== "undefined" ? (process.env.COMSPEC || process.env.SHELL || "") : "",
    uiKind: types.UIKind.Desktop,
    clipboard: {
      readText: async () => (await s.commands.executeCommand<string>("vscode.env.clipboard.readText")) ?? "",
      writeText: async (v: string) => {
        await s.commands.executeCommand("vscode.env.clipboard.writeText", v);
      },
    },
    openExternal: async (uri: types.Uri | string) => {
      const target = typeof uri === "string" ? uri : uri.toString();
      const res = await s.commands.executeCommand<boolean>("vscode.open", target);
      return res ?? true;
    },
    asExternalUri: (uri: types.Uri) => Promise.resolve(uri),
  };

  return function api(
    extension: ExtensionDescriptionDto,
    allExtensions?: () => ExtensionDescriptionDto[],
    activateExt?: (id: string) => Promise<unknown>,
  ): Record<string, unknown> {
    const toExtensionObject = (desc: ExtensionDescriptionDto) => ({
      id: desc.id,
      extensionUri: types.Uri.file(desc.extensionPath),
      extensionPath: desc.extensionPath,
      isActive: true,
      packageJSON: {
        name: desc.name,
        publisher: desc.publisher,
        version: desc.version,
        displayName: desc.displayName,
        main: desc.main,
        activationEvents: desc.activationEvents,
        contributes: desc.contributes,
        engines: desc.engines,
      },
      extensionKind: types.ExtensionKind.Workspace,
      exports: undefined,
      activate: () => (activateExt ? activateExt(desc.id) : Promise.resolve(undefined)),
    });

    return {
      // namespaces
      commands,
      window: {
        ...window,
        createWebviewPanel: (viewType: string, title: string, showOptions: any, options?: any) =>
          s.webviews.createWebviewPanel(viewType, title, showOptions, options, extension.extensionPath),
      },
      workspace,
      languages,
      scm,
      tasks,
      debug,
      tests,
      notebooks,
      env,
      extensions: {
        getExtension: (id: string) => {
          const list = allExtensions ? allExtensions() : [extension];
          const found = list.find((e) => e.id.toLowerCase() === id.toLowerCase());
          return found ? toExtensionObject(found) : undefined;
        },
        get all() {
          const list = allExtensions ? allExtensions() : [extension];
          return list.map(toExtensionObject);
        },
        onDidChange: new types.EventEmitter<void>().event,
      },
      l10n: {
        t: (message: string | { message: string; args?: Record<string, unknown> | unknown[] }, ...args: unknown[]) => {
          const raw = typeof message === "string" ? message : message.message;
          const replacements = typeof message === "string" ? args : message.args;
          if (!replacements) return raw;
          if (Array.isArray(replacements)) {
            return raw.replace(/\{(\d+)\}/g, (m, idx) =>
              replacements[Number(idx)] !== undefined ? String(replacements[Number(idx)]) : m,
            );
          }
          const dict = replacements as Record<string, unknown>;
          return raw.replace(/\{([a-zA-Z0-9_]+)\}/g, (m, key) =>
            dict[key] !== undefined ? String(dict[key]) : m,
          );
        },
        bundle: undefined,
        uri: undefined,
      },
      // version
      version: HOST_VSCODE_VERSION,
      // types
      Uri: types.Uri,
      Position: types.Position,
      Range: types.Range,
      Selection: types.Selection,
      Location: types.Location,
      Disposable: types.Disposable,
      EventEmitter: types.EventEmitter,
      CancellationTokenSource: types.CancellationTokenSource,
      MarkdownString: types.MarkdownString,
      CompletionItem: types.CompletionItem,
      CompletionList: types.CompletionList,
      SnippetString: types.SnippetString,
      Hover: types.Hover,
      Diagnostic: types.Diagnostic,
      DiagnosticRelatedInformation: types.DiagnosticRelatedInformation,
      TextEdit: types.TextEdit,
      WorkspaceEdit: types.WorkspaceEdit,
      CodeAction: types.CodeAction,
      CodeActionKind: types.CodeActionKind,
      DocumentSymbol: types.DocumentSymbol,
      SignatureHelp: types.SignatureHelp,
      SignatureInformation: types.SignatureInformation,
      ParameterInformation: types.ParameterInformation,
      ThemeIcon: types.ThemeIcon,
      ThemeColor: types.ThemeColor,
      TreeItem: types.TreeItem,
      RelativePattern: types.RelativePattern,
      FileSystemError: types.FileSystemError,
      // enums
      CompletionItemKind: types.CompletionItemKind,
      DiagnosticSeverity: types.DiagnosticSeverity,
      SymbolKind: types.SymbolKind,
      StatusBarAlignment: types.StatusBarAlignment,
      ConfigurationTarget: types.ConfigurationTarget,
      ViewColumn: types.ViewColumn,
      TreeItemCollapsibleState: types.TreeItemCollapsibleState,
      FileType: types.FileType,
      EndOfLine: types.EndOfLine,
      TextEditorRevealType: types.TextEditorRevealType,
      QuickPickItemKind: types.QuickPickItemKind,
      ProgressLocation: types.ProgressLocation,
      LogLevel: types.LogLevel,
      UIKind: types.UIKind,
      ExtensionKind: types.ExtensionKind,
      ExtensionMode: types.ExtensionMode,
      TextDocumentSaveReason: types.TextDocumentSaveReason,
      OverviewRulerLane: types.OverviewRulerLane,
      CodeActionTriggerKind: types.CodeActionTriggerKind,
      SignatureHelpTriggerKind: types.SignatureHelpTriggerKind,
    };
  };
}
