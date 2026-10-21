import { describe, expect, it } from "vitest";
import { SessionTurnBookkeeping } from "./sessionTurnBookkeeping";

interface Turn {
  sessionId: string;
  text: string;
}

const turn = (sessionId: string, text: string): Turn => ({ sessionId, text });

describe("SessionTurnBookkeeping", () => {
  it("tracks running state per session", () => {
    const book = new SessionTurnBookkeeping<Turn>();
    book.markRunning("a", true);
    expect(book.isRunning("a")).toBe(true);
    expect(book.isRunning("b")).toBe(false);
    expect(book.runningIds()).toEqual(["a"]);
    book.markRunning("b", true);
    book.markRunning("a", false);
    expect(book.isRunning("a")).toBe(false);
    expect(book.runningIds()).toEqual(["b"]);
  });

  it("queues follow-ups on their own session only", () => {
    const book = new SessionTurnBookkeeping<Turn>();
    book.enqueue(turn("a", "a1"));
    book.enqueue(turn("a", "a2"));
    book.enqueue(turn("b", "b1"));
    expect(book.queueFor("a").map((x) => x.text)).toEqual(["a1", "a2"]);
    expect(book.queueFor("b").map((x) => x.text)).toEqual(["b1"]);
    expect(book.queueFor("c")).toEqual([]);
    expect(book.queueFor(null)).toEqual([]);
  });

  it("drains only the finishing session's queue", () => {
    const book = new SessionTurnBookkeeping<Turn>();
    book.enqueue(turn("a", "a1"));
    book.enqueue(turn("b", "b1"));
    expect(book.shiftNext("a")?.text).toBe("a1");
    expect(book.shiftNext("a")).toBeUndefined();
    // b's queued turn is untouched by a finishing.
    expect(book.queueFor("b").map((x) => x.text)).toEqual(["b1"]);
  });

  it("removes a queued turn from its own session only", () => {
    const book = new SessionTurnBookkeeping<Turn>();
    book.enqueue(turn("a", "a1"));
    book.enqueue(turn("b", "b1"));
    book.removeAt("a", 0);
    expect(book.queueFor("a")).toEqual([]);
    expect(book.queueFor("b").map((x) => x.text)).toEqual(["b1"]);
  });

  it("rekeys a queue when the backend renames the session", () => {
    const book = new SessionTurnBookkeeping<Turn>();
    book.enqueue(turn("draft", "hello"));
    book.rekey("draft", "real", (x) => ({ ...x, sessionId: "real" }));
    expect(book.queueFor("draft")).toEqual([]);
    expect(book.queueFor("real")).toEqual([{ sessionId: "real", text: "hello" }]);
  });

  it("deleting or clearing drops queues without touching running flags", () => {
    const book = new SessionTurnBookkeeping<Turn>();
    book.markRunning("a", true);
    book.enqueue(turn("a", "a1"));
    book.enqueue(turn("b", "b1"));
    book.deleteSession("a");
    expect(book.queueFor("a")).toEqual([]);
    expect(book.isRunning("a")).toBe(true);
    book.clearQueues();
    expect(book.queueFor("b")).toEqual([]);
  });
});
