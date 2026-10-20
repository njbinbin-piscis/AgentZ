// ExtHostTesting: backs vscode.tests.createTestController. Test trees are
// published to the renderer's Test Explorer; runs are requested via $runTests.

import * as dto from "../common/dto";
import { IRPCProtocol } from "../common/proxyIdentifier";
import { ExtHostTestingShape, MainContext, MainThreadTestingShape } from "../common/protocol";
import { CancellationTokenSource, Disposable, EventEmitter, Range, Uri } from "./types-impl";

export class TestItemCollection implements Iterable<[string, TestItem]> {
  private readonly map = new Map<string, TestItem>();
  constructor(
    private readonly owner: TestItem | undefined,
    private readonly onChanged?: () => void,
  ) {}
  add(item: TestItem): void {
    item.parent = this.owner;
    this.map.set(item.id, item);
    this.onChanged?.();
  }
  delete(id: string): void {
    this.map.delete(id);
    this.onChanged?.();
  }
  get(id: string): TestItem | undefined {
    return this.map.get(id);
  }
  replace(items: readonly TestItem[]): void {
    this.map.clear();
    for (const i of items) {
      i.parent = this.owner;
      this.map.set(i.id, i);
    }
    this.onChanged?.();
  }
  forEach(cb: (item: TestItem, collection: TestItemCollection) => unknown, thisArg?: unknown): void {
    for (const item of this.map.values()) {
      cb.call(thisArg, item, this);
    }
  }
  get size(): number {
    return this.map.size;
  }
  values(): IterableIterator<TestItem> {
    return this.map.values();
  }
  clear(): void {
    this.map.clear();
  }
  [Symbol.iterator](): Iterator<[string, TestItem]> {
    return this.map.entries();
  }
}

class TestItem {
  readonly children: TestItemCollection;
  parent?: TestItem;
  range?: Range;
  canResolveChildren = false;
  busy = false;
  description?: string;
  sortText?: string;
  error?: string;
  tags: unknown[] = [];

  constructor(
    public id: string,
    public label: string,
    public uri?: Uri,
    onChanged?: () => void,
  ) {
    this.children = new TestItemCollection(this, onChanged);
  }
  get childCollection(): TestItemCollection {
    return this.children;
  }
}

function toDto(item: TestItem, parentId?: string): dto.TestItemDto {
  return {
    id: item.id,
    label: item.label,
    uri: item.uri?.toJSON(),
    range: item.range
      ? {
          startLine: item.range.start.line,
          startCharacter: item.range.start.character,
          endLine: item.range.end.line,
          endCharacter: item.range.end.character,
        }
      : undefined,
    parentId,
    children: [...item.children.values()].map((c) => toDto(c, item.id)),
  };
}

export interface TestRunResultDto {
  readonly completedAt: number;
  readonly results: readonly unknown[];
}

class TestController {
  readonly items: TestItemCollection;
  resolveHandler?: (item: TestItem | undefined) => Thenable<void> | void;
  refreshHandler?: (token: unknown) => Thenable<void> | void;
  private runHandlers: ((request: unknown, token: unknown) => void)[] = [];

  constructor(
    public readonly id: string,
    public label: string,
    private readonly publish: () => void,
    private readonly onRunFinished?: (result: TestRunResultDto) => void,
  ) {
    this.items = new TestItemCollection(undefined, this.publish);
  }
  createTestItem(id: string, label: string, uri?: Uri): TestItem {
    return new TestItem(id, label, uri, this.publish);
  }
  get itemsCollection(): TestItemCollection {
    return this.items;
  }
  createRunProfile(
    label: string,
    kind: number,
    runHandler: (request: unknown, token: unknown) => void,
    isDefault?: boolean,
    tag?: unknown,
    supportsContinuousRun?: boolean,
  ): {
    label: string;
    kind: number;
    isDefault: boolean;
    tag: unknown;
    supportsContinuousRun: boolean;
    runHandler: (request: unknown, token: unknown) => void;
    configureHandler: undefined;
    loadDetailedCoverage: undefined;
    onDidChangeDefault: ReturnType<typeof ExtHostTesting.emitter<boolean>>["event"];
    dispose(): void;
  } {
    this.runHandlers.push(runHandler);
    const defaultEmitter = new EventEmitter<boolean>();
    return {
      label,
      kind,
      isDefault: !!isDefault,
      tag,
      supportsContinuousRun: !!supportsContinuousRun,
      runHandler,
      configureHandler: undefined,
      loadDetailedCoverage: undefined,
      onDidChangeDefault: defaultEmitter.event,
      dispose: () => defaultEmitter.dispose(),
    };
  }
  createTestRun(_request: unknown, name?: string, persist = true) {
    const cts = new CancellationTokenSource();
    const disposeEmitter = new EventEmitter<void>();
    return {
      name,
      token: cts.token,
      isPersisted: persist,
      enqueued: (_test: unknown) => undefined,
      started: (_test: unknown) => undefined,
      skipped: (_test: unknown) => undefined,
      failed: (_test: unknown, _message: unknown, _duration?: number) => undefined,
      errored: (_test: unknown, _message: unknown, _duration?: number) => undefined,
      passed: (_test: unknown, _duration?: number) => undefined,
      appendOutput: (_output: string, _location?: unknown, _test?: unknown) => undefined,
      addCoverage: (_fileCoverage: unknown) => undefined,
      end: () => {
        if (persist) {
          this.onRunFinished?.({ completedAt: Date.now(), results: [] });
        }
        disposeEmitter.fire();
        disposeEmitter.dispose();
        cts.dispose();
      },
      onDidDispose: disposeEmitter.event,
    };
  }
  invalidateTestResults(_items?: unknown): void {
    /* no-op */
  }
  run(request: unknown): void {
    const cts = new CancellationTokenSource();
    for (const h of this.runHandlers) h(request, cts.token);
  }
  snapshot(): dto.TestItemDto[] {
    return [...this.items.values()].map((i) => toDto(i));
  }
  dispose(): void {
    this.items.clear();
  }
}

export class ExtHostTesting implements ExtHostTestingShape {
  private readonly proxy: MainThreadTestingShape;
  private readonly controllers = new Map<string, TestController>();
  private readonly _testResults: TestRunResultDto[] = [];
  readonly onDidChangeTestResultsEmitter = new EventEmitter<void>();

  constructor(rpc: IRPCProtocol) {
    this.proxy = rpc.getProxy(MainContext.MainThreadTesting);
  }
  get testResults(): readonly TestRunResultDto[] {
    return this._testResults;
  }
  createTestObserver() {
    const diffEmitter = new EventEmitter<unknown>();
    return {
      tests: [] as readonly unknown[],
      onDidChangeTests: diffEmitter.event,
      dispose: () => diffEmitter.dispose(),
    };
  }
  createTestController(id: string, label: string): TestController {
    const controller = new TestController(
      id,
      label,
      () => this.proxy.$publishTestItems(id, controller.snapshot()),
      (res) => {
        this._testResults.push(res);
        this.onDidChangeTestResultsEmitter.fire();
      },
    );
    this.controllers.set(id, controller);
    this.proxy.$registerTestController(id, label);
    return controller;
  }
  async $runTests(controllerId: string, testIds: string[]): Promise<void> {
    const controller = this.controllers.get(controllerId);
    if (!controller) return;
    controller.run({ include: testIds, exclude: [] });
  }
  static disposable(): Disposable {
    return new Disposable(() => undefined);
  }
  static emitter<T>(): EventEmitter<T> {
    return new EventEmitter<T>();
  }
}
