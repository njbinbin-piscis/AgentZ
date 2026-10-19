// ExtHostWorkspace + configuration + filesystem + diagnostics, backed by the
// renderer's MainThreadWorkspace (which proxies to the Tauri host fs/git).

import * as fs from "fs";
import * as path from "path";
import * as dto from "../common/dto";
import { IRPCProtocol } from "../common/proxyIdentifier";
import {
  ExtHostConfigurationShape,
  ExtHostWorkspaceShape,
  MainContext,
  MainThreadDiagnosticsShape,
  MainThreadWorkspaceShape,
} from "../common/protocol";
import * as conv from "./converters";
import { Diagnostic, Disposable, EventEmitter, FileSystemError, FileType, RelativePattern, Uri } from "./types-impl";

function setNestedProperty(target: Record<string, unknown>, dottedKey: string, value: unknown): void {
  const parts = dottedKey.split(".").filter(Boolean);
  if (parts.length === 0) return;
  let cur: Record<string, unknown> = target;
  for (let i = 0; i < parts.length - 1; i++) {
    const part = parts[i];
    const existing = cur[part];
    if (!existing || typeof existing !== "object" || Array.isArray(existing)) {
      cur[part] = {};
    }
    cur = cur[part] as Record<string, unknown>;
  }
  cur[parts[parts.length - 1]] = value;
}

export class WorkspaceConfiguration {
  private readonly _section!: string | undefined;
  private readonly _data!: Record<string, unknown>;
  private readonly _defaults!: Record<string, unknown>;
  private readonly _proxy!: MainThreadWorkspaceShape;

  constructor(
    section: string | undefined,
    data: Record<string, unknown>,
    proxy: MainThreadWorkspaceShape,
    defaults: Record<string, unknown> = {},
  ) {
    Object.defineProperty(this, "_section", { value: section, enumerable: false });
    Object.defineProperty(this, "_data", { value: data, enumerable: false });
    Object.defineProperty(this, "_defaults", { value: defaults, enumerable: false });
    Object.defineProperty(this, "_proxy", { value: proxy, enumerable: false });

    const prefix = section ? `${section}.` : "";
    const merged: Record<string, unknown> = { ...defaults, ...data };
    for (const [k, v] of Object.entries(merged)) {
      if (prefix) {
        if (k.startsWith(prefix)) {
          setNestedProperty(this as unknown as Record<string, unknown>, k.slice(prefix.length), v);
        }
      } else {
        setNestedProperty(this as unknown as Record<string, unknown>, k, v);
      }
    }
    if (section && section in merged && merged[section] && typeof merged[section] === "object") {
      Object.assign(this, merged[section]);
    }
  }
  private key(k: string): string {
    return this._section ? `${this._section}.${k}` : k;
  }
  get<T>(key: string, defaultValue?: T): T | undefined {
    const full = this.key(key);
    if (full in this._data && this._data[full] !== undefined) return this._data[full] as T;
    if (this._section && this._section in this._data) {
      const sub = this._data[this._section] as Record<string, unknown> | undefined;
      if (sub && typeof sub === "object" && key in sub && sub[key] !== undefined) return sub[key] as T;
    }
    if (full in this._defaults && this._defaults[full] !== undefined) return this._defaults[full] as T;
    return defaultValue;
  }
  has(key: string): boolean {
    const full = this.key(key);
    return full in this._data || full in this._defaults;
  }
  inspect<T>(key: string): {
    key: string;
    defaultValue?: T;
    globalValue?: T;
    workspaceValue?: T;
    workspaceFolderValue?: T;
  } {
    const full = this.key(key);
    const userVal = full in this._data ? (this._data[full] as T) : undefined;
    const defVal = full in this._defaults ? (this._defaults[full] as T) : undefined;
    return {
      key: full,
      defaultValue: defVal,
      globalValue: userVal,
      workspaceValue: userVal,
      workspaceFolderValue: undefined,
    };
  }
  update(key: string, value: unknown, target = 1): Promise<void> {
    const full = this.key(key);
    this._data[full] = value;
    return this._proxy.$updateConfiguration(full, value, target as number);
  }
}

export class DiagnosticCollection {
  constructor(public readonly name: string, private readonly proxy: MainThreadDiagnosticsShape) {}
  private store = new Map<string, Diagnostic[]>();
  set(uriOrEntries: Uri | readonly [Uri, readonly Diagnostic[] | undefined][], diagnostics?: readonly Diagnostic[]): void {
    if (Array.isArray(uriOrEntries)) {
      for (const [uri, diags] of uriOrEntries) {
        if (!diags) this.store.delete(uri.toString());
        else this.store.set(uri.toString(), [...diags]);
      }
      this.flush();
      return;
    }
    const uri = uriOrEntries as Uri;
    if (!diagnostics) {
      this.store.delete(uri.toString());
    } else {
      this.store.set(uri.toString(), [...diagnostics]);
    }
    this.flush();
  }
  get(uri: Uri): readonly Diagnostic[] | undefined {
    return this.store.get(uri.toString());
  }
  has(uri: Uri): boolean {
    return this.store.has(uri.toString());
  }
  forEach(cb: (uri: Uri, diagnostics: readonly Diagnostic[], collection: DiagnosticCollection) => unknown, thisArg?: unknown): void {
    for (const [k, v] of this.store.entries()) {
      cb.call(thisArg, Uri.parse(k), v, this);
    }
  }
  delete(uri: Uri): void {
    this.store.delete(uri.toString());
    this.flush();
  }
  clear(): void {
    this.store.clear();
    this.proxy.$clear(this.name);
  }
  dispose(): void {
    this.clear();
  }
  private flush(): void {
    const entries: [dto.UriComponents, dto.DiagnosticDto[]][] = [];
    for (const [key, diags] of this.store) {
      entries.push([Uri.parse(key).toJSON(), diags.map(conv.fromDiagnostic)]);
    }
    this.proxy.$changeMany(this.name, entries);
  }
}

export class FileSystemWatcher {
  private readonly createEmitter = new EventEmitter<Uri>();
  private readonly changeEmitter = new EventEmitter<Uri>();
  private readonly deleteEmitter = new EventEmitter<Uri>();
  private fsWatcher?: fs.FSWatcher;

  readonly onDidCreate = this.createEmitter.event;
  readonly onDidChange = this.changeEmitter.event;
  readonly onDidDelete = this.deleteEmitter.event;

  constructor(
    pattern: string | RelativePattern,
    public readonly ignoreCreateEvents = false,
    public readonly ignoreChangeEvents = false,
    public readonly ignoreDeleteEvents = false,
    private readonly onDispose?: () => void,
  ) {
    try {
      const baseDir =
        typeof pattern === "object" && pattern !== null && "base" in pattern
          ? pattern.base
          : undefined;
      const relPattern =
        typeof pattern === "object" && pattern !== null && "pattern" in pattern
          ? pattern.pattern
          : typeof pattern === "string"
            ? pattern
            : "";
      if (baseDir && fs.existsSync(baseDir) && relPattern && !relPattern.includes("*")) {
        const targetPath = path.join(baseDir, relPattern);
        let existed = fs.existsSync(targetPath);
        this.fsWatcher = fs.watch(baseDir, (_eventType, filename) => {
          if (!filename || path.basename(targetPath) !== filename) return;
          const nowExists = fs.existsSync(targetPath);
          const targetUri = Uri.file(targetPath);
          if (!existed && nowExists) {
            existed = true;
            if (!this.ignoreCreateEvents) this.createEmitter.fire(targetUri);
          } else if (existed && !nowExists) {
            existed = false;
            if (!this.ignoreDeleteEvents) this.deleteEmitter.fire(targetUri);
          } else if (nowExists) {
            if (!this.ignoreChangeEvents) this.changeEmitter.fire(targetUri);
          }
        });
      }
    } catch {
      // Ignore native watcher errors on unsupported paths
    }
  }

  dispose(): void {
    try {
      this.fsWatcher?.close();
    } catch {
      // ignore
    }
    this.createEmitter.dispose();
    this.changeEmitter.dispose();
    this.deleteEmitter.dispose();
    this.onDispose?.();
  }
}

export class ExtHostFileSystem {
  constructor(private readonly proxy: MainThreadWorkspaceShape) {}

  async readFile(uri: Uri): Promise<Uint8Array> {
    if (uri.scheme === "file" && uri.fsPath) {
      try {
        const buf = await fs.promises.readFile(uri.fsPath);
        return new Uint8Array(buf.buffer, buf.byteOffset, buf.byteLength);
      } catch {
        // Fall through to RPC proxy
      }
    }
    const text = await this.proxy.$readFile(uri.toJSON());
    return new TextEncoder().encode(text);
  }

  async writeFile(uri: Uri, content: Uint8Array): Promise<void> {
    if (uri.scheme === "file" && uri.fsPath) {
      try {
        await fs.promises.mkdir(path.dirname(uri.fsPath), { recursive: true });
        await fs.promises.writeFile(uri.fsPath, content);
        return;
      } catch {
        // Fall through to RPC proxy
      }
    }
    await this.proxy.$writeFile(uri.toJSON(), new TextDecoder().decode(content));
  }

  async stat(uri: Uri): Promise<{ type: number; ctime: number; mtime: number; size: number }> {
    if (uri.scheme === "file" && uri.fsPath) {
      try {
        const st = await fs.promises.stat(uri.fsPath);
        const type = st.isDirectory()
          ? FileType.Directory
          : st.isSymbolicLink()
            ? FileType.SymbolicLink
            : FileType.File;
        return {
          type,
          ctime: Math.floor(st.ctimeMs),
          mtime: Math.floor(st.mtimeMs),
          size: st.size,
        };
      } catch {
        throw FileSystemError.FileNotFound(uri);
      }
    }
    const res = await this.proxy.$stat(uri.toJSON());
    if (!res) {
      throw FileSystemError.FileNotFound(uri);
    }
    return {
      type: res.type,
      ctime: Date.now(),
      mtime: Date.now(),
      size: res.size,
    };
  }

  async readDirectory(uri: Uri): Promise<[string, number][]> {
    if (uri.scheme === "file" && uri.fsPath) {
      try {
        const entries = await fs.promises.readdir(uri.fsPath, { withFileTypes: true });
        return entries.map((e) => [
          e.name,
          e.isDirectory() ? FileType.Directory : e.isSymbolicLink() ? FileType.SymbolicLink : FileType.File,
        ]);
      } catch {
        // Fall through to RPC proxy
      }
    }
    return this.proxy.$readDirectory(uri.toJSON());
  }

  async createDirectory(uri: Uri): Promise<void> {
    if (uri.scheme === "file" && uri.fsPath) {
      await fs.promises.mkdir(uri.fsPath, { recursive: true });
    }
  }

  async delete(uri: Uri, options?: { recursive?: boolean; useTrash?: boolean }): Promise<void> {
    if (uri.scheme === "file" && uri.fsPath) {
      try {
        await fs.promises.rm(uri.fsPath, { recursive: options?.recursive ?? false, force: true });
        return;
      } catch {
        // Fall through to RPC proxy
      }
    }
    await this.proxy.$delete(uri.toJSON(), options?.recursive ?? false);
  }

  async rename(source: Uri, target: Uri, options?: { overwrite?: boolean }): Promise<void> {
    if (source.scheme === "file" && target.scheme === "file") {
      if (!options?.overwrite && fs.existsSync(target.fsPath)) {
        throw FileSystemError.FileExists(target);
      }
      await fs.promises.rename(source.fsPath, target.fsPath);
    }
  }

  async copy(source: Uri, target: Uri, options?: { overwrite?: boolean }): Promise<void> {
    if (source.scheme === "file" && target.scheme === "file") {
      await fs.promises.cp(source.fsPath, target.fsPath, {
        recursive: true,
        force: options?.overwrite ?? false,
      });
    }
  }

  isWritableFileSystem(_scheme: string): boolean {
    return true;
  }
}

export class ExtHostWorkspace implements ExtHostWorkspaceShape, ExtHostConfigurationShape {
  private readonly proxy: MainThreadWorkspaceShape;
  private readonly diagProxy: MainThreadDiagnosticsShape;
  private folders: Uri[] = [];
  private configData: Record<string, unknown> = {};
  private configDefaults: Record<string, unknown> = {};
  private watcherSeq = 0;

  readonly onDidChangeConfigurationEmitter = new EventEmitter<{ affectsConfiguration: (s: string) => boolean }>();
  readonly onDidChangeWorkspaceFoldersEmitter = new EventEmitter<void>();

  constructor(rpc: IRPCProtocol) {
    this.proxy = rpc.getProxy(MainContext.MainThreadWorkspace);
    this.diagProxy = rpc.getProxy(MainContext.MainThreadDiagnostics);
  }

  initialize(folders: dto.UriComponents[], config: Record<string, unknown>): void {
    this.folders = folders.map(Uri.from);
    this.configData = config;
  }

  registerConfigurationDefaults(defaults: Record<string, unknown>): void {
    Object.assign(this.configDefaults, defaults);
  }

  get workspaceFolders(): { uri: Uri; name: string; index: number }[] {
    return this.folders.map((uri, index) => ({
      uri,
      name: uri.path.replace(/\/+$/, "").split("/").pop() ?? uri.path,
      index,
    }));
  }
  get rootPath(): string | undefined {
    return this.folders[0]?.fsPath;
  }

  getConfiguration(section?: string): WorkspaceConfiguration {
    return new WorkspaceConfiguration(section, this.configData, this.proxy, this.configDefaults);
  }

  getWorkspaceFolder(uri: Uri): { uri: Uri; name: string; index: number } | undefined {
    const normUriPath = uri.path.replace(/\\/g, "/").replace(/^\/([a-zA-Z]:)/, (_, d: string) => `/${d.toLowerCase()}`);
    return this.workspaceFolders.find((f) => {
      const normFolderPath = f.uri.path
        .replace(/\\/g, "/")
        .replace(/^\/([a-zA-Z]:)/, (_, d: string) => `/${d.toLowerCase()}`)
        .replace(/\/+$/, "");
      return normUriPath === normFolderPath || normUriPath.startsWith(normFolderPath + "/");
    });
  }

  async findFiles(
    include: string | RelativePattern,
    exclude?: string | RelativePattern | null,
    maxResults = 1000,
  ): Promise<Uri[]> {
    const includeGlob = typeof include === "string" ? include : include.pattern;
    const excludeGlob =
      typeof exclude === "string" ? exclude : exclude && typeof exclude === "object" ? exclude.pattern : undefined;
    const result = await this.proxy.$findFiles(includeGlob, excludeGlob, maxResults);
    return result.map(Uri.from);
  }

  applyEdit(): Promise<boolean> {
    return Promise.resolve(false);
  }

  createDiagnosticCollection(name?: string): DiagnosticCollection {
    return new DiagnosticCollection(name ?? `diag-${Math.random().toString(36).slice(2)}`, this.diagProxy);
  }

  createFileSystem(): ExtHostFileSystem {
    return new ExtHostFileSystem(this.proxy);
  }

  registerFileSystemWatcher(
    globPattern: string | RelativePattern,
    ignoreCreateEvents?: boolean,
    ignoreChangeEvents?: boolean,
    ignoreDeleteEvents?: boolean,
  ): FileSystemWatcher {
    void ++this.watcherSeq;
    return new FileSystemWatcher(
      globPattern,
      ignoreCreateEvents,
      ignoreChangeEvents,
      ignoreDeleteEvents,
    );
  }

  $acceptConfigurationChanged(data: Record<string, unknown>): void {
    this.configData = data;
    this.onDidChangeConfigurationEmitter.fire({
      affectsConfiguration: (s: string) =>
        s in data || Object.keys(data).some((k) => k.startsWith(`${s}.`)),
    });
  }

  $acceptWorkspaceFoldersChanged(folders: dto.UriComponents[]): void {
    this.folders = folders.map(Uri.from);
    this.onDidChangeWorkspaceFoldersEmitter.fire();
  }
}
