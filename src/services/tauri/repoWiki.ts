/**
 * Repo Wiki, Incremental DeepWiki & Source-Anchored Repo Memory IPC.
 */
import { invoke } from "@tauri-apps/api/core";

export interface RepoWikiResult {
  /** Workspace-relative path the wiki/memory file was written to. */
  path: string;
  /** The generated markdown. */
  markdown: string;
}

export function generateRepoWiki(projectDir: string): Promise<RepoWikiResult> {
  return invoke<RepoWikiResult>("repo_wiki_generate", { projectDir });
}

export function generateDeepRepoWiki(projectDir: string): Promise<RepoWikiResult> {
  return invoke<RepoWikiResult>("repo_wiki_generate_deep", { projectDir });
}

export function readRepoMemory(projectDir: string): Promise<RepoWikiResult> {
  return invoke<RepoWikiResult>("repo_memory_read", { projectDir });
}
