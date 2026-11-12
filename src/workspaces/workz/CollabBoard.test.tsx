import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import CollabBoard from "./CollabBoard";
import type { KoiTodo } from "../../services/tauri/pool";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

const poolGet = vi.fn();
const poolMembers = vi.fn();
const poolTodos = vi.fn();
const poolMessages = vi.fn();
const onPoolEvent = vi.fn();
const onChatEvent = vi.fn();

vi.mock("../../services/tauri/pool", () => ({
  poolGet: (...args: unknown[]) => poolGet(...args),
  poolMembers: (...args: unknown[]) => poolMembers(...args),
  poolTodos: (...args: unknown[]) => poolTodos(...args),
  poolMessages: (...args: unknown[]) => poolMessages(...args),
  onPoolEvent: (...args: unknown[]) => onPoolEvent(...args),
}));

vi.mock("../../services/tauri/chat", () => ({
  onChatEvent: (...args: unknown[]) => onChatEvent(...args),
}));

vi.mock("./CollabBoard.css", () => ({}));

const todo = (id: string, status: string): KoiTodo => ({
  id,
  owner_id: "koi-1",
  title: `task-${id}`,
  description: "",
  status,
  priority: "medium",
  assigned_by: "piscis",
  pool_session_id: "pool-1",
  claimed_by: null,
});

describe("CollabBoard", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    poolGet.mockResolvedValue({ id: "pool-1", name: "Test Pool", status: "active" });
    poolMembers.mockResolvedValue([
      { koi_id: "koi-1", name: "Koi One", role: "dev", icon: "", color: "", status: "idle" },
    ]);
    poolTodos.mockResolvedValue([
      todo("t1", "todo"),
      todo("t2", "in_progress"),
      todo("t3", "needs_review"),
      todo("t4", "done"),
    ]);
    poolMessages.mockResolvedValue([]);
    onPoolEvent.mockResolvedValue(() => {});
    onChatEvent.mockResolvedValue(() => {});
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  async function renderBoard(): Promise<ReactTestRenderer> {
    let renderer!: ReactTestRenderer;
    await act(async () => {
      renderer = create(
        <CollabBoard projectDir="D:/proj" poolId="pool-1" onClose={() => {}} />,
      );
      await Promise.resolve();
    });
    return renderer;
  }

  it("shows needs_review todos in a dedicated review column", async () => {
    const renderer = await renderBoard();
    const text = JSON.stringify(renderer.toJSON());
    expect(text).toContain("collab.col_review");
    expect(text).toContain("task-t3");
    // The review card must not be swallowed by the done column.
    const reviewCol = renderer.root.findAll(
      (n) => n.props.className === "agentz-collab-col",
    );
    expect(reviewCol.length).toBe(4);
  });

  it("polls for reconciliation every 15s even without pool events", async () => {
    await renderBoard();
    const initialCalls = poolTodos.mock.calls.length;
    expect(initialCalls).toBeGreaterThanOrEqual(1);
    await act(async () => {
      vi.advanceTimersByTime(15_000);
      await Promise.resolve();
    });
    expect(poolTodos.mock.calls.length).toBeGreaterThan(initialCalls);
  });

  it("stops polling after unmount", async () => {
    const renderer = await renderBoard();
    await act(async () => {
      renderer.unmount();
      await Promise.resolve();
    });
    const callsAtUnmount = poolTodos.mock.calls.length;
    await act(async () => {
      vi.advanceTimersByTime(30_000);
      await Promise.resolve();
    });
    expect(poolTodos.mock.calls.length).toBe(callsAtUnmount);
  });
});
