// Canonical file:// URI helpers shared by the LSP client and Monaco wiring.
//
// We deliberately use Monaco's own URI implementation (the dependency-free
// `base/common/uri.js` module) rather than constructing URIs by hand. This makes
// the string produced here byte-identical to the model URI Monaco derives from
// an `<Editor path=...>` prop, so the LSP providers' URI matching is exact.
// The module has no DOM dependencies, so it also works under Node (unit tests).
import { URI } from "monaco-editor/esm/vs/base/common/uri.js";

export { URI };

/** Normalize Windows separators so `C:\a\b` and `C:/a/b` agree. */
function normalize(filePath: string): string {
  return filePath.replace(/\\/g, "/");
}

/**
 * Absolute filesystem path → canonical `file://` URI string.
 * Matches `monaco.Uri.file(path).toString()` for the same input.
 */
export function fileUriString(filePath: string): string {
  return URI.file(normalize(filePath)).toString();
}

/** Absolute filesystem path → Monaco URI value (same as `<Editor path>` yields). */
export function fileUri(filePath: string): URI {
  return URI.file(normalize(filePath));
}
