import ELK from "elkjs/lib/elk.bundled.js";

const elk = new ELK();

/** Layered left-to-right layout via ELK. Falls back to an empty map on failure. */
export async function elkLayout(
  nodes: { id: string; w: number; h: number }[],
  edges: { from: string; to: string }[],
): Promise<Map<string, { x: number; y: number }>> {
  const ids = new Set(nodes.map((n) => n.id));
  try {
    const res = await elk.layout({
      id: "root",
      layoutOptions: {
        "elk.algorithm": "layered",
        "elk.direction": "RIGHT",
        "elk.spacing.nodeNode": "28",
        "elk.layered.spacing.nodeNodeBetweenLayers": "70",
        "elk.layered.cycleBreaking.strategy": "GREEDY",
      },
      children: nodes.map((n) => ({ id: n.id, width: n.w, height: n.h })),
      edges: edges
        .filter((e) => ids.has(e.from) && ids.has(e.to) && e.from !== e.to)
        .map((e, i) => ({ id: `e${i}`, sources: [e.from], targets: [e.to] })),
    });
    const out = new Map<string, { x: number; y: number }>();
    res.children?.forEach((c) => out.set(c.id, { x: c.x ?? 0, y: c.y ?? 0 }));
    return out;
  } catch {
    return new Map();
  }
}

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Longest-path layering that tolerates cycles (bounded relaxation). */
export function layerize(
  ids: string[],
  edges: { from: string; to: string }[],
): Map<string, number> {
  const layer = new Map<string, number>(ids.map((i) => [i, 0]));
  const valid = edges.filter((e) => e.from !== e.to && layer.has(e.from) && layer.has(e.to));
  for (let pass = 0; pass < Math.min(ids.length, 12); pass++) {
    let changed = false;
    for (const e of valid) {
      const next = (layer.get(e.from) ?? 0) + 1;
      if (next > (layer.get(e.to) ?? 0) && next <= ids.length) {
        layer.set(e.to, next);
        changed = true;
      }
    }
    if (!changed) break;
  }
  return layer;
}

/** Squarified treemap. Items must be sorted by value descending. */
export function squarify<T>(
  items: { value: number; data: T }[],
  box: Rect,
): { rect: Rect; data: T }[] {
  const out: { rect: Rect; data: T }[] = [];
  const total = items.reduce((s, i) => s + Math.max(i.value, 1), 0);
  if (total <= 0 || box.w <= 0 || box.h <= 0) return out;
  const scale = (box.w * box.h) / total;
  const nodes = items.map((i) => ({ area: Math.max(i.value, 1) * scale, data: i.data }));

  let { x, y, w, h } = box;
  let row: typeof nodes = [];

  const worst = (r: typeof nodes, side: number) => {
    const s = r.reduce((a, n) => a + n.area, 0);
    const max = Math.max(...r.map((n) => n.area));
    const min = Math.min(...r.map((n) => n.area));
    return Math.max((side * side * max) / (s * s), (s * s) / (side * side * min));
  };

  const flush = () => {
    if (row.length === 0) return;
    const s = row.reduce((a, n) => a + n.area, 0);
    if (w >= h) {
      const rw = s / h;
      let cy = y;
      for (const n of row) {
        const rh = n.area / rw;
        out.push({ rect: { x, y: cy, w: rw, h: rh }, data: n.data });
        cy += rh;
      }
      x += rw;
      w -= rw;
    } else {
      const rh = s / w;
      let cx = x;
      for (const n of row) {
        const rw = n.area / rh;
        out.push({ rect: { x: cx, y, w: rw, h: rh }, data: n.data });
        cx += rw;
      }
      y += rh;
      h -= rh;
    }
    row = [];
  };

  for (const n of nodes) {
    const side = Math.min(w, h);
    if (row.length === 0 || worst([...row, n], side) <= worst(row, side)) {
      row.push(n);
    } else {
      flush();
      row.push(n);
    }
  }
  flush();
  return out;
}

export function hashColor(key: string, s = 55, l = 45): string {
  let h = 0;
  for (let i = 0; i < key.length; i++) h = (h * 31 + key.charCodeAt(i)) >>> 0;
  return `hsl(${h % 360} ${s}% ${l}%)`;
}

/** churn ratio 0..1 -> cool grey to hot orange/red. */
export function churnColor(ratio: number): string {
  const r = Math.max(0, Math.min(1, ratio));
  const hue = 210 - r * 200;
  const sat = 15 + r * 65;
  const light = 32 + r * 14;
  return `hsl(${hue} ${sat}% ${light}%)`;
}
