//! Backend mode query command.

use tauri::State;

use crate::backend::{BackendMode, BackendState};

/// Return the current backend mode (Embedded or Remote).
#[tauri::command]
pub async fn backend_mode(backend: State<'_, BackendState>) -> Result<BackendMode, String> {
    Ok(backend.mode())
}
