import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import ReactFlow, {
  Background,
  Controls,
  MarkerType,
  type Edge,
  type Node,
} from "reactflow";
import "reactflow/dist/style.css";
import {
  fetchCodeMap,
  type CodeMapData,
  type CodeMapFile,
} from "../../../services/tauri/codeMap";
import { churnColor, hashColor, layerize, squarify } from "./layout";
import "./CodeMap.css";

type ViewMode = "graph" | "treemap";
type Selection = { kind: "group" | "file"; id: string } | null;

interface Props {
  projectDir: string;
  onClose: () => void;
  onOpenFile: (relPath: string) => void;
  onAskAgent: (relPath: string) => void;
}

const MAX_FOCUS_FILES = 120;

export function CodeMapPanel({ projectDir, onClose, onOpenFile, onAskAgent }: Props) {
  const { t } = useTranslation();
  const [data, setData] = useState<CodeMapData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [mode, setMode] = useState<ViewMode>("graph");
  const [focusGroup, setFocusGroup] = useState<string | null>(null);
  const [selection, setSelection] = useState<Selection>(null);
  const [query, setQuery] = useState("");
  const [onlyRelated, setOnlyRelated] = useState(false);

  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const load = async () => {
      try {
        const d = await fetchCodeMap(projectDir);
        if (cancelled) return;
        setData(d);
        setError(null);
        if (!d.ready) timer = setTimeout(() => void load(), 2000);
      } catch (e) {
        if (!cancelled) setError(String(e));
      }
    };
    void load();
    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
    };
  }, [projectDir]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const fileById = useMemo(() => {
    const m = new Map<string, CodeMapFile>();
    data?.files.forEach((f) => m.set(f.id, f));
    return m;
  }, [data]);

  const q = query.trim().toLowerCase();

  const related = useMemo(() => {
    const set = new Set<string>();
    if (!data || !selection) return set;
    set.add(selection.id);
    if (selection.kind === "group" && !focusGroup) {
      data.group_edges.forEach((e) => {
        if (e.from === selection.id) set.add(e.to);
        if (e.to === selection.id) set.add(e.from);
      });
    } else {
      data.edges.forEach((e) => {
        if (e.from === selection.id) set.add(e.to);
        if (e.to === selection.id) set.add(e.from);
      });
    }
    return set;
  }, [data, selection, focusGroup]);

  const { nodes, edges } = useMemo(() => {
    if (!data || !data.ready) return { nodes: [] as Node[], edges: [] as Edge[] };
    type Item = { id: string; label: string; sub: string; color: string; match: boolean; size: number };
    let items: Item[];
    let rawEdges: { from: string; to: string; weight: number }[];

    if (!focusGroup) {
      items = data.groups.map((g) => ({
        id: g.name,
        label: g.name,
        sub: `${g.files} ${t("codeMap.files")} · ${g.loc.toLocaleString()} LOC`,
        color: hashColor(g.layer || g.name),
        match: !q || g.name.toLowerCase().includes(q),
        size: g.files,
      }));
      rawEdges = data.group_edges;
    } else {
      const files = data.files
        .filter((f) => f.group === focusGroup)
        .sort((a, b) => b.imported_by + b.imports - (a.imported_by + a.imports))
        .slice(0, MAX_FOCUS_FILES);
      const ids = new Set(files.map((f) => f.id));
      items = files.map((f) => ({
        id: f.id,
        label: f.name,
        sub: `${f.loc.toLocaleString()} LOC · ←${f.imported_by} →${f.imports}`,
        color: hashColor(f.layer || f.group),
        match: !q || f.path.toLowerCase().includes(q),
        size: 1,
      }));
      rawEdges = data.edges
        .filter((e) => ids.has(e.from) && ids.has(e.to))
        .map((e) => ({ ...e, weight: 1 }));
    }

    if (onlyRelated && selection) {
      items = items.filter((i) => related.has(i.id));
      const keep = new Set(items.map((i) => i.id));
      rawEdges = rawEdges.filter((e) => keep.has(e.from) && keep.has(e.to));
    }

    const layer = layerize(
      items.map((i) => i.id),
      rawEdges,
    );
    const columns = new Map<number, Item[]>();
    items.forEach((i) => {
      const l = layer.get(i.id) ?? 0;
      columns.set(l, [...(columns.get(l) ?? []), i]);
    });

    const outNodes: Node[] = [];
    [...columns.entries()]
      .sort((a, b) => a[0] - b[0])
      .forEach(([l, col]) => {
        col.forEach((it, row) => {
          const isSel = selection?.id === it.id;
          const isRel = related.has(it.id);
          outNodes.push({
            id: it.id,
            position: { x: l * 280, y: row * 84 },
            data: {
              label: (
                <div className="cm-node-body">
                  <div className="cm-node-title">{it.label}</div>
                  <div className="cm-node-sub">{it.sub}</div>
                </div>
              ),
            },
            style: {
              width: 230,
              borderRadius: 8,
              border: `2px solid ${isSel ? "#fff" : it.color}`,
              background: "var(--bg-elev)",
              color: "var(--fg)",
              opacity: it.match ? (selection && !isRel ? 0.45 : 1) : 0.2,
              boxShadow: isSel ? `0 0 0 2px ${it.color}` : undefined,
            },
          });
        });
      });

    const outEdges: Edge[] = rawEdges.map((e, idx) => {
      const active = selection && (e.from === selection.id || e.to === selection.id);
      return {
        id: `${e.from}->${e.to}-${idx}`,
        source: e.from,
        target: e.to,
        markerEnd: { type: MarkerType.ArrowClosed, width: 14, height: 14 },
        style: {
          stroke: active ? "#f59e0b" : "var(--border)",
          strokeWidth: active ? 2 : Math.min(1 + Math.log2(e.weight), 4),
          opacity: selection && !active ? 0.25 : 0.9,
        },
      };
    });
    return { nodes: outNodes, edges: outEdges };
  }, [data, focusGroup, onlyRelated, q, related, selection, t]);

  const onNodeClick = useCallback(
    (_: unknown, node: Node) => {
      setSelection({ kind: focusGroup ? "file" : "group", id: node.id });
    },
    [focusGroup],
  );

  const onNodeDoubleClick = useCallback(
    (_: unknown, node: Node) => {
      if (focusGroup) {
        onOpenFile(node.id);
      } else {
        setFocusGroup(node.id);
        setSelection(null);
      }
    },
    [focusGroup, onOpenFile],
  );

  const detail = useMemo(() => {
    if (!data || !selection) return null;
    if (selection.kind === "group") {
      const g = data.groups.find((x) => x.name === selection.id);
      if (!g) return null;
      const files = data.files
        .filter((f) => f.group === g.name)
        .sort((a, b) => b.loc - a.loc)
        .slice(0, 40);
      return { type: "group" as const, group: g, files };
    }
    const f = fileById.get(selection.id);
    if (!f) return null;
    const imports = data.edges.filter((e) => e.from === f.id).map((e) => e.to);
    const importedBy = data.edges.filter((e) => e.to === f.id).map((e) => e.from);
    return { type: "file" as const, file: f, imports, importedBy };
  }, [data, selection, fileById]);

  const treemap = useMemo(() => {
    if (!data || mode !== "treemap") return [];
    const W = 1000;
    const H = 600;
    const maxChurn = Math.max(1, ...data.files.map((f) => f.churn));
    const groups = data.groups
      .filter((g) => g.loc > 0)
      .sort((a, b) => b.loc - a.loc)
      .map((g) => ({ value: g.loc, data: g }));
    const gRects = squarify(groups, { x: 0, y: 0, w: W, h: H });
    return gRects.map(({ rect, data: g }) => {
      const pad = 14;
      const inner = { x: rect.x + 2, y: rect.y + pad, w: rect.w - 4, h: rect.h - pad - 2 };
      const files = data.files
        .filter((f) => f.group === g.name && f.loc > 0)
        .sort((a, b) => b.loc - a.loc)
        .slice(0, 200)
        .map((f) => ({ value: f.loc, data: f }));
      const fRects = inner.w > 4 && inner.h > 4 ? squarify(files, inner) : [];
      return {
        group: g,
        rect,
        files: fRects.map((r) => ({ ...r, color: churnColor(r.data.churn / maxChurn) })),
      };
    });
  }, [data, mode]);

  const renderBody = () => {
    if (error) return <div className="cm-empty">{error}</div>;
    if (!data) return <div className="cm-empty">{t("codeMap.loading")}</div>;
    if (!data.ready) return <div className="cm-empty">{t("codeMap.building")}</div>;
    if (data.files.length === 0) return <div className="cm-empty">{t("codeMap.empty")}</div>;
    if (mode === "treemap") {
      return (
        <div className="cm-treemap-wrap">
          <svg viewBox="0 0 1000 600" className="cm-treemap" preserveAspectRatio="xMidYMid meet">
            {treemap.map(({ group, rect, files }) => (
              <g key={group.name}>
                <rect
                  x={rect.x}
                  y={rect.y}
                  width={rect.w}
                  height={rect.h}
                  fill="none"
                  stroke="var(--border)"
                />
                {rect.w > 60 && (
                  <text x={rect.x + 4} y={rect.y + 10} className="cm-tm-group">
                    {group.name}
                  </text>
                )}
                {files.map(({ rect: r, data: f, color }) => (
                  <rect
                    key={f.id}
                    x={r.x}
                    y={r.y}
                    width={Math.max(r.w - 0.5, 0)}
                    height={Math.max(r.h - 0.5, 0)}
                    fill={color}
                    opacity={q && !f.path.toLowerCase().includes(q) ? 0.2 : 1}
                    stroke={selection?.id === f.id ? "#fff" : "none"}
                    strokeWidth={1.5}
                    onClick={() => setSelection({ kind: "file", id: f.id })}
                    onDoubleClick={() => onOpenFile(f.path)}
                  >
                    <title>{`${f.path}\n${f.loc} LOC · ${f.churn} commits`}</title>
                  </rect>
                ))}
              </g>
            ))}
          </svg>
          <div className="cm-legend">
            <span>{t("codeMap.treemapHint")}</span>
            <span className="cm-legend-bar" />
            <span>
              {data.churn_available
                ? t("codeMap.churnLegend", { days: data.churn_days })
                : t("codeMap.churnUnavailable")}
            </span>
          </div>
        </div>
      );
    }
    return (
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodeClick={onNodeClick}
        onNodeDoubleClick={onNodeDoubleClick}
        onPaneClick={() => setSelection(null)}
        nodesDraggable={false}
        nodesConnectable={false}
        fitView
        minZoom={0.1}
        proOptions={{ hideAttribution: true }}
      >
        <Background />
        <Controls showInteractive={false} />
      </ReactFlow>
    );
  };

  return (
    <div className="cm-overlay" role="dialog" aria-label={t("codeMap.title")}>
      <div className="cm-panel">
        <div className="cm-toolbar">
          <strong>{t("codeMap.title")}</strong>
          <div className="cm-seg">
            <button className={mode === "graph" ? "active" : ""} onClick={() => setMode("graph")}>
              {t("codeMap.viewGraph")}
            </button>
            <button
              className={mode === "treemap" ? "active" : ""}
              onClick={() => setMode("treemap")}
            >
              {t("codeMap.viewTreemap")}
            </button>
          </div>
          {mode === "graph" && focusGroup && (
            <button
              className="cm-btn"
              onClick={() => {
                setFocusGroup(null);
                setSelection(null);
              }}
            >
              ← {focusGroup}
            </button>
          )}
          <input
            className="cm-search"
            placeholder={t("codeMap.search")}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          {mode === "graph" && (
            <label className="cm-check">
              <input
                type="checkbox"
                checked={onlyRelated}
                onChange={(e) => setOnlyRelated(e.target.checked)}
                disabled={!selection}
              />
              {t("codeMap.onlyRelated")}
            </label>
          )}
          <span className="cm-spacer" />
          {data?.edges_truncated && <span className="cm-note">{t("codeMap.truncated")}</span>}
          <button className="cm-btn" onClick={onClose} aria-label={t("common.close")}>
            ✕
          </button>
        </div>
        <div className="cm-main">
          <div className="cm-canvas">{renderBody()}</div>
          <aside className="cm-side">
            {!detail && (
              <div className="cm-hint">
                {mode === "graph" && !focusGroup
                  ? t("codeMap.hintGroups")
                  : t("codeMap.hintFiles")}
              </div>
            )}
            {detail?.type === "group" && (
              <>
                <h4>{detail.group.name}</h4>
                <div className="cm-meta">
                  {detail.group.files} {t("codeMap.files")} ·{" "}
                  {detail.group.loc.toLocaleString()} LOC · {detail.group.churn}{" "}
                  {t("codeMap.commits")}
                </div>
                <button className="cm-btn" onClick={() => setFocusGroup(detail.group.name)}>
                  {t("codeMap.drillDown")}
                </button>
                <ul className="cm-list">
                  {detail.files.map((f) => (
                    <li key={f.id}>
                      <button onClick={() => onOpenFile(f.path)} title={f.path}>
                        {f.name}
                      </button>
                      <span>{f.loc}</span>
                    </li>
                  ))}
                </ul>
              </>
            )}
            {detail?.type === "file" && (
              <>
                <h4 title={detail.file.path}>{detail.file.name}</h4>
                <div className="cm-meta">
                  {detail.file.path}
                  <br />
                  {detail.file.loc.toLocaleString()} LOC · {detail.file.churn}{" "}
                  {t("codeMap.commits")}
                </div>
                <div className="cm-actions">
                  <button className="cm-btn" onClick={() => onOpenFile(detail.file.path)}>
                    {t("codeMap.openInEditor")}
                  </button>
                  <button className="cm-btn" onClick={() => onAskAgent(detail.file.path)}>
                    {t("codeMap.askAgent")}
                  </button>
                </div>
                <h5>
                  {t("codeMap.imports")} ({detail.imports.length})
                </h5>
                <ul className="cm-list">
                  {detail.imports.slice(0, 60).map((id) => (
                    <li key={id}>
                      <button onClick={() => setSelection({ kind: "file", id })} title={id}>
                        {fileById.get(id)?.name ?? id}
                      </button>
                    </li>
                  ))}
                </ul>
                <h5>
                  {t("codeMap.importedBy")} ({detail.importedBy.length})
                </h5>
                <ul className="cm-list">
                  {detail.importedBy.slice(0, 60).map((id) => (
                    <li key={id}>
                      <button onClick={() => setSelection({ kind: "file", id })} title={id}>
                        {fileById.get(id)?.name ?? id}
                      </button>
                    </li>
                  ))}
                </ul>
              </>
            )}
          </aside>
        </div>
      </div>
    </div>
  );
}
