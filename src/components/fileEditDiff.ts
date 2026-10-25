import { diffLines, type DiffOp } from "../workspaces/codez/lineDiff";

export interface FileEditDiff {
  path: string;
  ops: DiffOp[];
  added: number;
  removed: number;
}

/** LCS is O(n·m); beyond this a replaced block is shown as remove + add. */
const MAX_LCS_CELLS = 2_000_000;

function splitLines(s: string): string[] {
  if (s === "") return [];
  return s.replace(/\r\n/g, "\n").replace(/\n$/, "").split("\n");
}

function diffBlock(oldText: string, newText: string): DiffOp[] {
  const a = splitLines(oldText);
  const b = splitLines(newText);
  if (a.length * b.length <= MAX_LCS_CELLS) return diffLines(a, b);
  return [
    ...a.map((text) => ({ type: "remove" as const, text, bIndex: -1 })),
    ...b.map((text, bIndex) => ({ type: "add" as const, text, bIndex })),
  ];
}

/** Diff described by a file_edit / file_write call, or null for other tools. */
export function fileEditDiff(name: string, input: unknown): FileEditDiff | null {
  const i = input as Record<string, unknown> | null;
  if (!i || typeof i.path !== "string" || !i.path) return null;
  let ops: DiffOp[];
  if (name === "file_write" && typeof i.content === "string") {
    ops = diffBlock("", i.content);
  } else if (name === "file_edit") {
    const edits = Array.isArray(i.edits)
      ? (i.edits as Record<string, unknown>[])
      : [{ old_string: i.old_string, new_string: i.new_string }];
    const blocks = edits
      .filter((e) => typeof e?.old_string === "string" && typeof e?.new_string === "string")
      .map((e) => diffBlock(e.old_string as string, e.new_string as string));
    if (blocks.length === 0) return null;
    ops = blocks.flatMap((block, n) =>
      n === 0 ? block : [{ type: "equal" as const, text: "⋯", bIndex: -1 }, ...block],
    );
  } else {
    return null;
  }
  return {
    path: i.path,
    ops,
    added: ops.filter((o) => o.type === "add").length,
    removed: ops.filter((o) => o.type === "remove").length,
  };
}
