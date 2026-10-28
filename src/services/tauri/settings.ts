/** LLM settings IPC — reads/writes `config.json` via the Tauri backend. */
import { invoke } from "@tauri-apps/api/core";
import type { ModelPrice } from "../../utils/modelPricing";

export interface LlmProviderConfig {
  id: string;
  label: string;
  provider: string;
  model: string;
  api_key: string;
  base_url: string;
  max_tokens: number;
  context_window?: number;
  temperature?: number | null;
  top_p?: number | null;
  thinking?: boolean | null;
  stream?: boolean | null;
}

export interface McpServerConfig {
  name: string;
  transport: string;
  command: string;
  command_configured?: boolean;
  args: string[];
  args_configured?: boolean;
  url: string;
  url_configured?: boolean;
  env: Record<string, string>;
  headers?: Record<string, string>;
  enabled: boolean;
}

export interface LlmSettings {
  provider: string;
  model: string;
  custom_base_url: string;
  max_tokens: number;
  context_window: number;
  max_iterations: number;
  policy_mode: string;
  confirm_shell_commands?: boolean;
  confirm_file_writes?: boolean;
  always_allow_tools?: string[];
  enable_streaming: boolean;
  temperature?: number | null;
  top_p?: number | null;
  thinking?: boolean | null;
  language: string;
  vision_enabled: boolean;
  anthropic_api_key: string;
  openai_api_key: string;
  deepseek_api_key: string;
  qwen_api_key: string;
  minimax_api_key: string;
  zhipu_api_key: string;
  kimi_api_key: string;
  llm_providers: LlmProviderConfig[];
  /** User-entered list prices per provider + model (per million tokens). */
  model_prices?: ModelPrice[];
  mcp_servers: McpServerConfig[];
}

export interface SettingsResponse extends LlmSettings {
  config_dir: string;
  is_configured: boolean;
  secret_references: SecretReference[];
}

export interface SecretReference {
  reference: string;
  exists: boolean;
}

export function settingsSecretExists(reference: string): Promise<SecretReference> {
  return invoke<SecretReference>("settings_secret_exists", { reference });
}

export function settingsSecretReplace(reference: string, value: string): Promise<SecretReference> {
  return invoke<SecretReference>("settings_secret_replace", { reference, value });
}

export function settingsSecretDelete(reference: string): Promise<SecretReference> {
  return invoke<SecretReference>("settings_secret_delete", { reference });
}

export function getSettings(): Promise<SettingsResponse> {
  return invoke<SettingsResponse>("get_settings");
}

export function isConfigured(): Promise<boolean> {
  return invoke<boolean>("is_configured");
}

export function saveSettings(updates: LlmSettings): Promise<SettingsResponse> {
  return invoke<SettingsResponse>("save_settings", { updates });
}

export function saveSettingsWithSecrets(
  updates: LlmSettings,
  deletions: string[],
  replacements: { reference: string; value: string }[],
): Promise<SettingsResponse> {
  return invoke<SettingsResponse>("save_settings_with_secrets", { updates, deletions, replacements });
}

/** Get the provider id selected as the global "flash" (small/fast) model. */
export function getFlashProvider(): Promise<string | null> {
  return invoke<string | null>("flash_get");
}

/** Set (or clear with null) the global flash provider id. */
export function setFlashProvider(providerId: string | null): Promise<void> {
  return invoke<void>("flash_set", { providerId });
}
