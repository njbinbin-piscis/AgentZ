/** `.vsix` ingestion IPC + file picker. */
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

export interface VsixTheme {
  label: string;
  ui_theme: string;
  content: string;
}

export interface VsixSnippetSet {
  language: string;
  content: string;
}

export interface VsixManifest {
  name: string;
  display_name: string;
  publisher: string;
  version: string;
  themes: VsixTheme[];
  snippets: VsixSnippetSet[];
  languages: string[];
}

export function importVsix(path: string): Promise<VsixManifest> {
  return invoke<VsixManifest>("import_vsix", { path });
}

// ── Full extension install + management (runs extension JS via the host) ──────

export interface InstalledExtension {
  id: string;
  name: string;
  publisher: string;
  version: string;
  display_name: string;
  description: string;
  main: string | null;
  extension_path: string;
  activation_events: string[];
  contributes: unknown;
  enabled: boolean;
}

export function vsixInstall(path: string): Promise<InstalledExtension> {
  return invoke<InstalledExtension>("vsix_install", { path });
}

export function vsixInstallFromUrl(url: string): Promise<InstalledExtension> {
  return invoke<InstalledExtension>("vsix_install_from_url", { url });
}

export function vsixList(): Promise<InstalledExtension[]> {
  return invoke<InstalledExtension[]>("vsix_list");
}

export function vsixUninstall(id: string): Promise<void> {
  return invoke<void>("vsix_uninstall", { id });
}

export function vsixSetEnabled(id: string, enabled: boolean): Promise<void> {
  return invoke<void>("vsix_set_enabled", { id, enabled });
}

// ── Open VSX marketplace (https://open-vsx.org) ──────────────────────────────

export interface OpenVsxResult {
  namespace: string;
  name: string;
  version: string;
  displayName?: string;
  description?: string;
  downloadCount?: number;
  files?: { download?: string };
}

/** Search the Open VSX registry. */
export async function openVsxSearch(query: string, size = 15): Promise<OpenVsxResult[]> {
  const url = `https://open-vsx.org/api/-/search?query=${encodeURIComponent(query)}&size=${size}&sortBy=relevance`;
  const res = await fetch(url);
  if (!res.ok) throw new Error(`Open VSX search failed: ${res.status}`);
  const data = (await res.json()) as { extensions?: OpenVsxResult[] };
  return data.extensions ?? [];
}

/** Resolve the `.vsix` download URL for an Open VSX extension version. */
export async function openVsxDownloadUrl(namespace: string, name: string, version?: string): Promise<string> {
  const dl = (await openVsxMeta(namespace, name, version)).download;
  if (!dl) throw new Error("no download URL in Open VSX metadata");
  return dl;
}

export interface OpenVsxMeta {
  download?: string;
  /** `engines.vscode` range declared by the extension (e.g. "^1.75.0"). */
  enginesVscode?: string;
}

/** Fetch an Open VSX extension's full metadata (download URL + engines). */
export async function openVsxMeta(namespace: string, name: string, version?: string): Promise<OpenVsxMeta> {
  const base = `https://open-vsx.org/api/${namespace}/${name}${version ? `/${version}` : ""}`;
  const res = await fetch(base);
  if (!res.ok) throw new Error(`Open VSX metadata failed: ${res.status}`);
  const data = (await res.json()) as { files?: { download?: string }; engines?: Record<string, string> };
  return { download: data.files?.download, enginesVscode: data.engines?.vscode };
}

/** List published versions newest-first (Open VSX `/versions`). */
export async function openVsxVersions(namespace: string, name: string): Promise<string[]> {
  const res = await fetch(`https://open-vsx.org/api/${namespace}/${name}/versions`);
  if (!res.ok) throw new Error(`Open VSX versions failed: ${res.status}`);
  const data = (await res.json()) as { versions?: Record<string, string> };
  return Object.keys(data.versions ?? {});
}

export interface OpenVsxPick {
  version: string;
  download?: string;
  enginesVscode?: string;
  /** True when engines.vscode is satisfied by `hostVersion`. */
  compatible: boolean;
  /** True when we fell back from the marketplace's advertised latest. */
  fellBack: boolean;
}

/**
 * Prefer the newest Open VSX version whose engines.vscode satisfies the host.
 * If none do, return the latest anyway (soft-compat install).
 */
export async function openVsxPickCompatible(
  namespace: string,
  name: string,
  hostVersion: string,
  preferredVersion?: string,
  satisfiesFn: (version: string, range: string) => boolean = () => true,
  maxProbe = 12,
): Promise<OpenVsxPick> {
  const versions = preferredVersion
    ? [preferredVersion, ...(await openVsxVersions(namespace, name).catch(() => [] as string[])).filter((v) => v !== preferredVersion)]
    : await openVsxVersions(namespace, name);
  if (versions.length === 0) {
    const meta = await openVsxMeta(namespace, name, preferredVersion);
    const range = meta.enginesVscode;
    const ok = !range || range === "*" || satisfiesFn(hostVersion, range);
    return {
      version: preferredVersion ?? "latest",
      download: meta.download,
      enginesVscode: range,
      compatible: ok,
      fellBack: false,
    };
  }
  let latestMeta: OpenVsxMeta | undefined;
  for (const version of versions.slice(0, maxProbe)) {
    const meta = await openVsxMeta(namespace, name, version);
    if (!latestMeta) latestMeta = meta;
    const range = meta.enginesVscode;
    if (!range || range === "*" || satisfiesFn(hostVersion, range)) {
      return {
        version,
        download: meta.download,
        enginesVscode: range,
        compatible: true,
        fellBack: preferredVersion ? version !== preferredVersion : version !== versions[0],
      };
    }
  }
  const fallbackVersion = versions[0];
  const meta = latestMeta ?? (await openVsxMeta(namespace, name, fallbackVersion));
  return {
    version: fallbackVersion,
    download: meta.download,
    enginesVscode: meta.enginesVscode,
    compatible: false,
    fellBack: false,
  };
}

/** Prompt for a `.vsix` file. Returns null if cancelled. */
export async function openVsixDialog(): Promise<string | null> {
  const selected = await open({
    multiple: false,
    filters: [{ name: "VS Code extension", extensions: ["vsix"] }],
  });
  if (typeof selected === "string") return selected;
  return null;
}
