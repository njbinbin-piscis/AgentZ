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
  storagePath?: string;
  globalStorageUri: types.Uri;
  globalStoragePath: string;
  logUri: types.Uri;
  logPath: string;
  extensionMode: number;
  extension: Record<string, unknown>;
  environmentVariableCollection: Record<string, unknown>;
  languageModelAccessInformation: Record<string, unknown>;
  secrets: {
    get(k: string): Promise<string | undefined>;
    store(k: string, v: string): Promise<void>;
    delete(k: string): Promise<void>;
    onDidChange: types.Event<{ key: string }>;
  };
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

const packageJsonCache = new Map<string, Record<string, unknown>>();

export function readRawPackageJson(extensionPath: string): Record<string, unknown> | undefined {
  if (!extensionPath) return undefined;
  const cached = packageJsonCache.get(extensionPath);
  if (cached) return cached;
  try {
    const pkgPath = path.join(extensionPath, "package.json");
    if (fs.existsSync(pkgPath)) {
      const parsed = JSON.parse(fs.readFileSync(pkgPath, "utf8")) as Record<string, unknown>;
      packageJsonCache.set(extensionPath, parsed);
      return parsed;
    }
  } catch {
    // ignore
  }
  return undefined;
}

export function buildExtensionObject(
  desc: ExtensionDescriptionDto,
  activateExt?: (id: string) => Promise<unknown>,
): Record<string, unknown> {
  const rawPkg = readRawPackageJson(desc.extensionPath);
  return {
    id: desc.id,
    extensionUri: types.Uri.file(desc.extensionPath),
    extensionPath: desc.extensionPath,
    isActive: true,
    packageJSON: {
      ...(rawPkg ?? {}),
      name: desc.name,
      publisher: desc.publisher,
      version: desc.version,
      displayName: desc.displayName ?? rawPkg?.displayName,
      main: desc.main ?? rawPkg?.main,
      activationEvents: desc.activationEvents,
      contributes: desc.contributes ?? rawPkg?.contributes,
      engines: desc.engines ?? rawPkg?.engines,
    },
    extensionKind: types.ExtensionKind.Workspace,
    exports: undefined,
    activate: () => (activateExt ? activateExt(desc.id) : Promise.resolve(undefined)),
  };
}

function matchSingleSelector(
  sel: unknown,
  doc: { uri?: types.Uri; languageId?: string; fileName?: string } | undefined,
): number {
  if (!doc) return 0;
  if (typeof sel === "string") {
    if (sel === "*") return 5;
    return sel === doc.languageId ? 10 : 0;
  }
  if (sel && typeof sel === "object") {
    const filter = sel as { language?: string; scheme?: string; pattern?: string | { pattern?: string } };
    let score = 0;
    if (filter.scheme) {
      if (filter.scheme === "*") score = Math.max(score, 5);
      else if (filter.scheme === doc.uri?.scheme) score = 10;
      else return 0;
    }
    if (filter.language) {
      if (filter.language === "*") score = Math.max(score, 5);
      else if (filter.language === doc.languageId) score = 10;
      else return 0;
    }
    if (filter.pattern) {
      score = Math.max(score, 5);
    }
    return score;
  }
  return 0;
}

function matchDocumentSelector(
  selector: unknown,
  document: { uri?: types.Uri; languageId?: string; fileName?: string } | undefined,
): number {
  if (Array.isArray(selector)) {
    let best = 0;
    for (const s of selector) {
      const score = matchDocumentSelector(s, document);
      if (score > best) best = score;
    }
    return best;
  }
  return matchSingleSelector(selector, document);
}

const noopDisposable = () => new types.Disposable(() => undefined);

export function createApiFactory(services: Services) {
  const s = services;

  const commands = {
    registerCommand: (id: string, cb: (...a: unknown[]) => unknown, thisArg?: unknown) =>
      s.commands.registerCommand(id, cb, thisArg),
    registerTextEditorCommand: (id: string, cb: (...a: unknown[]) => unknown, thisArg?: unknown) =>
      s.commands.registerCommand(id, cb, thisArg),
    executeCommand: <T>(id: string, ...args: unknown[]) => s.commands.executeCommand<T>(id, ...args),
    getCommands: () => s.commands.getCommands(),
  };

  const wrapEditor = (document: unknown) => ({
    document,
    selection: new types.Selection(0, 0, 0, 0),
    selections: [new types.Selection(0, 0, 0, 0)],
    visibleRanges: [new types.Range(0, 0, 0, 0)],
    options: { tabSize: 2, insertSpaces: true },
    viewColumn: types.ViewColumn.One,
    edit: () => Promise.resolve(true),
    insertSnippet: () => Promise.resolve(true),
    setDecorations: () => undefined,
    revealRange: () => undefined,
    show: () => undefined,
    hide: () => undefined,
  });

  const tabGroups = {
    all: [] as unknown[],
    activeTabGroup: {
      isActive: true,
      viewColumn: types.ViewColumn.One,
      activeTab: undefined,
      tabs: [] as unknown[],
    },
    onDidChangeTabGroups: new types.EventEmitter<{
      opened: readonly unknown[];
      closed: readonly unknown[];
      changed: readonly unknown[];
    }>().event,
    onDidChangeTabs: new types.EventEmitter<{
      opened: readonly unknown[];
      closed: readonly unknown[];
      changed: readonly unknown[];
    }>().event,
    close: () => Promise.resolve(true),
  };

  const window = {
    showInformationMessage: (m: string, ...r: unknown[]) => s.window.showInformationMessage(m, ...r),
    showWarningMessage: (m: string, ...r: unknown[]) => s.window.showWarningMessage(m, ...r),
    showErrorMessage: (m: string, ...r: unknown[]) => s.window.showErrorMessage(m, ...r),
    showOpenDialog: () => Promise.resolve(undefined),
    showSaveDialog: () => Promise.resolve(undefined),
    showWorkspaceFolderPick: () => Promise.resolve(undefined),
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
    onDidWriteTerminalData: s.window.onDidWriteTerminalDataEmitter.event,
    onDidExecuteTerminalCommand: s.window.onDidExecuteTerminalCommandEmitter.event,
    onDidStartTerminalShellExecution: s.window.onDidStartTerminalShellExecutionEmitter.event,
    onDidEndTerminalShellExecution: s.window.onDidEndTerminalShellExecutionEmitter.event,
    onDidChangeTerminalShellIntegration: s.window.onDidChangeTerminalShellIntegrationEmitter.event,
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
    registerWebviewPanelSerializer: noopDisposable,
    registerCustomEditorProvider: noopDisposable,
    registerUriHandler: noopDisposable,
    registerFileDecorationProvider: noopDisposable,
    registerTerminalLinkProvider: noopDisposable,
    registerTerminalProfileProvider: noopDisposable,
    createTextEditorDecorationType: noopDisposable,
    tabGroups,
    activeColorTheme: { kind: types.ColorThemeKind.Dark },
    onDidChangeActiveColorTheme: new types.EventEmitter<{ kind: number }>().event,
    get activeTextEditor() {
      const doc = s.documents.getActiveDocument();
      return doc ? wrapEditor(doc) : undefined;
    },
    get visibleTextEditors() {
      return s.documents.all().map((document) => wrapEditor(document));
    },
    onDidChangeActiveTextEditor: (
      l: (e: unknown) => unknown,
      thisArgs?: unknown,
      disposables?: { dispose(): unknown }[],
    ) =>
      s.documents.onDidChangeActiveEmitter.event(
        (doc) => l.call(thisArgs, doc ? wrapEditor(doc) : undefined),
        undefined,
        disposables,
      ),
    onDidChangeVisibleTextEditors: new types.EventEmitter<unknown[]>().event,
    onDidChangeTextEditorSelection: new types.EventEmitter<unknown>().event,
    onDidChangeTextEditorVisibleRanges: new types.EventEmitter<unknown>().event,
    onDidChangeTextEditorOptions: new types.EventEmitter<unknown>().event,
    onDidChangeTextEditorViewColumn: new types.EventEmitter<unknown>().event,
    onDidChangeWindowState: new types.EventEmitter<{ focused: boolean }>().event,
    state: { focused: true, active: true },
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
    showNotebookDocument: async (doc: unknown) => {
      const uri =
        doc && typeof doc === "object" && "uri" in doc
          ? (doc as { uri: types.Uri }).uri
          : types.Uri.parse("untitled:Notebook-1.ipynb");
      return {
        notebook: doc ?? {
          uri,
          notebookType: "jupyter-notebook",
          version: 1,
          isDirty: false,
          isUntitled: true,
          isClosed: false,
          metadata: {},
          cellCount: 0,
          cellAt: () => undefined,
          getCells: () => [],
          save: () => Promise.resolve(true),
        },
        selection: new types.NotebookRange(0, 0),
        selections: [new types.NotebookRange(0, 0)],
        visibleRanges: [new types.NotebookRange(0, 0)],
        viewColumn: types.ViewColumn.One,
        revealRange: () => undefined,
      };
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
    get workspaceFile() {
      return undefined;
    },
    get textDocuments() {
      return s.documents.all();
    },
    notebookDocuments: [] as unknown[],
    get isTrusted() {
      return true;
    },
    onDidGrantWorkspaceTrust: new types.EventEmitter<void>().event,
    getConfiguration: (section?: string) => s.workspace.getConfiguration(section),
    getWorkspaceFolder: (uri: types.Uri) => s.workspace.getWorkspaceFolder(uri),
    updateWorkspaceFolders: () => true,
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
    openNotebookDocument: async (uriOrType?: types.Uri | string, content?: types.NotebookData) => {
      const uri =
        uriOrType instanceof types.Uri
          ? uriOrType
          : types.Uri.parse(`untitled:Untitled-${Date.now()}.ipynb`);
      const notebookType = typeof uriOrType === "string" ? uriOrType : "jupyter-notebook";
      const cells = (content?.cells ?? []).map((c, idx) => ({
        index: idx,
        notebook: undefined as unknown,
        kind: c.kind,
        document: {
          uri: types.Uri.parse(`${uri.toString()}#cell${idx}`),
          languageId: c.languageId,
          version: 1,
          getText: () => c.value,
          lineCount: c.value.split(/\r?\n/).length,
        },
        metadata: c.metadata ?? {},
        outputs: c.outputs ?? [],
        executionSummary: c.executionSummary,
      }));
      return {
        uri,
        notebookType,
        version: 1,
        isDirty: false,
        isUntitled: uri.scheme === "untitled",
        isClosed: false,
        metadata: content?.metadata ?? {},
        cellCount: cells.length,
        cellAt: (idx: number) => cells[idx],
        getCells: () => cells,
        save: () => Promise.resolve(true),
      };
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
    registerTextDocumentContentProvider: noopDisposable,
    registerFileSystemProvider: noopDisposable,
    registerPortAttributesProvider: noopDisposable,
    applyEdit: () => s.workspace.applyEdit(),
    save: () => Promise.resolve(undefined),
    saveAs: () => Promise.resolve(undefined),
    saveAll: () => Promise.resolve(true),
    onDidChangeConfiguration: s.workspace.onDidChangeConfigurationEmitter.event,
    onDidChangeWorkspaceFolders: s.workspace.onDidChangeWorkspaceFoldersEmitter.event,
    onDidOpenTextDocument: s.documents.onDidOpenEmitter.event,
    onDidCloseTextDocument: s.documents.onDidCloseEmitter.event,
    onDidChangeTextDocument: s.documents.onDidChangeEmitter.event,
    onDidSaveTextDocument: new types.EventEmitter<unknown>().event,
    onWillSaveTextDocument: new types.EventEmitter<unknown>().event,
    onDidOpenNotebookDocument: new types.EventEmitter<unknown>().event,
    onDidCloseNotebookDocument: new types.EventEmitter<unknown>().event,
    onDidChangeNotebookDocument: new types.EventEmitter<unknown>().event,
    onDidSaveNotebookDocument: new types.EventEmitter<unknown>().event,
    onDidCreateFiles: new types.EventEmitter<{ readonly files: readonly types.Uri[] }>().event,
    onDidDeleteFiles: new types.EventEmitter<{ readonly files: readonly types.Uri[] }>().event,
    onDidRenameFiles: new types.EventEmitter<{
      readonly files: readonly { readonly oldUri: types.Uri; readonly newUri: types.Uri }[];
    }>().event,
    onWillCreateFiles: new types.EventEmitter<unknown>().event,
    onWillDeleteFiles: new types.EventEmitter<unknown>().event,
    onWillRenameFiles: new types.EventEmitter<unknown>().event,
    fs: s.workspace.createFileSystem(),
  };

  const languages = {
    match: (selector: unknown, document: { uri?: types.Uri; languageId?: string; fileName?: string }) =>
      matchDocumentSelector(selector, document),
    setLanguageConfiguration: noopDisposable,
    getDiagnostics: () => [],
    onDidChangeDiagnostics: new types.EventEmitter<{ readonly uris: readonly types.Uri[] }>().event,
    createLanguageStatusItem: (id: string, selector: unknown) => ({
      id,
      name: id,
      selector,
      severity: types.LanguageStatusSeverity.Information,
      text: "",
      detail: "",
      busy: false,
      command: undefined,
      accessibilityInformation: undefined,
      dispose: () => undefined,
    }),
    registerCompletionItemProvider: (sel: any, provider: any, ...triggers: string[]) =>
      s.languageFeatures.registerCompletionItemProvider(sel, provider, triggers),
    registerHoverProvider: (sel: any, provider: any) =>
      s.languageFeatures.registerHoverProvider(sel, provider),
    registerDefinitionProvider: (sel: any, provider: any) =>
      s.languageFeatures.registerDefinitionProvider(sel, provider),
    registerDeclarationProvider: (sel: any, provider: any) =>
      s.languageFeatures.registerDefinitionProvider(sel, {
        provideDefinition: (doc: unknown, pos: unknown, tok: unknown) =>
          provider?.provideDeclaration?.(doc, pos, tok),
      }),
    registerTypeDefinitionProvider: noopDisposable,
    registerImplementationProvider: noopDisposable,
    registerReferenceProvider: (sel: any, provider: any) =>
      s.languageFeatures.registerReferenceProvider(sel, provider),
    registerDocumentHighlightProvider: (sel: any, provider: any) =>
      s.languageFeatures.registerDocumentHighlightProvider(sel, provider),
    registerDocumentSymbolProvider: (sel: any, provider: any) =>
      s.languageFeatures.registerDocumentSymbolProvider(sel, provider),
    registerWorkspaceSymbolProvider: noopDisposable,
    registerDocumentFormattingEditProvider: (sel: any, provider: any) =>
      s.languageFeatures.registerDocumentFormattingEditProvider(sel, provider),
    registerDocumentRangeFormattingEditProvider: noopDisposable,
    registerOnTypeFormattingEditProvider: noopDisposable,
    registerCodeActionsProvider: (sel: any, provider: any) =>
      s.languageFeatures.registerCodeActionsProvider(sel, provider),
    registerCodeLensProvider: noopDisposable,
    registerSignatureHelpProvider: (sel: any, provider: any, ...triggers: string[]) =>
      s.languageFeatures.registerSignatureHelpProvider(
        sel,
        provider,
        Array.isArray(triggers[0]) ? triggers[0] : triggers,
      ),
    registerRenameProvider: noopDisposable,
    registerDocumentLinkProvider: noopDisposable,
    registerColorProvider: noopDisposable,
    registerFoldingRangeProvider: noopDisposable,
    registerSelectionRangeProvider: noopDisposable,
    registerCallHierarchyProvider: noopDisposable,
    registerTypeHierarchyProvider: noopDisposable,
    registerLinkedEditingRangeProvider: noopDisposable,
    registerInlayHintsProvider: noopDisposable,
    registerInlineValuesProvider: noopDisposable,
    registerInlineCompletionItemProvider: noopDisposable,
    registerDocumentSemanticTokensProvider: noopDisposable,
    registerDocumentRangeSemanticTokensProvider: noopDisposable,
    registerEvaluatableExpressionProvider: noopDisposable,
    registerDocumentDropEditProvider: noopDisposable,
    registerDocumentPasteEditProvider: noopDisposable,
    createDiagnosticCollection: (name?: string) => s.workspace.createDiagnosticCollection(name),
    getLanguages: () => Promise.resolve<string[]>([]),
    setTextDocumentLanguage: (doc: unknown) => Promise.resolve(doc),
  };

  const scm = {
    createSourceControl: (id: string, label: string, rootUri?: types.Uri) =>
      s.scm.createSourceControl(id, label, rootUri),
  };

  const tasks = {
    registerTaskProvider: (type: string, provider: any) => s.tasks.registerTaskProvider(type, provider),
    executeTask: (task: unknown) => s.tasks.executeTask(task),
    fetchTasks: () => Promise.resolve([]),
    taskExecutions: [] as unknown[],
    onDidStartTask: new types.EventEmitter<unknown>().event,
    onDidEndTask: new types.EventEmitter<unknown>().event,
    onDidStartTaskProcess: new types.EventEmitter<unknown>().event,
    onDidEndTaskProcess: new types.EventEmitter<unknown>().event,
  };

  const debug = {
    registerDebugConfigurationProvider: (type: string, provider: any, _triggerKind?: number) =>
      s.debug.registerDebugConfigurationProvider(type, provider),
    registerDebugAdapterDescriptorFactory: () => s.debug.registerDebugAdapterDescriptorFactory(),
    registerDebugAdapterTrackerFactory: noopDisposable,
    registerDebugVisualizationProvider: noopDisposable,
    registerDebugVisualizationTreeProvider: noopDisposable,
    startDebugging: (folder: unknown, config: any) => s.debug.startDebugging(folder, config),
    stopDebugging: () => Promise.resolve(),
    addBreakpoints: () => undefined,
    removeBreakpoints: () => undefined,
    asDebugSourceUri: (src: unknown) => types.Uri.parse(`debug:${JSON.stringify(src ?? "")}`),
    onDidStartDebugSession: s.debug.onDidStartDebugSessionEmitter.event,
    onDidTerminateDebugSession: s.debug.onDidTerminateDebugSessionEmitter.event,
    onDidChangeActiveDebugSession: new types.EventEmitter<unknown>().event,
    onDidReceiveDebugSessionCustomEvent: s.debug.onDidReceiveDebugSessionCustomEvent.event,
    onDidChangeBreakpoints: new types.EventEmitter<unknown>().event,
    activeDebugConsole: {
      append: () => undefined,
      appendLine: () => undefined,
    },
    get activeDebugSession() {
      return undefined;
    },
    breakpoints: [] as unknown[],
  };

  const tests = {
    createTestController: (id: string, label: string) => s.testing.createTestController(id, label),
    get testResults() {
      return s.testing.testResults;
    },
    onDidChangeTestResults: s.testing.onDidChangeTestResultsEmitter.event,
    createTestObserver: () => s.testing.createTestObserver(),
  };

  const notebooks = {
    registerNotebookSerializer: (viewType: string, serializer: any) =>
      s.notebook.registerNotebookSerializer(viewType, serializer),
    createNotebookController: (id?: string, notebookType?: string, label?: string, handler?: unknown) => ({
      id: id ?? "notebook-controller",
      notebookType: notebookType ?? "*",
      label: label ?? "",
      description: undefined as string | undefined,
      detail: undefined as string | undefined,
      supportedLanguages: [] as string[],
      supportsExecutionOrder: false,
      executeHandler: handler,
      interruptHandler: undefined as unknown,
      variableProvider: undefined as unknown,
      onDidChangeSelectedNotebooks: new types.EventEmitter<unknown>().event,
      createNotebookCellExecution: () => ({
        executionOrder: undefined as number | undefined,
        token: new types.CancellationTokenSource().token,
        start: () => undefined,
        end: () => undefined,
        clearOutput: () => Promise.resolve(),
        appendOutput: () => Promise.resolve(),
        replaceOutput: () => Promise.resolve(),
        appendOutputItems: () => Promise.resolve(),
        replaceOutputItems: () => Promise.resolve(),
      }),
      createNotebookCellStatusBarItemProvider: noopDisposable,
      updateNotebookAffinity: () => undefined,
      dispose: () => undefined,
    }),
    registerNotebookCellStatusBarItemProvider: noopDisposable,
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
    onDidChangeShell: new types.EventEmitter<string>().event,
    logLevel: types.LogLevel.Info,
    onDidChangeLogLevel: new types.EventEmitter<types.LogLevel>().event,
    createTelemetryLogger: (
      sender?: { flush?: () => void | Thenable<void> },
      _options?: unknown,
    ) => {
      const enableEmitter = new types.EventEmitter<unknown>();
      const logger = {
        isUsageEnabled: false,
        isErrorsEnabled: false,
        onDidChangeEnableStates: enableEmitter.event,
        logUsage: () => undefined,
        logError: () => undefined,
        dispose: () => {
          enableEmitter.dispose();
          try {
            void sender?.flush?.();
          } catch {
            // ignore
          }
        },
      };
      return logger;
    },
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

  const lmTools = new Map<string, { name: string; tool: any }>();
  const lm = {
    get tools() {
      return [...lmTools.values()].map((t) => ({
        name: t.name,
        description: "",
        inputSchema: undefined,
        tags: [] as readonly string[],
      }));
    },
    onDidChangeChatModels: new types.EventEmitter<void>().event,
    selectChatModels: () => Promise.resolve([]),
    registerTool: (name: string, tool: any) => {
      lmTools.set(name, { name, tool });
      return new types.Disposable(() => lmTools.delete(name));
    },
    invokeTool: async (name: string, options: any, token?: unknown) => {
      const entry = lmTools.get(name);
      if (entry?.tool && typeof entry.tool.invoke === "function") {
        const tok = token ?? new types.CancellationTokenSource().token;
        const res = await entry.tool.invoke(options, tok);
        if (res) return res;
      }
      return new types.LanguageModelToolResult([]);
    },
    registerMcpServerDefinitionProvider: noopDisposable,
  };

  const authentication = {
    onDidChangeSessions: new types.EventEmitter<unknown>().event,
    getSession: () => Promise.resolve(undefined),
    getAccounts: () => Promise.resolve([]),
    registerAuthenticationProvider: noopDisposable,
  };

  const comments = {
    createCommentController: (id: string, label: string) => ({
      id,
      label,
      options: undefined,
      commentingRangeProvider: undefined,
      reactionHandler: undefined,
      createCommentThread: (uri: types.Uri, range: types.Range, threadComments: unknown[]) => ({
        uri,
        range,
        comments: threadComments,
        collapsibleState: 0,
        canReply: true,
        contextValue: undefined,
        label: undefined,
        state: 0,
        dispose: () => undefined,
      }),
      dispose: () => undefined,
    }),
  };

  const chat = {
    createChatParticipant: (id: string, handler: unknown) => ({
      id,
      requestHandler: handler,
      iconPath: undefined,
      followupProvider: undefined,
      onDidReceiveFeedback: new types.EventEmitter<unknown>().event,
      dispose: () => undefined,
    }),
  };

  return function api(
    extension: ExtensionDescriptionDto,
    allExtensions?: () => ExtensionDescriptionDto[],
    activateExt?: (id: string) => Promise<unknown>,
  ): Record<string, unknown> {
    const toExtensionObject = (desc: ExtensionDescriptionDto) =>
      buildExtensionObject(desc, activateExt);

    return {
      // Export all concrete runtime classes and enums from types-impl
      ...types,
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
      lm,
      authentication,
      comments,
      chat,
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
    };
  };
}
