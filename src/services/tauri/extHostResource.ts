import { invoke } from "@tauri-apps/api/core";

export interface ExtHostResourceSnapshot {
  child_present: boolean;
  connected: boolean;
  pending_requests: number;
  remote_watches: number;
}

export const extHostResourceSnapshot = () =>
  invoke<ExtHostResourceSnapshot>("ext_host_resource_snapshot");
