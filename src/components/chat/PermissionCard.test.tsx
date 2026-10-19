import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { afterEach, describe, expect, it, vi } from "vitest";
const { respond, getPreview } = vi.hoisted(() => ({ respond: vi.fn(), getPreview: vi.fn() }));
vi.mock("../../services/tauri/interactive", () => ({ respondPermissionRequest: respond, getPermissionPreview: getPreview }));
import PermissionCard, { type PermissionRequestCard } from "./PermissionCard";
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
let view: ReactTestRenderer | undefined;
afterEach(async () => { if (view) await act(async () => view?.unmount()); view = undefined; respond.mockReset(); getPreview.mockReset(); vi.useRealTimers(); });
const request: PermissionRequestCard = {
  requestId: "approval:a", toolName: "fixture-send", toolInput: { message: "A" }, description: "Send fixture A",
  approval: { digest: "digest-a", taskId: "task-a", environmentId: "env-a", expiresMs: 2000000000000 },
};
async function render(card = request, onResolved = vi.fn()) {
  getPreview.mockResolvedValue(card);
  await act(async () => { view = create(<PermissionCard request={card} onResolved={onResolved} />); });
  return onResolved;
}
function allow() { return view!.root.findAllByType("button")[1]; }
describe("bound approval card", () => {
  it("dismisses an unavailable host preview locally without granting or denying", async () => {
    getPreview.mockRejectedValue(new Error("request no longer exists"));
    const done = vi.fn();
    await act(async () => { view = create(<PermissionCard request={request} onResolved={done} />); });
    const close = view!.root.findAllByType("button")[0];
    expect(close.props.children).toBe("Dismiss");
    await act(async () => { close.props.onClick(); });
    expect(done).toHaveBeenCalledOnce(); expect(respond).not.toHaveBeenCalled();
  });
  it("an old expired dismiss handler cannot close the replacement request", async () => {
    const done = await render({ ...request, approval: { ...request.approval!, expiresMs: Date.now() - 1 } });
    const closeOld = view!.root.findAllByType("button")[0].props.onClick;
    const next = { ...request, requestId: "approval:b", approval: { ...request.approval!, digest: "digest-b" } };
    getPreview.mockResolvedValue(next);
    await act(async () => { view!.update(<PermissionCard request={next} onResolved={done} />); });
    await act(async () => { closeOld(); });
    expect(done).not.toHaveBeenCalled(); expect(respond).not.toHaveBeenCalled();
  });
  it("retries a failed host preview while approval stays disabled until it succeeds", async () => {
    getPreview.mockRejectedValueOnce(new Error("preview unavailable")).mockResolvedValueOnce(request);
    const done = vi.fn();
    await act(async () => { view = create(<PermissionCard request={request} onResolved={done} />); });
    expect(allow().props.disabled).toBe(true);
    expect(JSON.stringify(view!.toJSON())).toContain("preview unavailable");
    expect(respond).not.toHaveBeenCalled();
    await act(async () => { view!.root.findAllByType("button")[2].props.onClick(); });
    expect(getPreview).toHaveBeenCalledTimes(2); expect(allow().props.disabled).toBe(false);
    expect(JSON.stringify(view!.toJSON())).not.toContain("preview unavailable");
    respond.mockResolvedValue(undefined);
    await act(async () => { allow().props.onClick(); });
    expect(respond).toHaveBeenCalledWith("approval:a", true, "digest-a");
    expect(done).toHaveBeenCalledOnce();
  });
  it("ignores handlers retained from a replaced approval preview", async () => {
    const done = await render();
    const oldButtons = view!.root.findAllByType("button");
    const approveOld = oldButtons[1].props.onClick;
    const denyOld = oldButtons[0].props.onClick;
    const next = { ...request, requestId: "approval:b", approval: { ...request.approval!, digest: "digest-b" } };
    getPreview.mockResolvedValue(next);
    await act(async () => { view!.update(<PermissionCard request={next} onResolved={done} />); });
    await act(async () => { approveOld(); denyOld(); });
    expect(respond).not.toHaveBeenCalled(); expect(done).not.toHaveBeenCalled();
    respond.mockResolvedValue(undefined);
    await act(async () => { allow().props.onClick(); });
    expect(respond).toHaveBeenCalledWith("approval:b", true, "digest-b");
    expect(done).toHaveBeenCalledOnce();
  });
  it("ignores a retained approval handler after unmount", async () => {
    const done = await render(); const approve = allow().props.onClick;
    await act(async () => { view!.unmount(); }); view = undefined;
    await act(async () => { approve(); });
    expect(respond).not.toHaveBeenCalled(); expect(done).not.toHaveBeenCalled();
  });
  it("sends one decision for rapid approve and deny clicks before rendering", async () => {
    let finish!: () => void;
    respond.mockImplementation(() => new Promise<void>(resolve => { finish = resolve; }));
    const done = await render();
    const buttons = view!.root.findAllByType("button");
    await act(async () => {
      buttons[1].props.onClick(); buttons[1].props.onClick(); buttons[0].props.onClick();
    });
    expect(respond).toHaveBeenCalledOnce();
    expect(respond).toHaveBeenCalledWith("approval:a", true, "digest-a");
    await act(async () => { finish(); });
    expect(done).toHaveBeenCalledOnce();
  });
  it("rejects an approve handler while the trusted preview is still loading", async () => {
    getPreview.mockImplementation(() => new Promise(() => {}));
    await act(async () => { view = create(<PermissionCard request={request} onResolved={vi.fn()} />); });
    await act(async () => { allow().props.onClick(); });
    expect(respond).not.toHaveBeenCalled();
  });
  it("rejects a trusted preview with an invalid deadline", async () => {
    await render({ ...request, approval: { ...request.approval!, expiresMs: Number.NaN } });
    expect(allow().props.disabled).toBe(true);
    expect(JSON.stringify(view!.toJSON())).toContain("Invalid host approval preview");
    expect(respond).not.toHaveBeenCalled();
  });
  it("disables expired approval and dismisses locally without sending a decision", async () => {
    const done = await render({ ...request, approval: { ...request.approval!, expiresMs: Date.now() - 1 } });
    expect(allow().props.disabled).toBe(true);
    await act(async () => { allow().props.onClick(); });
    expect(respond).not.toHaveBeenCalled();
    await act(async () => { view!.root.findAllByType("button")[0].props.onClick(); });
    expect(done).toHaveBeenCalledOnce();
  });
  it("expires while open and rechecks a suspended clock at click time", async () => {
    vi.useFakeTimers(); vi.setSystemTime(1000);
    await render({ ...request, approval: { ...request.approval!, expiresMs: 2000 } });
    expect(allow().props.disabled).toBe(false);
    vi.setSystemTime(2001);
    await act(async () => { allow().props.onClick(); });
    expect(respond).not.toHaveBeenCalled(); expect(allow().props.disabled).toBe(true);
  });
  it("disables approval when its deadline timer fires", async () => {
    vi.useFakeTimers(); vi.setSystemTime(1000);
    await render({ ...request, approval: { ...request.approval!, expiresMs: 2000 } });
    await act(async () => { vi.advanceTimersByTime(1001); });
    expect(allow().props.disabled).toBe(true); expect(respond).not.toHaveBeenCalled();
  });
  it("displays the original task/environment and submits its exact preview digest", async () => {
    respond.mockResolvedValue(undefined); const done = await render();
    const serialized = JSON.stringify(view!.toJSON()); expect(serialized).toContain("task-a"); expect(serialized).toContain("env-a");
    await act(async () => { await allow().props.onClick(); });
    expect(respond).toHaveBeenCalledWith("approval:a", true, "digest-a"); expect(done).toHaveBeenCalledOnce();
  });
  it("does not allow a new approval namespace request without a bound preview", async () => {
    await render({ ...request, approval: undefined }); expect(allow().props.disabled).toBe(true);
  });
  it("does not acknowledge delivery failure as resolution", async () => {
    respond.mockRejectedValue(new Error("receiver closed; not delivered")); const done = await render();
    await act(async () => { await allow().props.onClick(); });
    expect(done).not.toHaveBeenCalled(); expect(JSON.stringify(view!.toJSON())).toContain("not delivered");
    expect(allow().props.disabled).toBe(false);
    respond.mockResolvedValue(undefined);
    await act(async () => { allow().props.onClick(); });
    expect(respond).toHaveBeenCalledTimes(2); expect(done).toHaveBeenCalledOnce();
  });
  it.each(["approval:b", "approval:a"])("an old response cannot dismiss a changed card %s while awaiting delivery", async (nextId) => {
    let complete!: () => void; respond.mockImplementation(() => new Promise<void>(resolve => { complete = resolve; }));
    const done = await render(); await act(async () => { allow().props.onClick(); });
    const next = { ...request, requestId: nextId, approval: { ...request.approval!, digest: "digest-b", taskId: "task-b" } };
    getPreview.mockResolvedValue(next);
    await act(async () => { view!.update(<PermissionCard request={next} onResolved={done} />); });
    await act(async () => { complete(); });
    expect(done).not.toHaveBeenCalled(); expect(JSON.stringify(view!.toJSON())).toContain("task-b");
    expect(allow().props.disabled).toBe(false);
  });
  it("ignores spoofed event parameters and displays the host snapshot", async () => {
    const spoof = { ...request, toolInput: { message: "forged" }, description: "forged", approval: { ...request.approval!, taskId: "forged-task" } };
    getPreview.mockResolvedValue(request);
    await act(async () => { view = create(<PermissionCard request={spoof} onResolved={vi.fn()} />); });
    const text=JSON.stringify(view!.toJSON()); expect(text).toContain("task-a"); expect(text).not.toContain("forged");
  });
  it("submits alwaysAllow=true when Always allow button is clicked", async () => {
    respond.mockResolvedValue(undefined);
    const done = await render();
    const alwaysAllowBtn = view!.root.findAllByType("button")[2];
    expect(alwaysAllowBtn.props.children).toBe("Always allow");
    await act(async () => { await alwaysAllowBtn.props.onClick(); });
    expect(respond).toHaveBeenCalledWith("approval:a", true, "digest-a", true);
    expect(done).toHaveBeenCalledOnce();
  });

});
