import type { RunEventCursor, RunEventStream } from "./runEvents";

const PAYLOAD_BYTES = 1024 * 1024;
// Six UTF-8 identities (including the repeated task ID), escaped JSON keys and safe integers.
const ENVELOPE_BYTES = PAYLOAD_BYTES + 4096;
const bytes = (value: unknown) => new TextEncoder().encode(JSON.stringify(value)).byteLength;

function object(value: unknown, keys: string[]): Record<string, unknown> {
  if (
    !value ||
    Object.getPrototypeOf(value) !== Object.prototype ||
    Object.keys(value).length !== keys.length ||
    keys.some((key) => !Object.prototype.hasOwnProperty.call(value, key))
  )
    throw new Error("Invalid committed event schema");
  return value as Record<string, unknown>;
}
function identity(value: unknown): string {
  if (
    typeof value !== "string" ||
    !value ||
    value.trim() !== value ||
    new TextEncoder().encode(value).byteLength > 256 ||
    Array.from(value).some(
      (character) => character.charCodeAt(0) < 32 || character.charCodeAt(0) === 127,
    )
  )
    throw new Error("Invalid committed event identity");
  return value;
}
function sequence(value: unknown, minimum = 0): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < minimum)
    throw new Error("Invalid committed event sequence");
  return value;
}
function streamKey(value: unknown): string {
  const record = object(value, ["task_id", "run_id", "attempt_id", "environment_id"]);
  return JSON.stringify(
    [record.task_id, record.run_id, record.attempt_id, record.environment_id].map(identity),
  );
}
function cursor(value: unknown): { key: string; sequence: number } {
  const record = object(value, ["stream", "sequence"]);
  return { key: streamKey(record.stream), sequence: sequence(record.sequence) };
}
function validateJson(value: unknown): void {
  let nodes = 0;
  const visit = (item: unknown, depth: number): void => {
    if (++nodes > 10000 || depth > 16) throw new Error("Committed event payload exceeds budget");
    if (item === null || typeof item === "boolean") return;
    if (typeof item === "string" && item.length <= 1048576) return;
    if (typeof item === "number" && Number.isFinite(item)) return;
    if (Array.isArray(item)) {
      item.forEach((child) => visit(child, depth + 1));
      return;
    }
    if (item && Object.getPrototypeOf(item) === Object.prototype) {
      Object.entries(item).forEach(([key, child]) => {
        visit(key, depth + 1);
        visit(child, depth + 1);
      });
      return;
    }
    throw new Error("Invalid committed event JSON payload");
  };
  visit(value, 0);
  if (bytes(value) > PAYLOAD_BYTES) throw new Error("Committed event payload exceeds budget");
}

/** Validates an initial notice before the authoritative current stream is known. */
export function committedNoticeBytes(envelope: unknown): number {
  const notice = object(envelope, ["sessionId", "channel", "payload"]);
  const session = identity(notice.sessionId);
  if (notice.channel !== "agent_event_committed")
    throw new Error("Invalid committed event channel");
  const event = object(notice.payload, ["cursor", "event_id", "kind", "payload", "created_ms"]);
  const received = cursor(event.cursor);
  const stream = (event.cursor as RunEventCursor).stream;
  if (stream.task_id !== session) throw new Error("Cross-task committed notice");
  sequence(received.sequence, 1);
  const id = identity(event.event_id);
  if (!id.startsWith("agent.") || id.length === 6 || event.kind !== "agent.event")
    throw new Error("Invalid committed agent event identity");
  sequence(event.created_ms);
  validateJson(event.payload);
  const size = bytes(envelope);
  if (size > ENVELOPE_BYTES) throw new Error("Committed event envelope exceeds budget");
  return size;
}

/** A notification requests replay only. It never acknowledges a cursor or projects text. */
export function committedHighWatermark(
  stream: RunEventStream,
  envelope: unknown,
  verifiedCursor: RunEventCursor | null,
  currentHighWatermark = 0,
): number | null {
  const expected = streamKey(stream);
  const high = sequence(currentHighWatermark);
  const verified = verifiedCursor === null ? null : cursor(verifiedCursor);
  if (verified && verified.key !== expected) throw new Error("Cross-stream verified cursor");
  const notice = object(envelope, ["sessionId", "channel", "payload"]);
  if (typeof notice.channel !== "string") throw new Error("Invalid committed event channel");
  if (notice.channel !== "agent_event_committed") return null;
  if (notice.sessionId !== null) identity(notice.sessionId);
  if (notice.sessionId !== stream.task_id) return null;
  const event = object(notice.payload, ["cursor", "event_id", "kind", "payload", "created_ms"]);
  const received = cursor(event.cursor);
  if (received.key !== expected) return null;
  committedNoticeBytes(envelope);
  // Agent payload remains opaque here; strict display validation belongs to replay projection.
  return Math.max(high, verified?.sequence ?? 0, received.sequence);
}
