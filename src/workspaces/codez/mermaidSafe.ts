let mermaidPromise: Promise<{
  parse: (code: string, options?: { suppressErrors?: boolean }) => Promise<unknown>;
  render: (id: string, code: string) => Promise<{ svg: string }>;
}> | null = null;

/** One lazy Mermaid entry point shared by chat and file previews. */
export function loadMermaid() {
  if (!mermaidPromise) {
    mermaidPromise = import("mermaid").then(({ default: mermaid }) => {
      mermaid.initialize({ startOnLoad: false, theme: "dark", securityLevel: "strict" });
      return mermaid;
    });
  }
  return mermaidPromise;
}

const SAFE_SVG_TAGS = new Set([
  "svg", "g", "path", "rect", "circle", "ellipse", "line", "polyline", "polygon",
  "text", "tspan", "defs", "marker", "title", "desc", "use", "lineargradient",
  "radialgradient", "stop", "clippath",
]);
const SAFE_SVG_ATTRIBUTES = new Set([
  "id", "class", "style", "viewbox", "width", "height", "x", "y", "x1", "x2", "y1",
  "y2", "cx", "cy", "r", "rx", "ry", "d", "points", "fill", "fill-opacity", "stroke",
  "stroke-width", "stroke-dasharray", "stroke-linecap", "stroke-linejoin", "opacity",
  "transform", "text-anchor", "font-family", "font-size", "font-weight", "dominant-baseline",
  "marker-start", "marker-mid", "marker-end", "clip-path", "offset", "stop-color",
  "stop-opacity", "preserveaspectratio", "role", "aria-label", "aria-roledescription",
]);

/** Strip executable DOM, external resources and dangerous CSS from Mermaid SVG. */
export function sanitizeMermaidSvg(svg: string): string {
  const document = new DOMParser().parseFromString(svg, "image/svg+xml");
  const root = document.documentElement;
  if (root.nodeName.toLowerCase() !== "svg" || document.querySelector("parsererror")) return "";
  for (const element of Array.from(document.querySelectorAll("*"))) {
    const tag = element.tagName.toLowerCase();
    if (!SAFE_SVG_TAGS.has(tag)) {
      element.remove();
      continue;
    }
    for (const attribute of Array.from(element.attributes)) {
      const name = attribute.name.toLowerCase();
      const value = attribute.value.trim();
      if (!SAFE_SVG_ATTRIBUTES.has(name) && !name.startsWith("data-")) {
        element.removeAttribute(attribute.name);
      } else if (name === "style" && /(?:url\s*\(|expression\s*\(|@import)/i.test(value)) {
        element.removeAttribute(attribute.name);
      }
    }
  }
  return new XMLSerializer().serializeToString(root);
}
