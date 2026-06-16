import { invoke } from "@tauri-apps/api/core";

export type SkillRegistry = "clawhub" | "skillhub";

export interface ClawHubSkill {
  slug: string;
  name: string;
  description: string;
  version: string;
  stars: number;
  tags: string[];
}

export interface ClawHubSearchResult {
  items: ClawHubSkill[];
  total: number;
  query: string;
}

export interface ClawHubInstallResult {
  slug: string;
  name: string;
  skill_dir: string;
}

export const clawHubApi = {
  search: (query: string, limit?: number, registry: SkillRegistry = "clawhub") =>
    invoke<ClawHubSearchResult>("clawhub_search", { query, limit, registry }),

  install: (slug: string, version?: string, registry: SkillRegistry = "clawhub") =>
    invoke<ClawHubInstallResult>("clawhub_install", {
      slug,
      version: version ?? null,
      registry,
    }),
};
