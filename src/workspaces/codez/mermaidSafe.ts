import { getAppearanceTheme } from "../../theme";

let mermaidPromise: Promise<{
  initialize: (config: Record<string, unknown>) => void;
  parse: (code: string, options?: { suppressErrors?: boolean }) => Promise<unknown>;
  render: (id: string, code: string) => Promise<{ svg: string }>;
}> | null = null;

let lastTheme: string | null = null;

/** One lazy Mermaid entry point shared by chat and file previews. */
export async function loadMermaid() {
  if (!mermaidPromise) {
    mermaidPromise = import("mermaid").then(({ default: mermaid }) => mermaid);
  }
  const mermaid = await mermaidPromise;
  const theme =
    typeof document !== "undefined" && getAppearanceTheme() === "light" ? "default" : "dark";
  if (theme !== lastTheme) {
    mermaid.initialize({
      startOnLoad: false,
      theme,
      securityLevel: "strict",
    });
    lastTheme = theme;
  }
  return mermaid;
}

const SAFE_SVG_TAGS = new Set([
  // SVG structure & shapes
  "svg", "g", "path", "rect", "circle", "ellipse", "line", "polyline", "polygon",
  "text", "tspan", "defs", "marker", "symbol", "title", "desc", "use",
  "lineargradient", "radialgradient", "stop", "clippath", "mask", "pattern",
  "filter", "fedropshadow", "fegaussianblur", "feoffset", "femerge", "femergenode", "feblend",
  "style", "foreignobject", "switch",
  // HTML label tags used inside Mermaid <foreignObject>
  "div", "span", "p", "br", "b", "i", "strong", "em", "code", "pre",
  "ul", "ol", "li", "hr", "table", "thead", "tbody", "tr", "th", "td", "blockquote",
]);

const SAFE_SVG_ATTRIBUTES = new Set([
  "id", "class", "style", "viewbox", "width", "height", "x", "y", "x1", "x2", "y1",
  "y2", "cx", "cy", "r", "rx", "ry", "d", "points", "fill", "fill-opacity", "fill-rule",
  "stroke", "stroke-width", "stroke-dasharray", "stroke-dashoffset", "stroke-linecap",
  "stroke-linejoin", "stroke-opacity", "opacity", "transform", "text-anchor",
  "font-family", "font-size", "font-weight", "font-style", "dominant-baseline",
  "alignment-baseline", "baseline-shift", "text-decoration", "letter-spacing",
  "word-spacing", "direction", "unicode-bidi", "xml:space", "textlength", "lengthadjust",
  "dx", "dy", "marker-start", "marker-mid", "marker-end", "markerwidth", "markerheight",
  "refx", "refy", "orient", "markerunits", "clip-path", "clip-rule", "mask", "filter",
  "offset", "stop-color", "stop-opacity", "gradientunits", "gradienttransform",
  "spreadmethod", "patternunits", "patterntransform", "stddeviation", "in", "in2", "result",
  "preserveaspectratio", "xmlns", "xmlns:xlink", "xlink:href", "href",
  "role", "aria-label", "aria-roledescription", "aria-hidden", "tabindex",
  "colspan", "rowspan",
]);

/** Only allow local fragment references (`url(#id)`); reject external URLs, @import, and JS expressions. */
function hasUnsafeCssValue(css: string): boolean {
  if (/(?:expression\s*\(|@import|javascript:|vbscript:|-moz-binding)/i.test(css)) {
    return true;
  }
  const urlMatches = css.matchAll(/url\s*\(\s*(['"]?)(.*?)\1\s*\)/gi);
  for (const match of urlMatches) {
    const target = (match[2] ?? "").trim();
    if (!target.startsWith("#")) {
      return true;
    }
  }
  return false;
}

/** Strip executable DOM, external resources and dangerous CSS from Mermaid SVG while keeping styles and labels. */
export function sanitizeMermaidSvg(svg: string): string {
  if (!svg || !svg.trim()) return "";
  // Parse via text/html so unescaped `>` in Mermaid's <style> rules or `<br>` inside
  // <foreignObject> HTML labels do not trigger strict XML parsererror and blank the diagram.
  const document = new DOMParser().parseFromString(svg, "text/html");
  const root = document.querySelector("svg");
  if (!root) return "";

  for (const element of Array.from(root.querySelectorAll("*"))) {
    const tag = element.tagName.toLowerCase();
    if (!SAFE_SVG_TAGS.has(tag)) {
      element.remove();
      continue;
    }
    if (tag === "style") {
      const css = element.textContent ?? "";
      if (hasUnsafeCssValue(css)) {
        element.remove();
        continue;
      }
    }
    for (const attribute of Array.from(element.attributes)) {
      const name = attribute.name.toLowerCase();
      const value = attribute.value.trim();
      if (!SAFE_SVG_ATTRIBUTES.has(name) && !name.startsWith("data-")) {
        element.removeAttribute(attribute.name);
      } else if ((name === "href" || name === "xlink:href") && !value.startsWith("#")) {
        element.removeAttribute(attribute.name);
      } else if (name === "style" && hasUnsafeCssValue(value)) {
        element.removeAttribute(attribute.name);
      }
    }
  }
  return root.outerHTML;
}
