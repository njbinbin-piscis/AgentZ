/**
 * Suppress the webview's built-in browser context menu app-wide.
 *
 * This is a desktop IDE, not a web page: the Chromium menu (Back / Reload /
 * Save as / Inspect…) leaks internals and offers actions that break the app
 * shell. Surfaces with their own right-click menus call `preventDefault()`
 * themselves (capture listeners run before this one); Monaco keeps its native
 * editor menu; anything marked `data-native-context-menu` opts out.
 *
 * Everything else gets the generic app menu (Copy / Paste / Select All, plus
 * link and image actions where relevant) instead of the browser menu.
 */
import { classifyLink, OPEN_LINK_EVENT, openExternal, type LinkTarget } from "../services/linkRouter";
import type { ContextMenuItem } from "../components/ContextMenu";
import i18n from "../i18n";

const NATIVE_SELECTOR = ".monaco-editor, [data-native-context-menu]";

export function isEditableTarget(el: Element | null): el is HTMLInputElement | HTMLTextAreaElement {
  if (!el) return false;
  if (el instanceof HTMLTextAreaElement) return !el.readOnly && !el.disabled;
  if (el instanceof HTMLInputElement) {
    const type = (el.getAttribute("type") || "text").toLowerCase();
    const textLike = ["text", "search", "url", "tel", "password", "email", "number"];
    return textLike.includes(type) && !el.readOnly && !el.disabled;
  }
  return (el as HTMLElement).isContentEditable === true;
}

function selectedText(): string {
  return window.getSelection()?.toString() ?? "";
}

function inputSelection(el: HTMLInputElement | HTMLTextAreaElement): string {
  const start = el.selectionStart ?? 0;
  const end = el.selectionEnd ?? 0;
  return el.value.slice(start, end);
}

async function writeClipboard(text: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    // Clipboard can be denied by the host; nothing sensible to fall back to.
  }
}

async function readClipboard(): Promise<string> {
  try {
    return await navigator.clipboard.readText();
  } catch {
    return "";
  }
}

function insertIntoInput(el: HTMLInputElement | HTMLTextAreaElement, text: string): void {
  if (!text) return;
  const start = el.selectionStart ?? el.value.length;
  const end = el.selectionEnd ?? el.value.length;
  el.setRangeText(text, start, end, "end");
  el.dispatchEvent(new Event("input", { bubbles: true }));
}

function routeLink(target: LinkTarget): void {
  if (target.kind === "ignore") return;
  if (target.kind === "external") {
    void openExternal(target.url).catch(() => {});
    return;
  }
  window.dispatchEvent(
    new CustomEvent(OPEN_LINK_EVENT, { detail: { mode: "global", target } }),
  );
}

/** Generic menu items for a right-click that no component handled. */
export function buildDefaultContextMenuItems(e: MouseEvent): ContextMenuItem[] {
  const t = i18n.t.bind(i18n);
  const target = e.target as Element | null;
  const items: ContextMenuItem[] = [];

  const anchor = target?.closest?.("a[href]") as HTMLAnchorElement | null;
  const image = target?.closest?.("img") as HTMLImageElement | null;

  if (anchor) {
    const link = classifyLink(anchor.getAttribute("href"));
    if (link.kind === "url" || link.kind === "external") {
      items.push({
        id: "open-link",
        label: t("contextMenu.openLink"),
        onSelect: () => routeLink(link),
      });
      items.push({
        id: "copy-link",
        label: t("contextMenu.copyLinkAddress"),
        onSelect: () => void writeClipboard(anchor.href),
      });
    } else if (link.kind === "file") {
      items.push({
        id: "open-file-link",
        label: t("contextMenu.openFile"),
        onSelect: () => routeLink(link),
      });
      items.push({
        id: "copy-file-link",
        label: t("contextMenu.copyPath"),
        onSelect: () => void writeClipboard(link.path),
      });
    }
  }

  if (image?.src) {
    const src = image.src;
    if (/^data:image\//i.test(src) || /^blob:/i.test(src)) {
      items.push({
        id: "copy-image",
        label: t("contextMenu.copyImage"),
        onSelect: () => {
          void (async () => {
            try {
              const blob = await (await fetch(src)).blob();
              const png =
                blob.type === "image/png"
                  ? blob
                  : await new Promise<Blob>((resolve, reject) => {
                      const img = new Image();
                      img.onload = () => {
                        const canvas = document.createElement("canvas");
                        canvas.width = img.naturalWidth;
                        canvas.height = img.naturalHeight;
                        canvas.getContext("2d")?.drawImage(img, 0, 0);
                        canvas.toBlob((b) => (b ? resolve(b) : reject(new Error("encode failed"))), "image/png");
                      };
                      img.onerror = () => reject(new Error("decode failed"));
                      img.src = src;
                    });
              await navigator.clipboard.write([new ClipboardItem({ "image/png": png })]);
            } catch {
              // Clipboard image write unsupported/denied — fall back to the URL.
              await writeClipboard(src);
            }
          })();
        },
      });
    } else {
      items.push({
        id: "copy-image-src",
        label: t("contextMenu.copyImageAddress"),
        onSelect: () => void writeClipboard(src),
      });
    }
  }

  const editable = isEditableTarget(target) ? target : null;
  const selection = editable && !(editable instanceof HTMLElement && editable.isContentEditable)
    ? inputSelection(editable as HTMLInputElement | HTMLTextAreaElement)
    : selectedText();

  if (items.length > 0) items.push({ id: "sep-ctx", label: "", separator: true });

  items.push({
    id: "copy",
    label: t("contextMenu.copy"),
    disabled: !selection,
    onSelect: () => void writeClipboard(selection),
  });

  if (editable) {
    if (!(editable instanceof HTMLElement && editable.isContentEditable)) {
      const field = editable as HTMLInputElement | HTMLTextAreaElement;
      items.push({
        id: "paste",
        label: t("contextMenu.paste"),
        onSelect: () => {
          void readClipboard().then((text) => insertIntoInput(field, text));
        },
      });
      items.push({
        id: "cut",
        label: t("contextMenu.cut"),
        disabled: !selection,
        onSelect: () => {
          void writeClipboard(selection).then(() => insertIntoInput(field, ""));
        },
      });
    }
    items.push({
      id: "select-all",
      label: t("contextMenu.selectAll"),
      onSelect: () => {
        if (editable instanceof HTMLInputElement || editable instanceof HTMLTextAreaElement) {
          editable.focus();
          editable.select();
        } else {
          document.execCommand?.("selectAll");
        }
      },
    });
  } else {
    items.push({
      id: "select-all",
      label: t("contextMenu.selectAll"),
      onSelect: () => {
        const root = (target?.closest?.(".agentz-markdown, .agentz-tool-trace-pre, pre, article") ??
          document.body) as HTMLElement;
        const range = document.createRange();
        range.selectNodeContents(root);
        const sel = window.getSelection();
        sel?.removeAllRanges();
        sel?.addRange(range);
      },
    });
  }

  return items;
}

export type DefaultMenuOpener = (x: number, y: number, items: ContextMenuItem[]) => void;

/** Install the suppressor. Returns an uninstall function.
 *
 *  Runs in the bubble phase on document: React component handlers (attached
 *  at the root) have already run, so a component that opened its own menu has
 *  called preventDefault and we stand down. Capture would beat React and
 *  replace every component menu with the generic one. */
export function installNativeContextMenuGuard(openDefaultMenu: DefaultMenuOpener): () => void {
  const onContextMenu = (e: MouseEvent) => {
    const target = e.target as Element | null;
    // Component menus and Monaco already handled (or own) this event.
    if (e.defaultPrevented) return;
    if (target?.closest?.(NATIVE_SELECTOR)) return;
    e.preventDefault();
    openDefaultMenu(e.clientX, e.clientY, buildDefaultContextMenuItems(e));
  };
  document.addEventListener("contextmenu", onContextMenu);
  return () => document.removeEventListener("contextmenu", onContextMenu);
}
