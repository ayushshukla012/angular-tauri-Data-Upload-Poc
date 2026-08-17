#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::Manager;

const MAX_FILE_SIZE: u64 = 25 * 1024 * 1024;

#[derive(Debug, Serialize)]
struct PickedFile {
    file_name: String,
    file_path: String,
    file_size: u64,
    extension: String,
    bytes: Vec<u8>,
}

#[derive(Debug, Serialize)]
struct PickedDocument {
    file_name: String,
    file_path: String,
    file_size: u64,
    extension: String,
}

#[derive(Debug, Deserialize)]
struct StoredDraftFile {
    id: String,
    name: String,
    #[serde(default)]
    r#type: String,
    #[serde(default, rename = "lastModified")]
    last_modified: i64,
    #[serde(default, rename = "filePath")]
    file_path: Option<String>,
    #[serde(default)]
    bytes: Vec<u8>,
}

#[derive(Debug, Deserialize)]
struct SaveDraftRequest {
    draft_id: Option<String>,
    draft_json: String,
    #[serde(default)]
    files: Vec<StoredDraftFile>,
}

#[derive(Debug, Serialize)]
struct SavedDraftSummary {
    id: String,
    #[serde(rename = "referenceNumber")]
    reference_number: String,
    #[serde(rename = "updatedAt")]
    updated_at: String,
    #[serde(rename = "rowCount")]
    row_count: usize,
    step: i64,
    #[serde(rename = "filePath", skip_serializing_if = "Option::is_none")]
    file_path: Option<String>,
}

#[derive(Debug, Serialize)]
struct LoadedDraft {
    #[serde(rename = "draftJson")]
    draft_json: String,
    files: Vec<StoredDraftFileResponse>,
}

#[derive(Debug, Serialize)]
struct StoredDraftFileResponse {
    id: String,
    name: String,
    #[serde(rename = "type")]
    r#type: String,
    #[serde(rename = "lastModified")]
    last_modified: i64,
    bytes: Vec<u8>,
}

fn storage_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Unable to resolve application data directory: {e}"))?;
    fs::create_dir_all(&root).map_err(|e| format!("Unable to create application data directory: {e}"))?;
    fs::create_dir_all(root.join("drafts")).map_err(|e| format!("Unable to create drafts directory: {e}"))?;
    Ok(root)
}

fn db_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(storage_root(app)?.join("data-upload-poc.sqlite3"))
}

fn connection(app: &tauri::AppHandle) -> Result<Connection, String> {
    let path = db_path(app)?;
    let conn = Connection::open(path).map_err(|e| format!("Unable to open SQLite database: {e}"))?;
    conn.execute_batch(
        "PRAGMA busy_timeout=5000;
         PRAGMA journal_mode=WAL;
         CREATE TABLE IF NOT EXISTS app_config (
           id INTEGER PRIMARY KEY,
           backend_base_url TEXT,
           created_at TEXT NOT NULL,
           updated_at TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS upload_draft (
           id TEXT PRIMARY KEY,
           upload_type TEXT,
           local_status TEXT,
           financial_year TEXT,
           information_type TEXT,
           category TEXT,
           sensitivity TEXT,
           pan TEXT,
           tan TEXT,
           itdrein TEXT,
           remarks TEXT,
           selected_file_name TEXT,
           selected_file_path TEXT,
           selected_file_size INTEGER,
           backend_upload_id TEXT,
           backend_status TEXT,
           acknowledgement_id TEXT,
           created_at TEXT NOT NULL,
           updated_at TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS api_retry_queue (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           draft_id TEXT NOT NULL,
           operation_type TEXT,
           request_payload_json TEXT,
           retry_status TEXT,
           last_error TEXT,
           created_at TEXT NOT NULL,
           updated_at TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS saved_draft (
           id TEXT PRIMARY KEY,
           file_path TEXT NOT NULL,
           reference_number TEXT NOT NULL,
           row_count INTEGER NOT NULL,
           step INTEGER NOT NULL,
           created_at TEXT NOT NULL,
           updated_at TEXT NOT NULL
         );",
    )
    .map_err(|e| format!("Unable to initialize SQLite schema: {e}"))?;
    Ok(conn)
}

fn now_iso() -> String {
    // Stable, timezone-independent POC timestamp. The exact display format is not a business rule.
    use std::time::{SystemTime, UNIX_EPOCH};
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    seconds.to_string()
}

fn sanitize_file_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') { ch } else { '_' })
        .collect()
}

#[tauri::command]
fn pick_file() -> Result<Option<PickedFile>, String> {
    let Some(path) = rfd::FileDialog::new()
        .set_title("Select supporting document")
        .pick_file()
    else {
        return Ok(None);
    };

    let metadata = fs::metadata(&path).map_err(|error| error.to_string())?;
    if metadata.len() > MAX_FILE_SIZE {
        return Err("Maximum allowed file size is 25 MB.".to_string());
    }

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "The selected file has an invalid name.".to_string())?
        .to_string();
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_string();
    let bytes = fs::read(&path).map_err(|error| error.to_string())?;

    Ok(Some(PickedFile {
        file_name,
        file_path: path.to_string_lossy().to_string(),
        file_size: metadata.len(),
        extension,
        bytes,
    }))
}

#[tauri::command]
fn pick_supporting_documents() -> Result<Vec<PickedDocument>, String> {
    let paths = rfd::FileDialog::new()
        .set_title("Select supporting document(s)")
        .pick_files()
        .unwrap_or_default();

    let mut result = Vec::with_capacity(paths.len());
    for path in paths {
        let metadata = fs::metadata(&path).map_err(|error| error.to_string())?;
        if metadata.len() > MAX_FILE_SIZE {
            return Err(format!(
                "File '{}' exceeds the maximum allowed size of 25 MB.",
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("selected document")
            ));
        }

        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| "The selected file has an invalid name.".to_string())?
            .to_string();
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_string();

        result.push(PickedDocument {
            file_name,
            file_path: path.to_string_lossy().to_string(),
            file_size: metadata.len(),
            extension,
        });
    }

    Ok(result)
}

#[tauri::command(rename_all = "camelCase")]
fn read_file_bytes(file_path: String) -> Result<Vec<u8>, String> {
    // TODO: In production, replace unrestricted path access with an allowlisted/native file-token
    // mechanism. The POC only reads paths that the user explicitly selected for upload.
    let metadata = fs::metadata(&file_path)
        .map_err(|error| format!("Unable to access selected file: {error}"))?;
    if metadata.len() > MAX_FILE_SIZE {
        return Err("Maximum allowed file size is 25 MB.".to_string());
    }
    fs::read(&file_path).map_err(|error| format!("Unable to read selected file: {error}"))
}


#[tauri::command]
fn save_draft(app: tauri::AppHandle, request: SaveDraftRequest) -> Result<SavedDraftSummary, String> {
    let root = storage_root(&app)?;
    let conn = connection(&app)?;

    let draft: Value = serde_json::from_str(&request.draft_json)
        .map_err(|e| format!("Invalid draft JSON: {e}"))?;
    let version = draft.get("version").and_then(Value::as_i64).unwrap_or_default();
    if version != 1 {
        return Err("Unsupported draft version.".to_string());
    }
    let reference_number = draft
        .get("packet")
        .and_then(|p| p.get("referenceNumber"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let step = draft.get("step").and_then(Value::as_i64).unwrap_or(1);
    let row_count = draft
        .get("rows")
        .and_then(Value::as_array)
        .map(|rows| rows.len())
        .unwrap_or(0);
    let updated_at = draft
        .get("savedAt")
        .and_then(Value::as_str)
        .unwrap_or_else(|| "")
        .to_string();

    let draft_id = request
        .draft_id
        .filter(|id| !id.trim().is_empty())
        .or_else(|| draft.get("draftId").and_then(Value::as_str).map(ToOwned::to_owned))
        .unwrap_or_else(|| format!("draft-{}", uuid_like()));

    let dir = root.join("drafts").join(&draft_id);
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| format!("Unable to replace saved draft files: {e}"))?;
    }
    fs::create_dir_all(dir.join("documents")).map_err(|e| format!("Unable to create draft directory: {e}"))?;

    let mut draft_value = draft;
    if let Some(obj) = draft_value.as_object_mut() {
        obj.insert("draftId".to_string(), Value::String(draft_id.clone()));
    }
    let draft_json = serde_json::to_string_pretty(&draft_value).map_err(|e| e.to_string())?;
    let draft_file = dir.join("draft.json");
    fs::write(&draft_file, draft_json).map_err(|e| format!("Unable to write draft file: {e}"))?;

    let mut first_file_name: Option<String> = None;
    let mut first_file_size: Option<u64> = None;
    for file in request.files {
        let safe_name = sanitize_file_name(&file.name);
        let local_path = dir.join("documents").join(format!("{}-{}", file.id, safe_name));

        if let Some(source) = file.file_path.as_ref().filter(|value| !value.trim().is_empty()) {
            // Keep the original user-selected path in the draft JSON so reopening the draft does
            // not duplicate potentially large files locally. The actual bytes remain in the user's
            // original file and are read one document at a time only during final submission.
            if !Path::new(source).exists() {
                return Err(format!("Unable to save document reference {}: file no longer exists.", file.name));
            }
            if first_file_name.is_none() {
                first_file_name = Some(file.name.clone());
                first_file_size = fs::metadata(source).ok().map(|m| m.len());
            }
        } else {
            fs::write(&local_path, &file.bytes).map_err(|e| format!("Unable to save document {}: {e}", file.name))?;
            if first_file_name.is_none() {
                first_file_name = Some(file.name);
                first_file_size = Some(file.bytes.len() as u64);
            }
        }
    }

    let timestamp = if updated_at.is_empty() { now_iso() } else { updated_at.clone() };
    conn.execute(
        "INSERT INTO saved_draft(id,file_path,reference_number,row_count,step,created_at,updated_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7)
         ON CONFLICT(id) DO UPDATE SET file_path=excluded.file_path, reference_number=excluded.reference_number,
         row_count=excluded.row_count, step=excluded.step, updated_at=excluded.updated_at",
        params![draft_id, draft_file.to_string_lossy(), reference_number, row_count as i64, step, timestamp, timestamp],
    )
    .map_err(|e| format!("Unable to save draft index in SQLite: {e}"))?;

    conn.execute(
        "INSERT INTO upload_draft(id,upload_type,local_status,selected_file_name,selected_file_path,selected_file_size,created_at,updated_at)
         VALUES(?1,'LOCAL_DRAFT','SAVED',?2,?3,?4,?5,?6)
         ON CONFLICT(id) DO UPDATE SET local_status='SAVED', selected_file_name=excluded.selected_file_name,
         selected_file_path=excluded.selected_file_path, selected_file_size=excluded.selected_file_size, updated_at=excluded.updated_at",
        params![draft_id, first_file_name, draft_file.to_string_lossy(), first_file_size.map(|v| v as i64), timestamp, timestamp],
    )
    .map_err(|e| format!("Unable to update SQLite draft state: {e}"))?;

    Ok(SavedDraftSummary {
        id: draft_id,
        reference_number: reference_number,
        updated_at: timestamp,
        row_count,
        step,
        file_path: Some(draft_file.to_string_lossy().to_string()),
    })
}

#[tauri::command]
fn list_drafts(app: tauri::AppHandle) -> Result<Vec<SavedDraftSummary>, String> {
    let conn = connection(&app)?;
    let mut stmt = conn
        .prepare("SELECT id,reference_number,updated_at,row_count,step,file_path FROM saved_draft ORDER BY updated_at DESC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(SavedDraftSummary {
                id: row.get(0)?,
                reference_number: row.get(1)?,
                updated_at: row.get(2)?,
                row_count: row.get::<_, i64>(3)? as usize,
                step: row.get(4)?,
                file_path: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| e.to_string())?);
    }
    Ok(result)
}

#[tauri::command(rename_all = "camelCase")]
fn load_draft(app: tauri::AppHandle, draft_id: String) -> Result<Option<LoadedDraft>, String> {
    let conn = connection(&app)?;
    let draft_path: Option<String> = conn
        .query_row("SELECT file_path FROM saved_draft WHERE id=?1", params![draft_id], |row| row.get(0))
        .optional()
        .map_err(|e| e.to_string())?;
    let Some(draft_path) = draft_path else { return Ok(None); };
    let path = PathBuf::from(draft_path);
    if !path.exists() {
        return Ok(None);
    }
    let draft_json = fs::read_to_string(&path).map_err(|e| format!("Unable to read saved draft: {e}"))?;
    let documents_dir = path.parent().unwrap_or(Path::new(".")).join("documents");
    let draft_value: Value = serde_json::from_str(&draft_json).map_err(|e| format!("Invalid saved draft JSON: {e}"))?;
    let documents = draft_value
        .get("documents")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut files = Vec::new();
    for doc in documents {
        let Some(id) = doc.get("id").and_then(Value::as_str) else { continue; };
        let Some(name) = doc.get("fileName").and_then(Value::as_str) else { continue; };
        if let Some(source_path) = doc.get("filePath").and_then(Value::as_str) {
            if Path::new(source_path).exists() {
                files.push(StoredDraftFileResponse {
                    id: id.to_string(),
                    name: name.to_string(),
                    r#type: doc.get("fileType").and_then(Value::as_str).unwrap_or("").to_string(),
                    last_modified: doc.get("lastModified").and_then(Value::as_i64).unwrap_or(0),
                    bytes: Vec::new(),
                });
                continue;
            }
        }
        let mut candidates = Vec::new();
        if documents_dir.exists() {
            for entry in fs::read_dir(&documents_dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let file_name = entry.file_name().to_string_lossy().to_string();
                if file_name.starts_with(&format!("{}-", id)) {
                    candidates.push(entry.path());
                }
            }
        }
        if let Some(file_path) = candidates.first() {
            let bytes = fs::read(file_path).map_err(|e| format!("Unable to read saved document {name}: {e}"))?;
            files.push(StoredDraftFileResponse {
                id: id.to_string(),
                name: name.to_string(),
                r#type: String::new(),
                last_modified: 0,
                bytes,
            });
        }
    }

    Ok(Some(LoadedDraft { draft_json, files }))
}

#[tauri::command(rename_all = "camelCase")]
fn delete_draft(app: tauri::AppHandle, draft_id: String) -> Result<(), String> {
    let conn = connection(&app)?;
    let draft_path: Option<String> = conn
        .query_row("SELECT file_path FROM saved_draft WHERE id=?1", params![draft_id], |row| row.get(0))
        .optional()
        .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM saved_draft WHERE id=?1", params![draft_id]).map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM upload_draft WHERE id=?1", params![draft_id]).map_err(|e| e.to_string())?;
    if let Some(path) = draft_path {
        if let Some(parent) = PathBuf::from(path).parent() {
            if parent.exists() {
                fs::remove_dir_all(parent).map_err(|e| format!("Unable to remove draft files: {e}"))?;
            }
        }
    }
    Ok(())
}

#[tauri::command]
fn get_storage_location(app: tauri::AppHandle) -> Result<String, String> {
    Ok(storage_root(&app)?.to_string_lossy().to_string())
}

#[tauri::command(rename_all = "camelCase")]
fn save_export_file(suggested_name: String, content: String) -> Result<Option<String>, String> {
    let Some(path) = rfd::FileDialog::new().set_file_name(&suggested_name).save_file() else {
        return Ok(None);
    };
    fs::write(&path, content).map_err(|e| format!("Unable to save file: {e}"))?;
    Ok(Some(path.to_string_lossy().to_string()))
}

#[tauri::command]
fn cleanup_temp_files(app: tauri::AppHandle) -> Result<(), String> {
    let root = storage_root(&app)?;
    let temp = root.join("temp");
    if temp.exists() {
        fs::remove_dir_all(&temp).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn uuid_like() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    format!("{:x}", nanos)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            pick_file,
            pick_supporting_documents,
            read_file_bytes,
            save_draft,
            list_drafts,
            load_draft,
            delete_draft,
            get_storage_location,
            save_export_file,
            cleanup_temp_files,
            get_app_version
        ])
        .run(tauri::generate_context!())
        .expect("error while running Insight Data Upload Utility");
}

trait OptionalRow<T> {
    fn optional(self) -> rusqlite::Result<Option<T>>;
}

impl<T> OptionalRow<T> for rusqlite::Result<T> {
    fn optional(self) -> rusqlite::Result<Option<T>> {
        match self {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err),
        }
    }
}
