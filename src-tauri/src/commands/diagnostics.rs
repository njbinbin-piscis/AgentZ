//! Diagnostic export: writes the app log with every known secret value redacted.

use piscis_kernel::store::settings::Settings;
use tauri::AppHandle;

use crate::commands::data_scope::resolve_global_config_dir;

/// Export the rolling app logs to `{config}/exports/` with secrets redacted.
/// Returns the absolute path of the exported file.
#[tauri::command]
pub async fn diagnostics_export_logs(app: AppHandle) -> Result<String, String> {
    let dir = resolve_global_config_dir(&app)?;
    let settings = Settings::load(&dir.join("config.json")).map_err(|e| e.to_string())?;
    let value = serde_json::to_value(&settings).map_err(|e| e.to_string())?;
    let out = crate::diagnostics::export_redacted_logs(&dir, &value)?;
    Ok(out.display().to_string())
}

#[tauri::command]
pub async fn diagnostics_preview_logs(
    app: AppHandle,
) -> Result<crate::diagnostics::DiagnosticExportPreview, String> {
    let dir = resolve_global_config_dir(&app)?;
    let settings = Settings::load(&dir.join("config.json")).map_err(|e| e.to_string())?;
    let value = serde_json::to_value(&settings).map_err(|e| e.to_string())?;
    crate::diagnostics::preview_redacted_logs(&dir, &value)
}
