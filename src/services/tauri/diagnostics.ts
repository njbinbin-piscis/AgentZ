import { invoke } from "@tauri-apps/api/core";

export interface DiagnosticContextSummary {
  latest_run_id?: string | null;
  latest_task_id?: string | null;
  latest_environment_id?: string | null;
  failure_count?: number;
}

export interface DiagnosticBackgroundTask {
  id: string;
  label: string;
  status: string;
  session_id: string;
  exit_code?: number | null;
  elapsed_ms: number;
}

export interface DiagnosticExportPreview {
  schema_version: number;
  app_version: string;
  files: { name: string; bytes: number }[];
  total_bytes: number;
  redaction_ready: boolean;
  context_summary?: DiagnosticContextSummary;
  background_tasks?: {
    running: number;
    total: number;
    tasks: DiagnosticBackgroundTask[];
  };
}

export function previewDiagnosticLogs(): Promise<DiagnosticExportPreview> {
  return invoke<DiagnosticExportPreview>("diagnostics_preview_logs");
}

export function exportDiagnosticLogs(): Promise<string> {
  return invoke<string>("diagnostics_export_logs");
}
