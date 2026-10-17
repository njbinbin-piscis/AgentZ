import type { LlmSettings } from "./settings";

const GLOBAL_KEYS = [
  ["anthropic_api_key", "model:anthropic"],
  ["openai_api_key", "model:openai"],
  ["deepseek_api_key", "model:deepseek"],
  ["qwen_api_key", "model:qwen"],
  ["minimax_api_key", "model:minimax"],
  ["zhipu_api_key", "model:zhipu"],
  ["kimi_api_key", "model:kimi"],
] as const;

export interface SecretReplacement {
  reference: string;
  value: string;
}

/** Keep staged model and MCP values out of the general settings IPC payload. */
export function splitSettingsSecretUpdates(form: LlmSettings): {
  updates: LlmSettings;
  replacements: SecretReplacement[];
} {
  const replacements: SecretReplacement[] = [];
  const updates = {
    ...form,
    llm_providers: form.llm_providers.map((provider) => ({ ...provider, api_key: "" })),
    mcp_servers: form.mcp_servers.map((server) => ({
      ...server,
      command: "",
      url: "",
      args: [],
      env: Object.fromEntries(Object.keys(server.env).map((key) => [key, ""])),
      headers: Object.fromEntries(Object.keys(server.headers ?? {}).map((key) => [key, ""])),
    })),
  };
  for (const [field, reference] of GLOBAL_KEYS) {
    if (form[field]) replacements.push({ reference, value: form[field] });
    updates[field] = "";
  }
  for (const provider of form.llm_providers) {
    if (provider.api_key) replacements.push({ reference: `provider:${provider.id}`, value: provider.api_key });
  }
  for (const server of form.mcp_servers) {
    if (server.command) replacements.push({ reference: `mcp:${server.name}:command`, value: server.command });
    if (server.url) replacements.push({ reference: `mcp:${server.name}:url`, value: server.url });
    if (server.args.length) replacements.push({ reference: `mcp:${server.name}:args`, value: JSON.stringify(server.args) });
    for (const [key, value] of Object.entries(server.env)) {
      if (value) replacements.push({ reference: `mcp-env:${JSON.stringify([server.name, key])}`, value });
    }
    for (const [key, value] of Object.entries(server.headers ?? {})) {
      if (value) replacements.push({ reference: `mcp-header:${JSON.stringify([server.name, key])}`, value });
    }
  }
  return { updates, replacements };
}
