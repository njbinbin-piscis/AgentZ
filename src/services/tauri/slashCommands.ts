import { invoke } from "@tauri-apps/api/core";

export interface SlashCommandInfo {
  id: string;
  slash_id: string;
  name: string;
  description: string;
  description_zh: string;
  argument_hint: string;
  tools: string[];
  source: string;
}

export interface SlashCommandResolveResult {
  id: string;
  slash_id: string;
  display: string;
  prompt: string;
  tools: string[];
}

export function listSlashCommands(): Promise<SlashCommandInfo[]> {
  return invoke<SlashCommandInfo[]>("slash_commands_list");
}

export function resolveSlashCommand(
  input: string,
  preferZh?: boolean,
): Promise<SlashCommandResolveResult | null> {
  return invoke<SlashCommandResolveResult | null>("slash_commands_resolve", {
    input,
    preferZh: preferZh ?? null,
  });
}
