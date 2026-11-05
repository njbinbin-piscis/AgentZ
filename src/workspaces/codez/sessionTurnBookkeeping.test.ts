import { describe, expect, it } from "vitest";
import { SessionTurnBookkeeping, queueRestoreIndex } from "./sessionTurnBookkeeping";

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

  it("moves a queued turn to the front of its own session", () => {
    const book = new SessionTurnBookkeeping<Turn>();
    book.enqueue(turn("a", "a1"));
    book.enqueue(turn("a", "a2"));
    const taken = book.takeAt("a", 1);
    expect(taken?.text).toBe("a2");
    book.enqueueFront(taken!);
    expect(book.queueFor("a").map((x) => x.text)).toEqual(["a2", "a1"]);
    expect(book.queueFor("b")).toEqual([]);
  });

  it("inserts a turn back at an index that still exists", () => {
    const book = new SessionTurnBookkeeping<Turn>();
    book.enqueue(turn("a", "a1"));
    book.enqueue(turn("a", "a3"));
    book.insertAt("a", 1, turn("a", "a2"));
    expect(book.queueFor("a").map((x) => x.text)).toEqual(["a1", "a2", "a3"]);
    book.insertAt("a", 99, turn("a", "a4"));
    expect(book.queueFor("a").map((x) => x.text)).toEqual(["a1", "a2", "a3", "a4"]);
  });

  it("restores an edited turn beside neighbors that are still queued", () => {
    // A B [edited] D, then A entered the iteration and E was appended.
    expect(queueRestoreIndex(["b", "d", "e"], ["a", "b"], ["d"])).toBe(1);
    // A and B both entered; D is still waiting.
    expect(queueRestoreIndex(["d", "e"], ["a", "b"], ["d"])).toBe(0);
    // Everything that was queued is gone; a message typed during the edit stays behind.
    expect(queueRestoreIndex(["e"], ["a", "b"], ["d"])).toBe(0);
    // The following message was removed; the one after it is the anchor.
    expect(queueRestoreIndex(["b", "e"], ["a", "b"], ["d", "e"])).toBe(1);
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
