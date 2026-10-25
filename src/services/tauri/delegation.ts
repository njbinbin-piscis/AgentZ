import { invoke } from "@tauri-apps/api/core";

export type DelegationMode = "auto" | "research" | "off";

export interface DelegationConfig {
  mode: DelegationMode;
  max_concurrent: number;
}

export interface DelegationMetrics {
  calls: number;
  failures: number;
  sub_input_tokens: number;
  sub_output_tokens: number;
  sub_tool_calls: number;
  model_fallbacks: number;
}

export function getDelegation(): Promise<DelegationConfig> {
  return invoke<DelegationConfig>("delegation_get");
}

export function setDelegation(config: DelegationConfig): Promise<DelegationConfig> {
  return invoke<DelegationConfig>("delegation_set", { config });
}

export function getDelegationMetrics(): Promise<DelegationMetrics> {
  return invoke<DelegationMetrics>("delegation_metrics");
}
