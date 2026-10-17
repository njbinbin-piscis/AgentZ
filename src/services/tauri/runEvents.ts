import { invoke } from "@tauri-apps/api/core";

export interface RunEventStream {
  task_id: string;
  run_id: string;
  attempt_id: string;
  environment_id: string;
}
export interface RunEventCursor {
  stream: RunEventStream;
  sequence: number;
}
export interface DurableRunEvent {
  cursor: RunEventCursor;
  event_id: string;
  kind: string;
  payload: unknown;
  created_ms: number;
}
export interface RunEventPage {
  stream: RunEventStream;
  events: DurableRunEvent[];
  next_cursor: RunEventCursor | null;
  has_more: boolean;
}

function identity(value: string): void {
  if (
    typeof value !== "string" ||
    !value ||
    value.trim() !== value ||
    value.length > 256 ||
    Array.from(value).some(
      (character) => character.charCodeAt(0) < 32 || character.charCodeAt(0) === 127,
    )
  ) {
    throw new Error("Invalid durable event identity");
  }
}
function streamKey(stream: RunEventStream): string {
  if (!stream || typeof stream !== "object") throw new Error("Missing durable event stream");
  const fields = [stream.task_id, stream.run_id, stream.attempt_id, stream.environment_id];
  fields.forEach(identity);
  return JSON.stringify(fields);
}
function cursorSequence(cursor: RunEventCursor, stream: RunEventStream): number {
  if (!cursor || streamKey(cursor.stream) !== streamKey(stream))
    throw new Error("Cross-stream event cursor");
  if (!Number.isSafeInteger(cursor.sequence) || cursor.sequence < 0)
    throw new Error("Invalid event sequence");
  return cursor.sequence;
}
function jsonKey(value: unknown): string {
  if (value === null || typeof value === "string" || typeof value === "boolean")
    return JSON.stringify(value);
  if (typeof value === "number" && Number.isFinite(value)) return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(jsonKey).join(",")}]`;
  if (value && typeof value === "object" && Object.getPrototypeOf(value) === Object.prototype) {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${jsonKey((value as Record<string, unknown>)[key])}`)
      .join(",")}}`;
  }
  throw new Error("Invalid durable event JSON payload");
}

/** Passive lifecycle history only; reading events never resumes a model or side effect. */
export async function readRunEvents(
  scopeHandle: string,
  sessionId: string,
  runId: string,
  attemptId: string,
  cursor: RunEventCursor | null = null,
  limit = 200,
): Promise<RunEventPage> {
  [scopeHandle, sessionId, runId, attemptId].forEach(identity);
  if (!Number.isInteger(limit) || limit < 1 || limit > 200)
    throw new Error("Event page limit must be 1..200");
  if (cursor) {
    cursorSequence(cursor, cursor.stream);
    if (
      cursor.stream.task_id !== sessionId ||
      cursor.stream.run_id !== runId ||
      cursor.stream.attempt_id !== attemptId
    )
      throw new Error("Cursor does not belong to requested Run/Attempt");
  }
  const page = await invoke<RunEventPage>("chat_run_events", {
    scopeHandle,
    sessionId,
    runId,
    attemptId,
    cursor,
    limit,
  });
  streamKey(page.stream);
  if (
    page.stream.task_id !== sessionId ||
    page.stream.run_id !== runId ||
    page.stream.attempt_id !== attemptId
  )
    throw new Error("Response belongs to another Run/Attempt");
  if (cursor && streamKey(page.stream) !== streamKey(cursor.stream))
    throw new Error("Response environment changed");
  if (page.events.length > limit) throw new Error("Event page exceeds requested limit");
  // Validate the first received page against its request cursor as well as its DTO.
  new RunEventCollector(page.stream, cursor, 1).append(page);
  return page;
}

/** Bounded, scoped, atomic page reducer. Exact replays deduplicate; conflicts fail closed. */
export class RunEventCollector {
  private readonly stream: RunEventStream;
  private readonly known = new Map<number, DurableRunEvent>();
  private readonly ids = new Map<string, number>();
  private sequence: number;
  private pages = 0;
  constructor(
    stream: RunEventStream,
    after: RunEventCursor | null = null,
    private readonly maxPages = 100,
  ) {
    streamKey(stream);
    if (!Number.isInteger(maxPages) || maxPages < 1 || maxPages > 1000)
      throw new Error("Invalid event page budget");
    this.stream = { ...stream };
    this.sequence = after ? cursorSequence(after, stream) : 0;
  }
  append(page: RunEventPage): DurableRunEvent[] {
    if (this.pages >= this.maxPages) throw new Error("Event page budget exhausted");
    if (streamKey(page.stream) !== streamKey(this.stream))
      throw new Error("Cross-stream event page");
    if (
      !Array.isArray(page.events) ||
      page.events.length > 200 ||
      typeof page.has_more !== "boolean"
    )
      throw new Error("Invalid bounded event page");
    const known = new Map(this.known);
    const ids = new Map(this.ids);
    let sequence = this.sequence;
    let previous = 0;
    for (const event of page.events) {
      const current = cursorSequence(event.cursor, this.stream);
      if (current < 1 || current < previous) throw new Error("Event order regressed");
      previous = current;
      identity(event.event_id);
      identity(event.kind);
      if (!Number.isSafeInteger(event.created_ms) || event.created_ms < 0)
        throw new Error("Invalid event timestamp");
      jsonKey(event.payload);
      const duplicate = known.get(current);
      if (duplicate) {
        if (jsonKey(duplicate) !== jsonKey(event))
          throw new Error("Conflicting event at the same sequence");
        continue;
      }
      if (ids.has(event.event_id)) throw new Error("Stable event identity changed sequence");
      if (current !== sequence + 1) throw new Error("Event sequence gap or unknown replay");
      known.set(current, JSON.parse(JSON.stringify(event)) as DurableRunEvent);
      ids.set(event.event_id, current);
      sequence = current;
    }
    const expectedNext = page.events.length ? previous : this.sequence;
    if (
      page.next_cursor === null
        ? expectedNext !== 0
        : cursorSequence(page.next_cursor, this.stream) !== expectedNext
    )
      throw new Error("Invalid next event cursor");
    if (page.has_more && sequence <= this.sequence)
      throw new Error("Event pagination did not advance");
    this.known.clear();
    known.forEach((event, key) => this.known.set(key, event));
    this.ids.clear();
    ids.forEach((seq, id) => this.ids.set(id, seq));
    this.sequence = sequence;
    this.pages += 1;
    return this.snapshot();
  }
  snapshot(): DurableRunEvent[] {
    return [...this.known.values()].map(
      (event) => JSON.parse(JSON.stringify(event)) as DurableRunEvent,
    );
  }
  cursor(): RunEventCursor | null {
    return this.sequence === 0 ? null : { stream: { ...this.stream }, sequence: this.sequence };
  }
  /** Isolated draft for one bounded replay batch; existing verified identities remain immutable. */
  fork(): RunEventCollector {
    const draft = new RunEventCollector(this.stream, this.cursor(), this.maxPages);
    this.known.forEach((event, key) => {
      draft.known.set(key, JSON.parse(JSON.stringify(event)) as DurableRunEvent);
    });
    this.ids.forEach((sequence, id) => draft.ids.set(id, sequence));
    return draft;
  }
}
