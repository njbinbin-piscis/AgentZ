import { expect, it } from "vitest";
import type { LlmSettings } from "./settings";
import { splitSettingsSecretUpdates } from "./settingsSecrets";

it("keeps synthetic model keys out of the legacy settings payload", () => {
  const marker = "AZ031_SETTINGS_IPC_SYNTHETIC_SECRET";
  const form = {
    openai_api_key: marker,
    llm_providers: [{ id: "named", api_key: marker }],
    mcp_servers: [{ name: "local", command: marker, url: marker, args: [marker], env: { CUSTOM: marker }, headers: { Authorization: marker } }],
  } as unknown as LlmSettings;
  const { updates, replacements } = splitSettingsSecretUpdates(form);
  expect(JSON.stringify(updates)).not.toContain(marker);
  expect(updates.openai_api_key).toBe("");
  expect(updates.llm_providers[0].api_key).toBe("");
  expect(updates.mcp_servers[0].command).toBe("");
  expect(updates.mcp_servers[0].url).toBe("");
  expect(updates.mcp_servers[0].args).toEqual([]);
  expect(updates.mcp_servers[0].env.CUSTOM).toBe("");
  expect(updates.mcp_servers[0].headers?.Authorization).toBe("");
  expect(replacements).toEqual([
    { reference: "model:openai", value: marker },
    { reference: "provider:named", value: marker },
    { reference: "mcp:local:command", value: marker },
    { reference: "mcp:local:url", value: marker },
    { reference: "mcp:local:args", value: JSON.stringify([marker]) },
    { reference: 'mcp-env:["local","CUSTOM"]', value: marker },
    { reference: 'mcp-header:["local","Authorization"]', value: marker },
  ]);
  expect(form.openai_api_key).toBe(marker);
});
