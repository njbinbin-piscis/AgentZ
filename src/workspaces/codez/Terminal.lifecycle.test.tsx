import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
const api = vi.hoisted(() => ({ create: vi.fn(), destroy: vi.fn(), listen: vi.fn(), write: vi.fn(), resize: vi.fn(), focus: vi.fn(), count: vi.fn(), destroyAll: vi.fn(), output: vi.fn(), alive: vi.fn(), confirm: vi.fn() }));
vi.mock("../../services/tauri/ide", () => ({ ideApi: { terminalCreate: api.create, terminalDestroy: api.destroy, terminalWrite: api.write, terminalResize: api.resize, terminalIsAlive: api.alive, terminalCount: api.count, terminalDestroyAll: api.destroyAll }, onTerminalOutput: api.listen }));
vi.mock("../../services/tauri/confirm", () => ({ confirmDialog: api.confirm }));
vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (key: string) => key }) }));
vi.mock("./useAppTheme", () => ({ cssVar: (_: string, fallback: string) => fallback }));
vi.mock("@xterm/addon-fit", () => ({ FitAddon: class { fit() {} } }));
vi.mock("@xterm/addon-web-links", () => ({ WebLinksAddon: class {} }));
vi.mock("@xterm/xterm", () => ({ Terminal: class {
  cols = 80; rows = 24; options = {};
  loadAddon() {} open() {} onData() {} onResize() {} attachCustomKeyEventHandler() {}
  dispose() {} write(data: string) { api.output(data); } focus() { api.focus(); } getSelection() { return ""; }
} }));
import TerminalPanel, { confirmTerminalCloseOnProjectChange, destroyAllTerminals } from "./Terminal";
let renderer: ReactTestRenderer | undefined;
const props = (projectDir = "A", visible = true) => ({ projectDir, visible, onHide: vi.fn() });
beforeEach(() => {
  vi.stubGlobal("document", { documentElement: { dataset: {} } });
  vi.stubGlobal("MutationObserver", class { observe() {} disconnect() {} });
  vi.stubGlobal("ResizeObserver", class { observe() {} disconnect() {} });
  vi.stubGlobal("window", { addEventListener() {}, removeEventListener() {}, setTimeout: () => 0, clearTimeout() {} });
  vi.stubGlobal("requestAnimationFrame", () => 0); vi.stubGlobal("cancelAnimationFrame", () => {});
  api.create.mockReset().mockResolvedValue(undefined); api.destroy.mockReset().mockResolvedValue(undefined);
  api.listen.mockReset().mockResolvedValue(() => {});
});
afterEach(async () => { await act(async () => renderer?.unmount()); renderer = undefined; vi.unstubAllGlobals(); });
it("hiding and changing selection callbacks preserve the existing PTY", async () => {
  await act(async () => { renderer = create(<TerminalPanel {...props()} />, { createNodeMock: () => ({}) }); });
  expect(api.create).toHaveBeenCalledTimes(1);
  await act(async () => { renderer!.update(<TerminalPanel {...props("A", false)} onSendSelectionToChat={vi.fn()} />); });
  expect(api.destroy).not.toHaveBeenCalled();
  await act(async () => { renderer!.update(<TerminalPanel {...props()} onSendSelectionToChat={vi.fn()} />); });
  expect(api.create).toHaveBeenCalledTimes(1);
});
it("project changes allocate a different ID and retire the old terminal", async () => {
  await act(async () => { renderer = create(<TerminalPanel {...props()} />, { createNodeMock: () => ({}) }); });
  const oldId = api.create.mock.calls[0][0];
  await act(async () => { renderer!.update(<TerminalPanel {...props("B")} />); });
  const newCalls = api.create.mock.calls.filter(call => call[1] === "B");
  expect(newCalls.length).toBeGreaterThan(0);
  expect(newCalls.every(call => call[0] !== oldId)).toBe(true);
  expect(api.destroy).toHaveBeenCalledWith(oldId);
});

it("late close confirmation cannot remove a new project's terminal", async () => {
  let answer!: (value: boolean) => void;
  api.alive.mockResolvedValue(true);
  api.confirm.mockReturnValue(new Promise<boolean>(resolve => { answer = resolve; }));
  await act(async () => { renderer = create(<TerminalPanel {...props()} />, { createNodeMock: () => ({}) }); });
  await act(async () => { renderer!.root.findAllByType("button").find(b => b.props.title === "ide.terminalNewTab")!.props.onClick(); });
  await act(async () => { renderer!.root.findAllByType("button").find(b => b.props.title === "ide.terminalCloseTab")!.props.onClick({stopPropagation() {}}); });
  await act(async () => { renderer!.update(<TerminalPanel {...props("B")} />); });
  const before = JSON.stringify(renderer!.toJSON());
  await act(async () => { answer(true); });
  expect(JSON.stringify(renderer!.toJSON())).toBe(before);
});

it("a new listener ignores native output for a retired backend ID", async () => {
  await act(async () => { renderer = create(<TerminalPanel {...props()} />, { createNodeMock: () => ({}) }); });
  const oldId = api.create.mock.calls[0][0];
  await act(async () => { renderer!.update(<TerminalPanel {...props("B")} />); });
  const newId = api.create.mock.calls[api.create.mock.calls.length - 1][0];
  const listener = api.listen.mock.calls[api.listen.mock.calls.length - 1][0];
  api.output.mockClear();
  listener({id: oldId, data: "retired"});
  expect(api.output).not.toHaveBeenCalled();
  listener({id: newId, data: "owned"});
  expect(api.output).toHaveBeenCalledWith("owned");
});

it("uncertain count refuses navigation and cleanup errors propagate", async () => {
  api.count.mockRejectedValue(new Error("count unavailable"));
  expect(await confirmTerminalCloseOnProjectChange(key => key)).toBe(false);
  api.destroyAll.mockRejectedValue(new Error("cleanup failed"));
  await expect(destroyAllTerminals()).rejects.toThrow("cleanup failed");
});

it("delayed terminal activation respects focus on the owning dock tablist", async () => {
  const callbacks: Array<() => void> = [];
  vi.stubGlobal("document", {documentElement: {dataset: {}}, activeElement: {closest: () => ({})}});
  vi.stubGlobal("window", {addEventListener() {}, removeEventListener() {}, setTimeout: (callback: () => void) => { callbacks.push(callback); return 0; }, clearTimeout() {}});
  api.focus.mockClear();
  await act(async () => { renderer = create(<TerminalPanel {...props()} />, {createNodeMock: () => ({closest: () => ({contains: () => true})})}); });
  callbacks.forEach(callback => callback());
  expect(api.focus).not.toHaveBeenCalled();
});
