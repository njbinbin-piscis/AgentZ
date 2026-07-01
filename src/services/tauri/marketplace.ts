/**
 * Marketplace IPC (Phase 4) — unified multi-source discovery + install across
 * Tools / Skills / Agents / Teams / Connectors. Mirrors `commands::marketplace`.
 */
import { invoke } from "@tauri-apps/api/core";

export type MarketCategory = "tool" | "skill" | "agent" | "team" | "connector";

export interface MarketItem {
  id: string;
  name: string;
  description: string;
  version: string;
  category: MarketCategory;
  /** "clawhub" | "local" | "builtin" | "remote" */
  source: string;
  icon: string;
  tag: string;
  stars: number;
  installed: boolean;
  authorized: boolean;
}

export function marketplaceSearch(category: MarketCategory, query: string): Promise<MarketItem[]> {
  return invoke<MarketItem[]>("marketplace_search", { category, query });
}

export function marketplaceInstall(
  category: MarketCategory,
  source: string,
  identifier: string,
  version?: string | null,
): Promise<void> {
  return invoke<void>("marketplace_install", {
    category,
    source,
    identifier,
    version: version ?? null,
  });
}

export function marketplaceUninstall(category: MarketCategory, id: string): Promise<void> {
  return invoke<void>("marketplace_uninstall", { category, id });
}

/**
 * Current official cloud marketplace base URL (resolved with full precedence:
 * user override → env → build default; dev → localhost:8137).
 */
export function getCloudBaseUrl(): Promise<string> {
  return invoke<string>("get_cloud_base_url");
}

/** Persist a user override for the cloud marketplace base URL (empty clears it). */
export function setCloudBaseUrl(url: string): Promise<void> {
  return invoke<void>("set_cloud_base_url", { url });
}
