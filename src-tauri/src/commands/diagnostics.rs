//! Diagnostic export: writes the app log with every known secret value redacted.

use piscis_kernel::store::settings::Settings;
use tauri::{AppHandle, Manager};

use crate::commands::data_scope::resolve_global_config_dir;
use crate::diagnostics::{BackgroundDiagSummary, BackgroundDiagTask};

/// Snapshot the live background-task registry for diagnostic exports.
fn background_summary(app: &AppHandle) -> BackgroundDiagSummary {
    let Some(state) = app.try_state::<crate::state::AppState>() else {
        return BackgroundDiagSummary::default();
    };
    let tasks = state.background_runs.list(None, None);
    BackgroundDiagSummary {
        running: tasks.iter().filter(|t| t.status == "running").count(),
        total: tasks.len(),
        tasks: tasks
            .into_iter()
            .map(|t| BackgroundDiagTask {
                id: t.id,
                label: t.label.chars().take(200).collect(),
                status: t.status.to_string(),
                session_id: t.session_id,
                exit_code: t.exit_code,
                elapsed_ms: t.elapsed_ms,
            })
            .collect(),
    }
}

/// Export the rolling app logs to `{config}/exports/` with secrets redacted.
/// Returns the absolute path of the exported file.
#[tauri::command]
pub async fn diagnostics_export_logs(app: AppHandle) -> Result<String, String> {
    let dir = resolve_global_config_dir(&app)?;
    let settings = Settings::load(&dir.join("config.json")).map_err(|e| e.to_string())?;
    let value = serde_json::to_value(&settings).map_err(|e| e.to_string())?;
    let background = background_summary(&app);
    let out = crate::diagnostics::export_redacted_logs_with_background(&dir, &value, background)?;
    Ok(out.display().to_string())
}

#[tauri::command]
pub async fn diagnostics_preview_logs(
    app: AppHandle,
) -> Result<crate::diagnostics::DiagnosticExportPreview, String> {
    let dir = resolve_global_config_dir(&app)?;
    let settings = Settings::load(&dir.join("config.json")).map_err(|e| e.to_string())?;
    let value = serde_json::to_value(&settings).map_err(|e| e.to_string())?;
    let background = background_summary(&app);
    crate::diagnostics::preview_redacted_logs_with_background(&dir, &value, background)
}
