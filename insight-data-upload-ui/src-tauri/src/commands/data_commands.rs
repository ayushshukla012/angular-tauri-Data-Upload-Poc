use crate::{connection, NativePersonRow};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{Emitter, Manager};
use tauri::path::BaseDirectory;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeConfigShape { pub spring_boot: SpringBootConfigShape }
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpringBootConfigShape { pub base_url: String }

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RowWindow { pub rows: Vec<NativePersonRow>, pub total_count: usize, pub start: usize }

fn runtime_config_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    // Tauri 2 resource resolution is used so the same logical path works across bundle targets.
    app.path().resolve("config/app-config.json", BaseDirectory::Resource).map_err(|e| format!("Unable to resolve runtime config: {e}"))
}

#[tauri::command]
pub fn get_runtime_config(app: tauri::AppHandle) -> Result<RuntimeConfigShape, String> {
    let path = runtime_config_path(&app)?;
    let text = fs::read_to_string(&path).map_err(|e| format!("Unable to read {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("Invalid runtime config: {e}"))
}

fn normalized_filter(filter: &str) -> Option<String> {
    let trimmed = filter.trim().to_lowercase();
    if trimmed.is_empty() { None } else { Some(format!("%{}%", trimmed)) }
}

#[tauri::command]
pub fn get_row_window(app: tauri::AppHandle, dataset_id: String, start: usize, count: usize, filter: String) -> Result<RowWindow, String> {
    let conn = connection(&app)?;
    let like = normalized_filter(&filter);
    let (total_count, mut rows) = if let Some(ref value) = like {
        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM imported_rows WHERE dataset_id=?1 AND (LOWER(pan) LIKE ?2 OR LOWER(name) LIKE ?2 OR LOWER(case_id) LIKE ?2 OR LOWER(email) LIKE ?2 OR LOWER(address) LIKE ?2 OR LOWER(state) LIKE ?2 OR LOWER(serial_no) LIKE ?2)",
            rusqlite::params![dataset_id, value], |row| row.get(0)
        ).map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare(
            "SELECT serial_no,case_id,pan,name,dob_doi,mobile,email,pin_code,address,state,verification_status,information_fy,information_source_type,information_source_description,information_type,information_description,information_value,source,finding,actionable_ay,statutory_reason,verification_result_type,income_escaping_assessment_value,verification_information_value,result_description FROM imported_rows WHERE dataset_id=?1 AND (LOWER(pan) LIKE ?2 OR LOWER(name) LIKE ?2 OR LOWER(case_id) LIKE ?2 OR LOWER(email) LIKE ?2 OR LOWER(address) LIKE ?2 OR LOWER(state) LIKE ?2 OR LOWER(serial_no) LIKE ?2) ORDER BY CAST(serial_no AS INTEGER), rowid LIMIT ?3 OFFSET ?4"
        ).map_err(|e| e.to_string())?;
        let rows = stmt.query_map(rusqlite::params![dataset_id, value, count as i64, start as i64], crate::row_from_sql)
            .map_err(|e| e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e| e.to_string())?;
        (total as usize, rows)
    } else {
        let total: i64 = conn.query_row("SELECT COUNT(*) FROM imported_rows WHERE dataset_id=?1", rusqlite::params![dataset_id], |row| row.get(0)).map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare(
            "SELECT serial_no,case_id,pan,name,dob_doi,mobile,email,pin_code,address,state,verification_status,information_fy,information_source_type,information_source_description,information_type,information_description,information_value,source,finding,actionable_ay,statutory_reason,verification_result_type,income_escaping_assessment_value,verification_information_value,result_description FROM imported_rows WHERE dataset_id=?1 ORDER BY CAST(serial_no AS INTEGER), rowid LIMIT ?2 OFFSET ?3"
        ).map_err(|e| e.to_string())?;
        let rows = stmt.query_map(rusqlite::params![dataset_id, count as i64, start as i64], crate::row_from_sql)
            .map_err(|e| e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e| e.to_string())?;
        (total as usize, rows)
    };
    Ok(RowWindow { rows: std::mem::take(&mut rows), total_count, start })
}

#[tauri::command]
pub async fn stream_row_window(app: tauri::AppHandle, dataset_id: String, start: usize, count: usize, chunk_size: usize, filter: String) -> Result<(), String> {
    let chunk = chunk_size.max(1);
    let mut offset = start;
    let end = start.saturating_add(count);
    while offset < end {
        let take = (end - offset).min(chunk);
        let window = get_row_window(app.clone(), dataset_id.clone(), offset, take, filter.clone())?;
        app.emit("data-row-window-chunk", &window).map_err(|e| e.to_string())?;
        if window.rows.is_empty() { break; }
        offset = offset.saturating_add(window.rows.len());
    }
    Ok(())
}
