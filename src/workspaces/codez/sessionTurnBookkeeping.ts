/**
 * Per-session turn bookkeeping for the CodeZ assistant panel.
 *
 * Several sessions may be in flight at once (the backend gives each turn its
 * own task key), so running flags and follow-up queues must be keyed by
 * session id. Panel-wide singletons here were the root of a cross-session
 * leak: a message sent in a fresh session was queued behind an unrelated
 * session's turn, and that queue was then displayed in every session.
 */
export interface SessionTurnLike {
  sessionId: string;
}

export class SessionTurnBookkeeping<T extends SessionTurnLike> {
  private running = new Set<string>();
  private queues = new Map<string, T[]>();

  markRunning(id: string | null, on: boolean): void {
    if (!id) return;
    if (on) this.running.add(id);
    else this.running.delete(id);
  }

  isRunning(id: string | null | undefined): boolean {
    return !!id && this.running.has(id);
  }

  runningIds(): string[] {
    return Array.from(this.running);
  }

  /** Append a follow-up turn to its own session's queue; returns that queue. */
  enqueue(turn: T): readonly T[] {
    const q = this.queues.get(turn.sessionId) ?? [];
    q.push(turn);
    this.queues.set(turn.sessionId, q);
    return q;
  }

  queueFor(id: string | null | undefined): readonly T[] {
    if (!id) return [];
    return this.queues.get(id) ?? [];
  }

  /** Remove and return the next queued turn for this session only. */
  shiftNext(id: string): T | undefined {
    return this.queues.get(id)?.shift();
  }

  /** Remove the item at `index` and return it. */
  takeAt(id: string, index: number): T | undefined {
    const queue = this.queues.get(id);
    if (!queue || index < 0 || index >= queue.length) return undefined;
    return queue.splice(index, 1)[0];
  }

  /** Put a follow-up at the front of its session queue. */
  enqueueFront(turn: T): readonly T[] {
    return this.insertAt(turn.sessionId, 0, turn);
  }

  /** Insert a follow-up at `index`, clamped to the queue that exists now. */
  insertAt(sessionId: string, index: number, turn: T): readonly T[] {
    const queue = this.queues.get(sessionId) ?? [];
    const at = Math.max(0, Math.min(index, queue.length));
    queue.splice(at, 0, turn);
    this.queues.set(sessionId, queue);
    return queue;
  }

  removeAt(id: string, index: number): void {
    this.queues.get(id)?.splice(index, 1);
  }

  /** Move a queue when the backend assigns a different session id. */
  rekey(from: string, to: string, remap: (turn: T) => T): void {
    const q = this.queues.get(from);
    if (!q) return;
    this.queues.delete(from);
    this.queues.set(to, q.map(remap));
  }

  deleteSession(id: string): void {
    this.queues.delete(id);
  }

  clearQueues(): void {
    this.queues.clear();
  }
}

/**
 * Where a queued message returns after the user edits it.
 * `beforeIds` / `afterIds` are the neighbors at the moment it left the queue.
 * Neighbors that have already entered the iteration are skipped, so the slot
 * slides forward. Messages added while editing stay behind that slot.
 */
export function queueRestoreIndex(
  currentIds: readonly string[],
  beforeIds: readonly string[],
  afterIds: readonly string[],
): number {
  for (const id of afterIds) {
    const at = currentIds.indexOf(id);
    if (at >= 0) return at;
  }
  for (let i = beforeIds.length - 1; i >= 0; i--) {
    const at = currentIds.indexOf(beforeIds[i]);
    if (at >= 0) return at + 1;
  }
  return 0;
}
