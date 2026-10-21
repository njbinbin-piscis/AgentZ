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
