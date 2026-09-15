#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use csv::StringRecord;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Instant;
use std::net::TcpListener;

#[cfg(target_os = "windows")]
use keyring::Entry;

static SCHEMA_INITIALIZED: AtomicBool = AtomicBool::new(false);
static SAVE_DRAFT_MUTEX: Mutex<()> = Mutex::new(());
use tauri::{Emitter, Manager};
use tauri::path::BaseDirectory;

mod commands;


const MAX_FILE_SIZE: u64 = 25 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredOAuthTokens {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_at: Option<i64>,
    #[serde(default)]
    token_type: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
struct OAuthCallbackPayload {
    url: String,
}

const OAUTH_CREDENTIAL_SERVICE: &str = "insight-data-upload";
const OAUTH_CREDENTIAL_USER: &str = "oauth-tokens";


#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CsvImportConfig {
    #[serde(default = "default_import_batch_rows")]
    batch_rows: usize,
    #[serde(default = "default_import_ready_rows")]
    ready_rows: usize,
    #[serde(default = "default_import_progress_rows")]
    progress_rows: usize,
    #[serde(default = "default_import_sqlite_cache_mb")]
    sqlite_cache_mb: usize,
    #[serde(default = "default_import_max_file_bytes")]
    max_file_bytes: u64,
}

fn default_import_batch_rows() -> usize { 25_000 }
fn default_import_ready_rows() -> usize { 100 }
fn default_import_progress_rows() -> usize { 10_000 }
fn default_import_sqlite_cache_mb() -> usize { 64 }
fn default_import_max_file_bytes() -> u64 { 0 }

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
#[allow(dead_code)]
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
    #[serde(rename = "rowStoreId", skip_serializing_if = "Option::is_none")]
    row_store_id: Option<String>,
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

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NativeInformationDetails {
    information_fy: String, information_source_type: String, information_source_description: String, information_type: String,
    information_description: String, information_value: String, source: String, finding: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeInformationDetailsInput {
    information_fy: String, information_source_type: String, information_source_description: String, information_type: String,
    information_description: String, information_value: String, source: String, finding: String,
}
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NativeVerificationDetails {
    actionable_ay: String, statutory_reason: String, verification_result_type: String, income_escaping_assessment_value: String, information_value: String, result_description: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeVerificationDetailsInput {
    actionable_ay: String, statutory_reason: String, verification_result_type: String, income_escaping_assessment_value: String, information_value: String, result_description: String,
}
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NativePersonRow {
    serial_no: String, case_id: String, pan: String, name: String, dob_doi: String, mobile: String, email: String, pin_code: String, address: String, state: String,
    verification_status: String, information_details: NativeInformationDetails, verification_details: NativeVerificationDetails,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativePersonRowInput {
    serial_no: String, case_id: String, pan: String, name: String, dob_doi: String, mobile: String, email: String, pin_code: String, address: String, state: String,
    verification_status: String, information_details: NativeInformationDetailsInput, verification_details: NativeVerificationDetailsInput,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CsvImportResult { imported_count: usize, total_count: usize }
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PagedRows { rows: Vec<NativePersonRow>, total_count: usize }
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PickedCsvFile { file_name: String, file_path: String, file_size: u64 }

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

fn ensure_imported_rows_columns(conn: &Connection) -> Result<(), String> {
    let mut existing = std::collections::HashSet::new();
    let mut stmt = conn.prepare("PRAGMA table_info(imported_rows)").map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1)).map_err(|e| e.to_string())?;
    for column in rows {
        existing.insert(column.map_err(|e| e.to_string())?);
    }

    // Existing installations can have an older SQLite database created by a previous POC build.
    // SQLite's CREATE TABLE IF NOT EXISTS does not alter such a table, so add only the columns
    // required by the current row-store implementation.
    let required: &[(&str, &str)] = &[
        ("dataset_id", "TEXT NOT NULL DEFAULT ''"),
        ("row_order", "INTEGER NOT NULL DEFAULT 0"),
        ("serial_no", "TEXT NOT NULL DEFAULT ''"),
        ("case_id", "TEXT NOT NULL DEFAULT ''"),
        ("pan", "TEXT NOT NULL DEFAULT ''"),
        ("name", "TEXT NOT NULL DEFAULT ''"),
        ("dob_doi", "TEXT NOT NULL DEFAULT ''"),
        ("mobile", "TEXT NOT NULL DEFAULT ''"),
        ("email", "TEXT NOT NULL DEFAULT ''"),
        ("pin_code", "TEXT NOT NULL DEFAULT ''"),
        ("address", "TEXT NOT NULL DEFAULT ''"),
        ("state", "TEXT NOT NULL DEFAULT ''"),
        ("verification_status", "TEXT NOT NULL DEFAULT 'Pending'"),
        ("information_fy", "TEXT NOT NULL DEFAULT ''"),
        ("information_source_type", "TEXT NOT NULL DEFAULT ''"),
        ("information_source_description", "TEXT NOT NULL DEFAULT ''"),
        ("information_type", "TEXT NOT NULL DEFAULT ''"),
        ("information_description", "TEXT NOT NULL DEFAULT ''"),
        ("information_value", "TEXT NOT NULL DEFAULT ''"),
        ("source", "TEXT NOT NULL DEFAULT ''"),
        ("finding", "TEXT NOT NULL DEFAULT ''"),
        ("actionable_ay", "TEXT NOT NULL DEFAULT ''"),
        ("statutory_reason", "TEXT NOT NULL DEFAULT ''"),
        ("verification_result_type", "TEXT NOT NULL DEFAULT ''"),
        ("income_escaping_assessment_value", "TEXT NOT NULL DEFAULT ''"),
        ("verification_information_value", "TEXT NOT NULL DEFAULT ''"),
        ("result_description", "TEXT NOT NULL DEFAULT ''"),
    ];
    let mut row_order_added = false;
    for (column, definition) in required {
        if !existing.contains(*column) {
            if *column == "row_order" {
                row_order_added = true;
            }
            conn.execute(&format!("ALTER TABLE imported_rows ADD COLUMN {column} {definition}"), [])
                .map_err(|e| format!("Unable to migrate local row store column {column}: {e}"))?;
        }
    }

    if row_order_added {
        conn.execute(
            "UPDATE imported_rows SET row_order=CAST(serial_no AS INTEGER) WHERE row_order=0",
            [],
        ).map_err(|e| e.to_string())?;
    }
    // No secondary row_order index during import. SQLite rowid preserves insertion order.

    Ok(())
}

fn ensure_metadata_columns(conn: &Connection) -> Result<(), String> {
    // Older POC installations may already have these tables with a smaller schema.
    // CREATE TABLE IF NOT EXISTS does not add missing columns, so close/save must
    // explicitly migrate the metadata tables too.
    let migrations: &[(&str, &[(&str, &str)])] = &[
        ("upload_draft", &[
            ("upload_type", "TEXT"),
            ("local_status", "TEXT"),
            ("financial_year", "TEXT"),
            ("information_type", "TEXT"),
            ("category", "TEXT"),
            ("sensitivity", "TEXT"),
            ("pan", "TEXT"),
            ("tan", "TEXT"),
            ("itdrein", "TEXT"),
            ("remarks", "TEXT"),
            ("selected_file_name", "TEXT"),
            ("selected_file_path", "TEXT"),
            ("selected_file_size", "INTEGER"),
            ("backend_upload_id", "TEXT"),
            ("backend_status", "TEXT"),
            ("acknowledgement_id", "TEXT"),
            ("created_at", "TEXT"),
            ("updated_at", "TEXT"),
        ]),
        ("saved_draft", &[
            ("file_path", "TEXT"),
            ("reference_number", "TEXT"),
            ("row_count", "INTEGER"),
            ("step", "INTEGER"),
            ("created_at", "TEXT"),
            ("updated_at", "TEXT"),
            ("row_store_id", "TEXT"),
        ]),
        ("dataset_metadata", &[
            ("revision", "INTEGER NOT NULL DEFAULT 0"),
        ]),
    ];

    for (table, columns) in migrations {
        let mut existing = std::collections::HashSet::new();
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})")).map_err(|e| e.to_string())?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1)).map_err(|e| e.to_string())?;
        for column in rows {
            existing.insert(column.map_err(|e| e.to_string())?);
        }
        for (column, definition) in *columns {
            if !existing.contains(*column) {
                conn.execute(&format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"), [])
                    .map_err(|e| format!("Unable to migrate local {table} column {column}: {e}"))?;
            }
        }
    }
    Ok(())
}

/// Lightweight connection: busy timeout only. WAL is enabled by the import path and
/// persists at the database level; read connections do not re-issue journal_mode=WAL while
/// a bulk import is active. No schema migrations.
pub(crate) fn connection_fast(app: &tauri::AppHandle) -> Result<Connection, String> {
    let path = db_path(app)?;
    let conn = Connection::open(path).map_err(|e| format!("Unable to open SQLite database: {e}"))?;
    conn.execute_batch(
        "PRAGMA busy_timeout=5000;",
    )
    .map_err(|e| format!("Unable to configure SQLite connection: {e}"))?;
    Ok(conn)
}

/// Full connection: runs schema creation + migrations on the first call per app session.
/// Subsequent calls skip migrations and behave like connection_fast().
pub(crate) fn connection(app: &tauri::AppHandle) -> Result<Connection, String> {
    let conn = connection_fast(app)?;
    if SCHEMA_INITIALIZED.swap(true, Ordering::SeqCst) {
        return Ok(conn);
    }
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS app_config (
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
           updated_at TEXT NOT NULL,
           row_store_id TEXT
         );
         CREATE TABLE IF NOT EXISTS dataset_metadata (
           dataset_id TEXT PRIMARY KEY,
           row_count INTEGER NOT NULL DEFAULT 0,
           status TEXT NOT NULL DEFAULT 'READY',
           source_file_path TEXT,
           updated_at TEXT NOT NULL DEFAULT '',
           revision INTEGER NOT NULL DEFAULT 0
         );
         CREATE TABLE IF NOT EXISTS imported_rows (
           dataset_id TEXT NOT NULL,
           row_order INTEGER NOT NULL DEFAULT 0,
           serial_no TEXT NOT NULL,
           case_id TEXT NOT NULL,
           pan TEXT NOT NULL,
           name TEXT NOT NULL,
           dob_doi TEXT NOT NULL,
           mobile TEXT NOT NULL,
           email TEXT NOT NULL,
           pin_code TEXT NOT NULL,
           address TEXT NOT NULL,
           state TEXT NOT NULL,
           verification_status TEXT NOT NULL,
           information_fy TEXT NOT NULL,
           information_source_type TEXT NOT NULL,
           information_source_description TEXT NOT NULL,
           information_type TEXT NOT NULL,
           information_description TEXT NOT NULL,
           information_value TEXT NOT NULL,
           source TEXT NOT NULL,
           finding TEXT NOT NULL,
           actionable_ay TEXT NOT NULL,
           statutory_reason TEXT NOT NULL,
           verification_result_type TEXT NOT NULL,
           income_escaping_assessment_value TEXT NOT NULL,
           verification_information_value TEXT NOT NULL,
           result_description TEXT NOT NULL,
           PRIMARY KEY(dataset_id, case_id)
         );",
    )
    .map_err(|e| format!("Unable to initialize SQLite schema: {e}"))?;
    ensure_imported_rows_columns(&conn)?;
    ensure_metadata_columns(&conn)?;
    ensure_revision_triggers(&conn)?;
    Ok(conn)
}


fn ensure_revision_triggers(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TRIGGER IF NOT EXISTS trg_imported_rows_revision_insert
             AFTER INSERT ON imported_rows
             BEGIN
                 UPDATE dataset_metadata SET revision = revision + 1,
                     updated_at = CAST(strftime('%s','now') AS TEXT)
                 WHERE dataset_id = NEW.dataset_id;
             END;
         CREATE TRIGGER IF NOT EXISTS trg_imported_rows_revision_update
             AFTER UPDATE OF
                 serial_no,case_id,pan,name,dob_doi,mobile,email,pin_code,address,state,
                 verification_status,information_fy,information_source_type,
                 information_source_description,information_type,information_description,
                 information_value,source,finding,actionable_ay,statutory_reason,
                 verification_result_type,income_escaping_assessment_value,
                 verification_information_value,result_description
             ON imported_rows
             BEGIN
                 UPDATE dataset_metadata SET revision = revision + 1,
                     updated_at = CAST(strftime('%s','now') AS TEXT)
                 WHERE dataset_id = NEW.dataset_id;
             END;
         CREATE TRIGGER IF NOT EXISTS trg_imported_rows_revision_delete
             AFTER DELETE ON imported_rows
             BEGIN
                 UPDATE dataset_metadata SET revision = revision + 1,
                     updated_at = CAST(strftime('%s','now') AS TEXT)
                 WHERE dataset_id = OLD.dataset_id;
             END;"
    ).map_err(|e| format!("Unable to initialize row-store revision triggers: {e}"))
}

fn drop_revision_triggers(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "DROP TRIGGER IF EXISTS trg_imported_rows_revision_insert;
         DROP TRIGGER IF EXISTS trg_imported_rows_revision_update;
         DROP TRIGGER IF EXISTS trg_imported_rows_revision_delete;"
    ).map_err(|e| format!("Unable to disable row-store revision triggers: {e}"))
}

fn row_store_revision(conn: &Connection, dataset_id: &str) -> Result<i64, String> {
    conn.query_row(
        "SELECT COALESCE(revision,0) FROM dataset_metadata WHERE dataset_id=?1",
        params![dataset_id],
        |row| row.get(0),
    ).optional()
    .map(|value| value.unwrap_or(0))
    .map_err(|e| format!("Unable to read row-store revision: {e}"))
}

struct RevisionTriggerGuard<'a> {
    conn: &'a Connection,
}

impl<'a> Drop for RevisionTriggerGuard<'a> {
    fn drop(&mut self) {
        let _ = ensure_revision_triggers(self.conn);
    }
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
async fn pick_file() -> Result<Option<PickedFile>, String> {
    let Some(handle) = rfd::AsyncFileDialog::new()
        .set_title("Select supporting document")
        .pick_file()
        .await
    else {
        return Ok(None);
    };
    let path = handle.path().to_path_buf();

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
async fn pick_supporting_documents() -> Result<Vec<PickedDocument>, String> {
    let handles = rfd::AsyncFileDialog::new()
        .set_title("Select supporting document(s)")
        .pick_files()
        .await
        .unwrap_or_default();
    let paths: Vec<std::path::PathBuf> = handles
        .iter()
        .map(|handle| handle.path().to_path_buf())
        .collect();

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


fn ensure_dataset_metadata_row(conn: &Connection, dataset_id: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO dataset_metadata(dataset_id,row_count,status,source_file_path,updated_at,revision)
         VALUES(?1,0,'READY',NULL,?2,0)
         ON CONFLICT(dataset_id) DO NOTHING",
        params![dataset_id, now_iso()],
    ).map_err(|e| format!("Unable to initialize row-store metadata: {e}"))?;
    Ok(())
}

fn clear_dataset(conn: &Connection, dataset_id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM imported_rows WHERE dataset_id=?1", params![dataset_id]).map_err(|e| e.to_string())?;
    Ok(())
}

const ROW_SELECT: &str = "SELECT serial_no,case_id,pan,name,dob_doi,mobile,email,pin_code,address,state,verification_status,information_fy,information_source_type,information_source_description,information_type,information_description,information_value,source,finding,actionable_ay,statutory_reason,verification_result_type,income_escaping_assessment_value,verification_information_value,result_description FROM imported_rows";

pub(crate) fn row_from_sql(row: &rusqlite::Row<'_>) -> rusqlite::Result<NativePersonRow> {
    Ok(NativePersonRow {
        serial_no: row.get(0)?, case_id: row.get(1)?, pan: row.get(2)?, name: row.get(3)?, dob_doi: row.get(4)?, mobile: row.get(5)?, email: row.get(6)?, pin_code: row.get(7)?, address: row.get(8)?, state: row.get(9)?, verification_status: row.get(10)?,
        information_details: NativeInformationDetails { information_fy: row.get(11)?, information_source_type: row.get(12)?, information_source_description: row.get(13)?, information_type: row.get(14)?, information_description: row.get(15)?, information_value: row.get(16)?, source: row.get(17)?, finding: row.get(18)? },
        verification_details: NativeVerificationDetails { actionable_ay: row.get(19)?, statutory_reason: row.get(20)?, verification_result_type: row.get(21)?, income_escaping_assessment_value: row.get(22)?, information_value: row.get(23)?, result_description: row.get(24)? },
    })
}

fn execute_upsert(conn: &Connection, dataset_id: &str, row: &NativePersonRowInput) -> Result<(), String> {
    let row_order = row.serial_no.parse::<i64>().unwrap_or(0);
    let changed = conn
        .execute(
            "UPDATE imported_rows SET
                row_order=?1, serial_no=?2, pan=?3, name=?4, dob_doi=?5, mobile=?6, email=?7,
                pin_code=?8, address=?9, state=?10, verification_status=?11, information_fy=?12,
                information_source_type=?13, information_source_description=?14, information_type=?15,
                information_description=?16, information_value=?17, source=?18, finding=?19, actionable_ay=?20,
                statutory_reason=?21, verification_result_type=?22, income_escaping_assessment_value=?23,
                verification_information_value=?24, result_description=?25
             WHERE dataset_id=?26 AND case_id=?27",
            params![
                row_order,row.serial_no,row.pan,row.name,row.dob_doi,row.mobile,row.email,row.pin_code,
                row.address,row.state,row.verification_status,row.information_details.information_fy,
                row.information_details.information_source_type,row.information_details.information_source_description,
                row.information_details.information_type,row.information_details.information_description,
                row.information_details.information_value,row.information_details.source,row.information_details.finding,
                row.verification_details.actionable_ay,row.verification_details.statutory_reason,
                row.verification_details.verification_result_type,row.verification_details.income_escaping_assessment_value,
                row.verification_details.information_value,row.verification_details.result_description,
                dataset_id,row.case_id
            ],
        )
        .map_err(|e| format!("Unable to update row: {e}"))?;

    if changed == 0 {
        conn.execute(
            "INSERT INTO imported_rows(dataset_id,row_order,serial_no,case_id,pan,name,dob_doi,mobile,email,pin_code,address,state,verification_status,information_fy,information_source_type,information_source_description,information_type,information_description,information_value,source,finding,actionable_ay,statutory_reason,verification_result_type,income_escaping_assessment_value,verification_information_value,result_description) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27)",
            params![dataset_id,row_order,row.serial_no,row.case_id,row.pan,row.name,row.dob_doi,row.mobile,row.email,row.pin_code,row.address,row.state,row.verification_status,row.information_details.information_fy,row.information_details.information_source_type,row.information_details.information_source_description,row.information_details.information_type,row.information_details.information_description,row.information_details.information_value,row.information_details.source,row.information_details.finding,row.verification_details.actionable_ay,row.verification_details.statutory_reason,row.verification_details.verification_result_type,row.verification_details.income_escaping_assessment_value,row.verification_details.information_value,row.verification_details.result_description],
        )
        .map_err(|e| format!("Unable to insert row: {e}"))?;
    }
    Ok(())
}

fn execute_upsert_tx(tx: &rusqlite::Transaction<'_>, dataset_id: &str, row: &NativePersonRowInput) -> Result<(), String> {
    let row_order = row.serial_no.parse::<i64>().unwrap_or(0);
    let changed = tx
        .execute(
            "UPDATE imported_rows SET
                row_order=?1, serial_no=?2, pan=?3, name=?4, dob_doi=?5, mobile=?6, email=?7,
                pin_code=?8, address=?9, state=?10, verification_status=?11, information_fy=?12,
                information_source_type=?13, information_source_description=?14, information_type=?15,
                information_description=?16, information_value=?17, source=?18, finding=?19, actionable_ay=?20,
                statutory_reason=?21, verification_result_type=?22, income_escaping_assessment_value=?23,
                verification_information_value=?24, result_description=?25
             WHERE dataset_id=?26 AND case_id=?27",
            params![
                row_order,row.serial_no,row.pan,row.name,row.dob_doi,row.mobile,row.email,row.pin_code,
                row.address,row.state,row.verification_status,row.information_details.information_fy,
                row.information_details.information_source_type,row.information_details.information_source_description,
                row.information_details.information_type,row.information_details.information_description,
                row.information_details.information_value,row.information_details.source,row.information_details.finding,
                row.verification_details.actionable_ay,row.verification_details.statutory_reason,
                row.verification_details.verification_result_type,row.verification_details.income_escaping_assessment_value,
                row.verification_details.information_value,row.verification_details.result_description,
                dataset_id,row.case_id
            ],
        )
        .map_err(|e| format!("Unable to update row: {e}"))?;

    if changed == 0 {
        tx.execute(
            "INSERT INTO imported_rows(dataset_id,row_order,serial_no,case_id,pan,name,dob_doi,mobile,email,pin_code,address,state,verification_status,information_fy,information_source_type,information_source_description,information_type,information_description,information_value,source,finding,actionable_ay,statutory_reason,verification_result_type,income_escaping_assessment_value,verification_information_value,result_description) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27)",
            params![dataset_id,row_order,row.serial_no,row.case_id,row.pan,row.name,row.dob_doi,row.mobile,row.email,row.pin_code,row.address,row.state,row.verification_status,row.information_details.information_fy,row.information_details.information_source_type,row.information_details.information_source_description,row.information_details.information_type,row.information_details.information_description,row.information_details.information_value,row.information_details.source,row.information_details.finding,row.verification_details.actionable_ay,row.verification_details.statutory_reason,row.verification_details.verification_result_type,row.verification_details.income_escaping_assessment_value,row.verification_details.information_value,row.verification_details.result_description],
        )
        .map_err(|e| format!("Unable to insert row: {e}"))?;
    }
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
async fn pick_csv_file() -> Result<Option<PickedCsvFile>, String> {
    let Some(handle) = rfd::AsyncFileDialog::new()
        .set_title("Select CSV file")
        .add_filter("CSV files", &["csv"])
        .pick_file()
        .await
    else { return Ok(None); };
    let path = handle.path().to_path_buf();
    let metadata = fs::metadata(&path).map_err(|e| format!("Unable to access selected CSV: {e}"))?;
    let file_name = path.file_name().and_then(|n| n.to_str()).ok_or_else(|| "The selected CSV has an invalid name.".to_string())?.to_string();
    Ok(Some(PickedCsvFile { file_name, file_path: path.to_string_lossy().to_string(), file_size: metadata.len() }))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CsvImportProgress {
    dataset_id: String,
    imported_count: usize,
    elapsed_ms: u128,
    rows_per_second: u64,
    ready: bool,
    completed: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CsvImportError {
    dataset_id: String,
    message: String,
}

fn emit_csv_progress(app: &tauri::AppHandle, progress: &CsvImportProgress) {
    println!("[RUST] emit_csv_progress: dataset_id='{}', imported_count={}, ready={}, completed={}", progress.dataset_id, progress.imported_count, progress.ready, progress.completed);
    let _ = app.emit("csv-import-progress", progress);
}

fn emit_csv_error(app: &tauri::AppHandle, dataset_id: String, message: String) {
    println!("[RUST] emit_csv_error: dataset_id='{}', message='{}'", dataset_id, message);
    let _ = app.emit(
        "csv-import-error",
        CsvImportError { dataset_id, message },
    );
}

fn optimized_csv_insert_sql() -> &'static str {
    "INSERT INTO imported_rows(
        dataset_id,row_order,serial_no,case_id,pan,name,dob_doi,mobile,email,pin_code,address,state,
        verification_status,information_fy,information_source_type,information_source_description,
        information_type,information_description,information_value,source,finding,actionable_ay,
        statutory_reason,verification_result_type,income_escaping_assessment_value,
        verification_information_value,result_description
    ) VALUES(
        ?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,
        ?22,?23,?24,?25,?26,?27
    )"
}

#[inline(always)]
fn trim_csv(value: Option<&str>) -> &str {
    value.unwrap_or("").trim()
}

/// Reintroduces the CSV validation that existed in the original implementation,
/// but keeps the hot path allocation-free and regex-free. The checks are deliberately
/// performed in Rust while the CSV record is already in memory, so Angular never has
/// to parse or validate the 500K+ rows.
#[inline(always)]
fn validate_csv_fast(record: &StringRecord, row_number: usize) -> Result<(), String> {
    if record.len() != 19 {
        return Err(format!(
            "Invalid CSV data at row {row_number}: expected 19 columns but found {}.",
            record.len()
        ));
    }

    // Same required-field rules as the previous CSV importer. Description (column 14)
    // was intentionally optional in the previous implementation and remains optional.
    const REQUIRED_FIELDS: &[(usize, &str)] = &[
        (0, "PAN"),
        (1, "Name"),
        (2, "DOB/DOI"),
        (3, "Mobile"),
        (4, "E-Mail"),
        (5, "PIN Code"),
        (6, "Address"),
        (7, "State"),
        (8, "FY"),
        (9, "Information Type"),
        (10, "Findings"),
        (11, "Source"),
        (12, "Information Value"),
        (14, "Actionable AY"),
        (15, "Verification Result Type"),
        (16, "Statutory Reason"),
        (17, "Income Escaping Assessment Value"),
        (18, "Verification Information Value"),
    ];

    for &(index, field) in REQUIRED_FIELDS {
        if trim_csv(record.get(index)).is_empty() {
            return Err(format!(
                "Invalid CSV data at row {row_number}: {field} is required."
            ));
        }
    }

    let pan = trim_csv(record.get(0)).as_bytes();
    let valid_pan = pan.len() == 10
        && pan[0].is_ascii_alphabetic()
        && pan[1].is_ascii_alphabetic()
        && pan[2].is_ascii_alphabetic()
        && pan[3].is_ascii_alphabetic()
        && pan[4].is_ascii_alphabetic()
        && pan[5].is_ascii_digit()
        && pan[6].is_ascii_digit()
        && pan[7].is_ascii_digit()
        && pan[8].is_ascii_digit()
        && pan[9].is_ascii_alphabetic();

    if !valid_pan {
        return Err(format!("Invalid PAN at CSV row {row_number}."));
    }

    let mobile = trim_csv(record.get(3));
    if mobile.len() != 10 || !mobile.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("Invalid Mobile at CSV row {row_number}."));
    }

    let pin_code = trim_csv(record.get(5));
    if pin_code.len() != 6 || !pin_code.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("Invalid PIN Code at CSV row {row_number}."));
    }

    let email = trim_csv(record.get(4));
    if !email.as_bytes().contains(&b'@') || !email.as_bytes().contains(&b'.') {
        return Err(format!("Invalid E-Mail at CSV row {row_number}."));
    }

    Ok(())
}

fn read_csv_import_config(app: &tauri::AppHandle) -> CsvImportConfig {
    let defaults = CsvImportConfig {
        batch_rows: default_import_batch_rows(),
        ready_rows: default_import_ready_rows(),
        progress_rows: default_import_progress_rows(),
        sqlite_cache_mb: default_import_sqlite_cache_mb(),
        max_file_bytes: default_import_max_file_bytes(),
    };

    let Ok(path) = app
        .path()
        .resolve("config/app-config.json", BaseDirectory::Resource)
    else {
        return defaults;
    };

    let Ok(text) = fs::read_to_string(path) else {
        return defaults;
    };

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Root {
        #[serde(default)]
        csv_import: Option<CsvImportConfig>,
    }

    serde_json::from_str::<Root>(&text)
        .ok()
        .and_then(|root| root.csv_import)
        .unwrap_or(defaults)
}

fn run_csv_import_blocking(
    app: tauri::AppHandle,
    dataset_id: String,
    file_path: String,
    reference_number: String,
) -> Result<CsvImportResult, String> {
    let timer = Instant::now();
    let config = read_csv_import_config(&app);
    let batch_rows = config.batch_rows.max(1);
    let progress_rows = config.progress_rows.max(1);
    let ready_rows = config.ready_rows.max(1);
    let sqlite_cache_mb = config.sqlite_cache_mb.max(16);

    if config.max_file_bytes > 0 {
        let size = fs::metadata(&file_path)
            .map_err(|e| format!("Unable to inspect CSV file '{}': {e}", file_path))?
            .len();

        if size > config.max_file_bytes {
            return Err(format!(
                "The selected CSV is {:.2} MB, above the configured maximum of {:.2} MB.",
                size as f64 / 1024.0 / 1024.0,
                config.max_file_bytes as f64 / 1024.0 / 1024.0
            ));
        }
    }

    let file = File::open(&file_path)
        .map_err(|e| format!("Unable to open CSV file '{}': {e}", file_path))?;

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        // Keep the parser strict only about CSV shape. Field values are validated below
        // without allocating or transforming the stored value.
        .from_reader(BufReader::with_capacity(32 * 1024 * 1024, file));

    let headers = reader
        .headers()
        .map_err(|e| format!("Unable to read CSV headers: {e}"))?
        .clone();

    let expected = [
        "PAN", "Name", "DOB/DOI", "Mobile", "E-Mail", "PIN Code", "Address",
        "State", "FY", "Information Type", "Findings", "Source", "Information Value",
        "Description", "Actionable AY", "Verification Result Type", "Statutory Reason",
        "Income Escaping Assessment Value", "Verification Information Value",
    ];

    if headers.len() != expected.len()
        || headers
            .iter()
            .zip(expected.iter())
            .any(|(actual, expected)| actual.trim().trim_start_matches('\u{feff}') != *expected)
    {
        return Err(
            "Invalid CSV header. The file must contain the required 19 columns in the exported order."
                .to_string(),
        );
    }

    let conn = connection(&app)?;

    // Revision triggers are useful for tamper detection after import, but firing them
    // 500K times during a bulk ingest would add avoidable write amplification.
    drop_revision_triggers(&conn)?;
    let _revision_trigger_guard = RevisionTriggerGuard { conn: &conn };

    // Drop any legacy row_order secondary index before the bulk load. Pagination uses
    // SQLite rowid insertion order, so no secondary B-tree is needed for the SLA path.
    conn.execute(
        "DROP INDEX IF EXISTS idx_imported_rows_dataset_row_order",
        [],
    )
    .map_err(|e| format!("Unable to prepare SQLite import indexes: {e}"))?;

    conn.execute_batch(&format!(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=OFF;
         PRAGMA temp_store=MEMORY;
         PRAGMA cache_size=-{};
         PRAGMA wal_autocheckpoint=0;
         PRAGMA mmap_size=268435456;",
        sqlite_cache_mb * 1024
    ))
    .map_err(|e| format!("Unable to configure SQLite import: {e}"))?;

    conn.execute(
        "DELETE FROM imported_rows WHERE dataset_id=?1",
        params![dataset_id],
    )
    .map_err(|e| format!("Unable to clear the previous dataset: {e}"))?;

    conn.execute(
        "DELETE FROM dataset_metadata WHERE dataset_id=?1",
        params![dataset_id],
    )
    .map_err(|e| format!("Unable to clear dataset metadata: {e}"))?;
    conn.execute(
        "INSERT INTO dataset_metadata(dataset_id,row_count,status,source_file_path,updated_at,revision)
         VALUES(?1,0,'IMPORTING',?2,?3,0)
         ON CONFLICT(dataset_id) DO UPDATE SET row_count=0,status='IMPORTING',
             source_file_path=excluded.source_file_path,updated_at=excluded.updated_at",
        params![dataset_id, file_path, now_iso()],
    )
    .map_err(|e| format!("Unable to initialize import metadata: {e}"))?;

    let reference = if reference_number.trim().is_empty() {
        "LOCAL"
    } else {
        reference_number.trim()
    };

    let insert_sql = optimized_csv_insert_sql();
    let mut imported = 0usize;
    let mut ready_emitted = false;
    let mut next_progress_at = ready_rows;
    let mut record = StringRecord::new();

    loop {
        let current_batch_size = if !ready_emitted {
            ready_rows
        } else {
            batch_rows.max(1)
        };

        let tx = conn
            .unchecked_transaction()
            .map_err(|e| format!("Unable to start CSV import batch: {e}"))?;

        let mut batch_inserted = 0usize;

        {
            let mut stmt = tx
                .prepare_cached(insert_sql)
                .map_err(|e| format!("Unable to prepare CSV insert statement: {e}"))?;

            while batch_inserted < current_batch_size {
                if !reader
                    .read_record(&mut record)
                    .map_err(|e| format!("Unable to read CSV row {}: {e}", imported + 2))?
                {
                    break;
                }

                // Restore the original per-row validation, but execute it natively in Rust.
                // No Angular-side CSV parsing, object mapping, regex validation or per-row
                // validation work occurs for large native imports.
                let row_number = imported + 2;
                validate_csv_fast(&record, row_number)?;
                let order = imported + 1;
                let serial_no = format!("{order:07}");
                let case_id = format!("{reference}-{serial_no}");
                let csv_value = |index: usize| record.get(index).unwrap_or("");

                stmt.execute(params![
                    dataset_id,
                    order as i64,
                    serial_no,
                    case_id,
                    csv_value(0),
                    csv_value(1),
                    csv_value(2),
                    csv_value(3),
                    csv_value(4),
                    csv_value(5),
                    csv_value(6),
                    csv_value(7),
                    "Pending",
                    csv_value(8),
                    "",
                    "",
                    csv_value(9),
                    csv_value(13),
                    csv_value(12),
                    csv_value(11),
                    csv_value(10),
                    csv_value(14),
                    csv_value(16),
                    csv_value(15),
                    csv_value(17),
                    csv_value(18),
                    "",
                ])
                .map_err(|e| format!("Unable to save CSV row {row_number}: {e}"))?;

                imported += 1;
                batch_inserted += 1;
            }
        }

        tx.commit()
            .map_err(|e| format!("Unable to commit CSV import batch: {e}"))?;

        if batch_inserted == 0 {
            break;
        }

        conn.execute(
            "INSERT INTO dataset_metadata(dataset_id,row_count,status,source_file_path,updated_at)
             VALUES(?1,?2,'IMPORTING',?3,?4)
             ON CONFLICT(dataset_id) DO UPDATE SET
                 row_count=excluded.row_count,
                 status='IMPORTING',
                 source_file_path=excluded.source_file_path,
                 updated_at=excluded.updated_at",
            params![dataset_id, imported as i64, file_path, now_iso()],
        )
        .map_err(|e| format!("Unable to update import metadata: {e}"))?;

        if !ready_emitted && imported >= ready_rows {
            // First page is available for display. The Angular workflow remains locked until
            // completed=true so user edits cannot race the ongoing import/finalization.
            ready_emitted = true;
            next_progress_at = imported.saturating_add(progress_rows);
        }

        let should_emit_progress = !ready_emitted
            || imported >= next_progress_at
            || batch_inserted < current_batch_size;

        if should_emit_progress {
            let elapsed_ms = timer.elapsed().as_millis();
            let rows_per_second =
                (imported as f64 / timer.elapsed().as_secs_f64().max(0.001)) as u64;

            emit_csv_progress(
                &app,
                &CsvImportProgress {
                    dataset_id: dataset_id.clone(),
                    imported_count: imported,
                    elapsed_ms,
                    rows_per_second,
                    ready: ready_emitted,
                    completed: false,
                },
            );

            while next_progress_at <= imported {
                next_progress_at = next_progress_at.saturating_add(progress_rows);
            }
        }
    }

    if imported == 0 {
        conn.execute(
            "DELETE FROM imported_rows WHERE dataset_id=?1",
            params![dataset_id],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM dataset_metadata WHERE dataset_id=?1",
            params![dataset_id],
        )
        .map_err(|e| e.to_string())?;
        return Err("CSV contains no data rows.".to_string());
    }


    conn.execute(
        "UPDATE dataset_metadata
         SET row_count=?2,status='READY',source_file_path=?3,updated_at=?4,
             revision=revision+1
         WHERE dataset_id=?1",
        params![dataset_id, imported as i64, file_path, now_iso()],
    )
    .map_err(|e| format!("Unable to finalize import metadata: {e}"))?;

    // Restore normal durability after the reconstructible bulk load. During the import the
    // source CSV can be used to rebuild the dataset, so synchronous=OFF removes fsync work
    // from the hot path. User edits run later with NORMAL durability.
    conn.execute_batch(
        "PRAGMA synchronous=NORMAL;"
    )
    .map_err(|e| format!("Unable to restore SQLite durability mode: {e}"))?;

    let elapsed_ms = timer.elapsed().as_millis();
    let rows_per_second =
        (imported as f64 / timer.elapsed().as_secs_f64().max(0.001)) as u64;

    emit_csv_progress(
        &app,
        &CsvImportProgress {
            dataset_id: dataset_id.clone(),
            imported_count: imported,
            elapsed_ms,
            rows_per_second,
            ready: true,
            completed: true,
        },
    );

    Ok(CsvImportResult {
        imported_count: imported,
        total_count: imported,
    })
}

#[tauri::command(rename_all = "camelCase")]
async fn import_csv_to_store(
    app: tauri::AppHandle,
    dataset_id: String,
    file_path: String,
    reference_number: String,
) -> Result<(), String> {
    println!("[RUST] import_csv_to_store: dataset_id='{}', file_path='{}', ref='{}'", dataset_id, file_path, reference_number);
    let worker_app = app.clone();

    let _ = tauri::async_runtime::spawn_blocking(move || {
        println!("[RUST] Background worker thread started.");
        let result = run_csv_import_blocking(
            worker_app.clone(),
            dataset_id.clone(),
            file_path,
            reference_number,
        );

        if let Err(message) = result {
            println!("[RUST] Background worker error: {}", message);
            emit_csv_error(&worker_app, dataset_id, message);
        } else {
            println!("[RUST] Background worker completed successfully.");
        }
    });

    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CsvImportStatus {
    imported_count: usize,
    ready: bool,
    completed: bool,
}

#[tauri::command(rename_all = "camelCase")]
fn get_csv_import_status(app: tauri::AppHandle, dataset_id: String) -> Result<CsvImportStatus, String> {
    let conn = connection_fast(&app)?;
    let result: Option<(i64, String)> = conn
        .query_row(
            "SELECT row_count, status FROM dataset_metadata WHERE dataset_id=?1",
            params![dataset_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    let Some((row_count, status)) = result else {
        return Ok(CsvImportStatus {
            imported_count: 0,
            ready: false,
            completed: false,
        });
    };

    Ok(CsvImportStatus {
        imported_count: row_count.max(0) as usize,
        ready: row_count > 0,
        completed: status == "READY",
    })
}

#[tauri::command(rename_all = "camelCase")]
async fn get_row_page(
    app: tauri::AppHandle,
    dataset_id: String,
    page: i64,
    page_size: i64,
    filter: String,
) -> Result<PagedRows, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let conn = connection_fast(&app)?;
        let page_size = page_size.clamp(1, 1000);
        let page = page.max(1);
        let offset = (page - 1) * page_size;
        let term = filter.trim().to_ascii_lowercase();
        let like = format!("%{}%", term.replace('%', "\\%").replace('_', "\\_"));

        let count: i64 = if term.is_empty() {
            conn.query_row(
                "SELECT COALESCE(
                    (SELECT row_count FROM dataset_metadata WHERE dataset_id=?1),
                    (SELECT COUNT(*) FROM imported_rows WHERE dataset_id=?1)
                 )",
                params![dataset_id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?
        } else {
            conn.query_row(
                "SELECT COUNT(*)
                 FROM imported_rows
                 WHERE dataset_id=?1
                   AND (
                     pan LIKE ?2 ESCAPE '\\'
                     OR name LIKE ?2 ESCAPE '\\'
                     OR email LIKE ?2 ESCAPE '\\'
                     OR address LIKE ?2 ESCAPE '\\'
                     OR state LIKE ?2 ESCAPE '\\'
                     OR serial_no LIKE ?2 ESCAPE '\\'
                   )",
                params![dataset_id, like],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?
        };

        // Rows are inserted sequentially and edits use UPDATE (not REPLACE), so SQLite rowid
        // preserves insertion order for this dataset. Avoiding the secondary row_order B-tree
        // removes a large bulk-load and final-index-build cost for 500K+ rows.
        let order_column = "rowid";

        let mut stmt = if term.is_empty() {
            conn.prepare(&format!(
                "{ROW_SELECT} WHERE dataset_id=?1 ORDER BY {order_column} LIMIT ?2 OFFSET ?3"
            ))
            .map_err(|e| e.to_string())?
        } else {
            conn.prepare(&format!(
                "{ROW_SELECT} WHERE dataset_id=?1
                 AND (
                     pan LIKE ?4 ESCAPE '\\'
                     OR name LIKE ?4 ESCAPE '\\'
                     OR email LIKE ?4 ESCAPE '\\'
                     OR address LIKE ?4 ESCAPE '\\'
                     OR state LIKE ?4 ESCAPE '\\'
                     OR serial_no LIKE ?4 ESCAPE '\\'
                 )
                 ORDER BY {order_column} LIMIT ?2 OFFSET ?3"
            ))
            .map_err(|e| e.to_string())?
        };

        let rows = if term.is_empty() {
            stmt.query_map(
                params![dataset_id, page_size, offset],
                row_from_sql,
            )
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
        } else {
            stmt.query_map(
                params![dataset_id, page_size, offset, like],
                row_from_sql,
            )
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
        };

        Ok(PagedRows {
            rows,
            total_count: count as usize,
        })
    }).await.map_err(|e| e.to_string())?
}
#[tauri::command(rename_all = "camelCase")]
async fn get_row_by_case_id(app: tauri::AppHandle,dataset_id:String,case_id:String)->Result<Option<NativePersonRow>,String>{
    tauri::async_runtime::spawn_blocking(move || {
        let conn=connection(&app)?;
        conn.query_row(&format!("{ROW_SELECT} WHERE dataset_id=?1 AND case_id=?2"),params![dataset_id,case_id],row_from_sql).optional().map_err(|e|e.to_string())
    }).await.map_err(|e| e.to_string())?
}
#[tauri::command(rename_all = "camelCase")]
async fn upsert_row(app: tauri::AppHandle,dataset_id:String,row:NativePersonRowInput)->Result<(),String>{
    tauri::async_runtime::spawn_blocking(move || {
        let conn=connection(&app)?;
        ensure_dataset_metadata_row(&conn, &dataset_id)?;
        execute_upsert(&conn,&dataset_id,&row)
    }).await.map_err(|e| e.to_string())?
}
fn delete_rows_blocking(app: tauri::AppHandle,dataset_id:String,case_ids:Vec<String>)->Result<(),String>{
    if case_ids.is_empty() { return Ok(()); }
    let conn=connection(&app)?;
    let tx=conn.unchecked_transaction().map_err(|e|e.to_string())?;
    for id in case_ids {
        tx.execute("DELETE FROM imported_rows WHERE dataset_id=?1 AND case_id=?2",params![dataset_id,id]).map_err(|e|e.to_string())?;
    }
    let remaining: i64 = tx.query_row("SELECT COUNT(*) FROM imported_rows WHERE dataset_id=?1", params![dataset_id], |r| r.get(0)).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e|e.to_string())?;
    conn.execute(
        "UPDATE dataset_metadata SET row_count=?2, updated_at=?3 WHERE dataset_id=?1",
        params![dataset_id, remaining.max(0), now_iso()],
    ).map_err(|e| e.to_string())?;
    Ok(())
}
fn set_rows_status_blocking(app: tauri::AppHandle,dataset_id:String,case_ids:Vec<String>,status:String)->Result<(),String>{if !matches!(status.as_str(),"Pending"|"Approved"|"Completed"){return Err("Invalid verification status.".to_string());}let conn=connection(&app)?;let tx=conn.unchecked_transaction().map_err(|e|e.to_string())?;for id in case_ids{tx.execute("UPDATE imported_rows SET verification_status=?1 WHERE dataset_id=?2 AND case_id=?3",params![status,dataset_id,id]).map_err(|e|e.to_string())?;}tx.commit().map_err(|e|e.to_string())?;Ok(())}
#[tauri::command(rename_all = "camelCase")]
async fn clear_row_store(app: tauri::AppHandle,dataset_id:String)->Result<(),String>{
    tauri::async_runtime::spawn_blocking(move || {
        let conn = connection(&app)?;
        clear_dataset(&conn,&dataset_id)
    }).await.map_err(|e| format!("Unable to schedule row-store cleanup: {e}"))?
}
fn clone_row_store_blocking(app: tauri::AppHandle,source_dataset_id:String,target_dataset_id:String)->Result<usize,String>{let conn=connection(&app)?;clear_dataset(&conn,&target_dataset_id)?;let count=conn.execute("INSERT INTO imported_rows(dataset_id,row_order,serial_no,case_id,pan,name,dob_doi,mobile,email,pin_code,address,state,verification_status,information_fy,information_source_type,information_source_description,information_type,information_description,information_value,source,finding,actionable_ay,statutory_reason,verification_result_type,income_escaping_assessment_value,verification_information_value,result_description) SELECT ?1,row_order,serial_no,case_id,pan,name,dob_doi,mobile,email,pin_code,address,state,verification_status,information_fy,information_source_type,information_source_description,information_type,information_description,information_value,source,finding,actionable_ay,statutory_reason,verification_result_type,income_escaping_assessment_value,verification_information_value,result_description FROM imported_rows WHERE dataset_id=?2",params![target_dataset_id,source_dataset_id]).map_err(|e|e.to_string())?;Ok(count)}
fn rename_row_store_blocking(app: tauri::AppHandle,source_dataset_id:String,target_dataset_id:String)->Result<usize,String>{let conn=connection(&app)?;if source_dataset_id!=target_dataset_id{clear_dataset(&conn,&target_dataset_id)?;conn.execute("UPDATE imported_rows SET dataset_id=?1 WHERE dataset_id=?2",params![target_dataset_id,source_dataset_id]).map_err(|e|e.to_string())?;}conn.query_row("SELECT COUNT(*) FROM imported_rows WHERE dataset_id=?1",params![target_dataset_id],|r|r.get::<_,i64>(0)).map(|v|v as usize).map_err(|e|e.to_string())}
fn seed_row_store_blocking(app: tauri::AppHandle,dataset_id:String,rows:Vec<NativePersonRowInput>)->Result<usize,String>{let conn=connection(&app)?;ensure_dataset_metadata_row(&conn,&dataset_id)?;clear_dataset(&conn,&dataset_id)?;let tx=conn.unchecked_transaction().map_err(|e|e.to_string())?;for row in &rows{execute_upsert_tx(&tx,&dataset_id,row)?;}tx.commit().map_err(|e|e.to_string())?;Ok(rows.len())}
#[tauri::command(rename_all = "camelCase")]
async fn delete_rows(app: tauri::AppHandle, dataset_id: String, case_ids: Vec<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || delete_rows_blocking(app, dataset_id, case_ids))
        .await
        .map_err(|e| format!("Unable to schedule row deletion: {e}"))?
}

#[tauri::command(rename_all = "camelCase")]
async fn set_rows_status(app: tauri::AppHandle, dataset_id: String, case_ids: Vec<String>, status: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || set_rows_status_blocking(app, dataset_id, case_ids, status))
        .await
        .map_err(|e| format!("Unable to schedule status update: {e}"))?
}

#[tauri::command(rename_all = "camelCase")]
async fn clone_row_store(app: tauri::AppHandle, source_dataset_id: String, target_dataset_id: String) -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(move || clone_row_store_blocking(app, source_dataset_id, target_dataset_id))
        .await
        .map_err(|e| format!("Unable to clone row store: {e}"))?
}

#[tauri::command(rename_all = "camelCase")]
async fn rename_row_store(app: tauri::AppHandle, source_dataset_id: String, target_dataset_id: String) -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(move || rename_row_store_blocking(app, source_dataset_id, target_dataset_id))
        .await
        .map_err(|e| format!("Unable to rename row store: {e}"))?
}

#[tauri::command(rename_all = "camelCase")]
async fn seed_row_store(app: tauri::AppHandle, dataset_id: String, rows: Vec<NativePersonRowInput>) -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(move || seed_row_store_blocking(app, dataset_id, rows))
        .await
        .map_err(|e| format!("Unable to seed row store: {e}"))?
}

#[tauri::command(rename_all = "camelCase")]
async fn export_csv_file(
    app: tauri::AppHandle,
    dataset_id: String,
    suggested_name: String,
) -> Result<Option<String>, String> {
    if dataset_id.trim().is_empty() {
        return Err("No row dataset is available to export.".to_string());
    }
    let default_name = if suggested_name.to_ascii_lowercase().ends_with(".csv") {
        suggested_name
    } else {
        format!("{suggested_name}.csv")
    };

    let Some(handle) = rfd::AsyncFileDialog::new()
        .set_title("Export CSV")
        .set_file_name(&default_name)
        .add_filter("CSV files", &["csv"])
        .save_file()
        .await
    else {
        return Ok(None);
    };
    let path = handle.path().to_path_buf();

    let export_path = path.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let conn = connection_fast(&app)?;
        let file = File::create(&export_path)
            .map_err(|e| format!("Unable to create CSV file '{}': {e}", export_path.display()))?;
        let mut writer = csv::WriterBuilder::new()
            .has_headers(false)
            .from_writer(BufWriter::with_capacity(4 * 1024 * 1024, file));

        writer.write_record([
            "PAN", "Name", "DOB/DOI", "Mobile", "E-Mail", "PIN Code", "Address", "State", "FY",
            "Information Type", "Findings", "Source", "Information Value", "Description", "Actionable AY",
            "Verification Result Type", "Statutory Reason", "Income Escaping Assessment Value",
            "Verification Information Value",
        ]).map_err(|e| format!("Unable to write CSV header: {e}"))?;

        let mut stmt = conn.prepare(
            "SELECT pan,name,dob_doi,mobile,email,pin_code,address,state,information_fy,information_type,
                    finding,source,information_value,information_description,actionable_ay,
                    verification_result_type,statutory_reason,income_escaping_assessment_value,
                    verification_information_value
             FROM imported_rows WHERE dataset_id=?1 ORDER BY rowid"
        ).map_err(|e| format!("Unable to prepare CSV export query: {e}"))?;

        let mut rows = stmt.query(params![dataset_id])
            .map_err(|e| format!("Unable to read rows for CSV export: {e}"))?;

        while let Some(row) = rows.next().map_err(|e| format!("Unable to read CSV export row: {e}"))? {
            writer.write_record([
                row.get::<_, String>(0).map_err(|e| e.to_string())?,
                row.get::<_, String>(1).map_err(|e| e.to_string())?,
                row.get::<_, String>(2).map_err(|e| e.to_string())?,
                row.get::<_, String>(3).map_err(|e| e.to_string())?,
                row.get::<_, String>(4).map_err(|e| e.to_string())?,
                row.get::<_, String>(5).map_err(|e| e.to_string())?,
                row.get::<_, String>(6).map_err(|e| e.to_string())?,
                row.get::<_, String>(7).map_err(|e| e.to_string())?,
                row.get::<_, String>(8).map_err(|e| e.to_string())?,
                row.get::<_, String>(9).map_err(|e| e.to_string())?,
                row.get::<_, String>(10).map_err(|e| e.to_string())?,
                row.get::<_, String>(11).map_err(|e| e.to_string())?,
                row.get::<_, String>(12).map_err(|e| e.to_string())?,
                row.get::<_, String>(13).map_err(|e| e.to_string())?,
                row.get::<_, String>(14).map_err(|e| e.to_string())?,
                row.get::<_, String>(15).map_err(|e| e.to_string())?,
                row.get::<_, String>(16).map_err(|e| e.to_string())?,
                row.get::<_, String>(17).map_err(|e| e.to_string())?,
                row.get::<_, String>(18).map_err(|e| e.to_string())?,
            ]).map_err(|e| format!("Unable to write CSV row: {e}"))?;
        }

        writer.flush().map_err(|e| format!("Unable to finalize CSV export: {e}"))?;
        Ok(())
    }).await.map_err(|e| format!("CSV export task failed: {e}"))??;

    Ok(Some(path.to_string_lossy().to_string()))
}

#[derive(Debug, Serialize, Deserialize)]
struct DraftIntegrityFile {
    id: String,
    relative_path: String,
    sha256: String,
    size: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct DraftIntegrityPayload {
    version: u32,
    draft_id: String,
    draft_sha256: String,
    #[serde(default)]
    row_store_id: Option<String>,
    #[serde(default)]
    row_store_revision: Option<i64>,
    #[serde(default)]
    row_store_sha256: Option<String>,
    files: Vec<DraftIntegrityFile>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DraftIntegrityManifest {
    payload: DraftIntegrityPayload,
    mac: String,
}

fn safe_draft_dir_name(draft_id: &str) -> String {
    let mut out = String::with_capacity(draft_id.len().min(80));
    for ch in draft_id.chars().take(80) {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() { "draft".to_string() } else { out }
}

fn draft_dir(root: &Path, draft_id: &str) -> PathBuf {
    root.join("drafts").join(safe_draft_dir_name(draft_id))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    hex_encode(&hmac_sha256::Hash::hash(bytes))
}

fn sha256_file(path: &Path) -> Result<(String, u64), String> {
    let file = File::open(path).map_err(|e| format!("Unable to read integrity file '{}': {e}", path.display()))?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    let mut reader = BufReader::with_capacity(1024 * 1024, file);
    let mut hasher = hmac_sha256::Hash::new();
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let read = std::io::Read::read(&mut reader, &mut buffer).map_err(|e| e.to_string())?;
        if read == 0 { break; }
        hasher.update(&buffer[..read]);
    }
    Ok((hex_encode(&hasher.finalize()), metadata.len()))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn hex_decode(value: &str) -> Result<Vec<u8>, String> {
    let bytes = value.as_bytes();
    if bytes.len() % 2 != 0 { return Err("Invalid integrity MAC.".to_string()); }
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for chunk in bytes.chunks_exact(2) {
        let high = (chunk[0] as char).to_digit(16).ok_or_else(|| "Invalid integrity MAC.".to_string())?;
        let low = (chunk[1] as char).to_digit(16).ok_or_else(|| "Invalid integrity MAC.".to_string())?;
        out.push(((high << 4) | low) as u8);
    }
    Ok(out)
}

#[cfg(windows)]
fn draft_integrity_key() -> Result<Vec<u8>, String> {
    let entry = keyring::Entry::new("com.insight.datauploads.utility", "draft-integrity-key")
        .map_err(|e| format!("Unable to access Windows Credential Manager: {e}"))?;
    match entry.get_secret() {
        Ok(secret) if !secret.is_empty() => Ok(secret),
        Ok(_) => Err("The stored draft integrity key is empty.".to_string()),
        Err(keyring::Error::NoEntry) => {
            let mut key = [0u8; 32];
            getrandom::fill(&mut key).map_err(|e| format!("Unable to generate integrity key: {e}"))?;
            entry.set_secret(&key).map_err(|e| format!("Unable to store integrity key in Windows Credential Manager: {e}"))?;
            Ok(key.to_vec())
        }
        Err(error) => Err(format!("Unable to read draft integrity key from Windows Credential Manager: {error}")),
    }
}

#[cfg(not(windows))]
fn draft_integrity_key() -> Result<Vec<u8>, String> {
    // Development fallback for non-Windows builds. The packaged Windows application uses
    // Windows Credential Manager through keyring, so the secret is not stored beside drafts.
    Ok(b"insight-data-upload-development-integrity-key".to_vec())
}

fn set_tree_readonly(root: &Path, readonly: bool) -> Result<(), String> {
    if !root.exists() { return Ok(()); }
    for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if metadata.is_dir() {
            set_tree_readonly(&path, readonly)?;
        } else if metadata.is_file() {
            let mut permissions = metadata.permissions();
            permissions.set_readonly(readonly);
            fs::set_permissions(&path, permissions).map_err(|e| format!("Unable to change permissions for '{}': {e}", path.display()))?;
        }
    }
    Ok(())
}

fn hmac_for_payload(payload: &DraftIntegrityPayload) -> Result<String, String> {
    let key = draft_integrity_key()?;
    let bytes = serde_json::to_vec(payload).map_err(|e| format!("Unable to serialize integrity payload: {e}"))?;
    Ok(hex_encode(&hmac_sha256::HMAC::mac(&bytes, &key)))
}

fn row_store_hash(conn: &Connection, dataset_id: &str) -> Result<String, String> {
    let mut hasher = hmac_sha256::Hash::new();
    let mut stmt = conn.prepare(
        "SELECT rowid,serial_no,case_id,pan,name,dob_doi,mobile,email,pin_code,address,state,
                verification_status,information_fy,information_source_type,information_source_description,
                information_type,information_description,information_value,source,finding,actionable_ay,
                statutory_reason,verification_result_type,income_escaping_assessment_value,
                verification_information_value,result_description
         FROM imported_rows WHERE dataset_id=?1 ORDER BY rowid"
    ).map_err(|e| format!("Unable to prepare draft integrity query: {e}"))?;
    let mut rows = stmt.query(params![dataset_id]).map_err(|e| e.to_string())?;
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        for index in 0..26 {
            match index {
                0 => { let value: i64 = row.get(index).map_err(|e| e.to_string())?; hasher.update(value.to_string().as_bytes()); }
                _ => { let value: String = row.get(index).map_err(|e| e.to_string())?; hasher.update(value.as_bytes()); }
            }
            hasher.update(&[0]);
        }
    }
    Ok(hex_encode(&hasher.finalize()))
}

fn build_integrity_manifest(
    conn: &Connection,
    draft_dir: &Path,
    draft_id: &str,
    row_store_id: Option<&str>,
) -> Result<DraftIntegrityManifest, String> {
    let draft_path = draft_dir.join("draft.json");
    let draft_bytes = fs::read(&draft_path).map_err(|e| format!("Unable to read draft for integrity sealing: {e}"))?;
    let draft_value: Value = serde_json::from_slice(&draft_bytes).map_err(|e| format!("Invalid draft JSON while sealing: {e}"))?;
    let mut files = Vec::new();
    if let Some(documents) = draft_value.get("documents").and_then(Value::as_array) {
        for doc in documents {
            let Some(id) = doc.get("id").and_then(Value::as_str) else { continue; };
            let relative_path = doc.get("filePath").and_then(Value::as_str).unwrap_or("").to_string();
            if relative_path.is_empty() { continue; }
            let path = draft_dir.join(&relative_path);
            let root = draft_dir.canonicalize().map_err(|e| e.to_string())?;
            let canonical = path.canonicalize().map_err(|e| format!("Unable to resolve attached document '{}': {e}", path.display()))?;
            if !canonical.starts_with(&root) { return Err("Draft document path escapes the draft directory.".to_string()); }
            let (sha256, size) = sha256_file(&canonical)?;
            files.push(DraftIntegrityFile { id: id.to_string(), relative_path, sha256, size });
        }
    }
    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    let normalized_row_store_id = row_store_id
        .filter(|v| !v.trim().is_empty())
        .map(ToOwned::to_owned);
    let revision = normalized_row_store_id
        .as_deref()
        .map(|id| row_store_revision(conn, id))
        .transpose()?;

    let payload = DraftIntegrityPayload {
        version: 2,
        draft_id: draft_id.to_string(),
        draft_sha256: sha256_bytes(&draft_bytes),
        row_store_id: normalized_row_store_id,
        row_store_revision: revision,
        row_store_sha256: None,
        files,
    };
    let mac = hmac_for_payload(&payload)?;
    Ok(DraftIntegrityManifest { payload, mac })
}

fn verify_draft_integrity(conn: &Connection, draft_dir: &Path, draft_id: &str) -> Result<(), String> {
    let lock = draft_dir.join("draft.lock");
    let manifest_path = draft_dir.join("integrity.json");
    let draft_path = draft_dir.join("draft.json");
    if !lock.exists() || !manifest_path.exists() || !draft_path.exists() {
        return Err("Draft is not sealed. Save it again using the utility before opening it.".to_string());
    }
    let manifest: DraftIntegrityManifest = serde_json::from_slice(&fs::read(&manifest_path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Invalid draft integrity manifest: {e}"))?;
    if !matches!(manifest.payload.version, 1 | 2) || manifest.payload.draft_id != draft_id {
        return Err("Draft integrity manifest is invalid.".to_string());
    }
    let key = draft_integrity_key()?;
    let payload_bytes = serde_json::to_vec(&manifest.payload).map_err(|e| format!("Unable to serialize integrity payload: {e}"))?;
    let mac_bytes: [u8; 32] = hex_decode(&manifest.mac)?
        .try_into()
        .map_err(|_| "Invalid integrity MAC.".to_string())?;
    if !hmac_sha256::HMAC::verify(&payload_bytes, &key, &mac_bytes) {
        return Err("Draft integrity verification failed: manifest was modified outside the utility.".to_string());
    }
    let draft_bytes = fs::read(&draft_path).map_err(|e| e.to_string())?;
    if sha256_bytes(&draft_bytes) != manifest.payload.draft_sha256 {
        return Err("Draft integrity verification failed: draft.json was modified outside the utility.".to_string());
    }
    if let Some(expected_revision) = manifest.payload.row_store_revision {
        let row_store_id = manifest.payload.row_store_id.as_deref().unwrap_or(draft_id);
        let actual_revision = row_store_revision(conn, row_store_id)?;
        if actual_revision != expected_revision {
            return Err("Draft integrity verification failed: row data was modified outside the utility.".to_string());
        }
    } else if let Some(expected) = manifest.payload.row_store_sha256.as_deref() {
        // Backward compatibility for drafts created by the previous v1 integrity scheme.
        let row_store_id = manifest.payload.row_store_id.as_deref().unwrap_or(draft_id);
        let actual = row_store_hash(conn, row_store_id)?;
        if actual != expected {
            return Err("Draft integrity verification failed: row data was modified outside the utility.".to_string());
        }
    }

    let documents_dir = draft_dir.join("documents");
    let expected_paths: std::collections::HashSet<String> = manifest.payload.files.iter().map(|f| f.relative_path.clone()).collect();
    if documents_dir.exists() {
        for entry in fs::read_dir(&documents_dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if !entry.path().is_file() { return Err("Draft contains an unexpected documents directory entry.".to_string()); }
            let relative = format!("documents/{}", entry.file_name().to_string_lossy());
            if !expected_paths.contains(&relative) { return Err("Draft contains a document that was added outside the utility.".to_string()); }
        }
    }
    for file in &manifest.payload.files {
        let path = draft_dir.join(&file.relative_path);
        let (actual_hash, actual_size) = sha256_file(&path)?;
        if actual_size != file.size || actual_hash != file.sha256 {
            return Err(format!("Draft integrity verification failed: document '{}' was modified outside the utility.", file.relative_path));
        }
    }
    Ok(())
}

fn migrate_legacy_draft(
    conn: &Connection,
    draft_path: &Path,
    draft_id: &str,
) -> Result<(), String> {
    let Some(draft_dir) = draft_path.parent() else { return Err("Invalid draft directory.".to_string()); };
    set_tree_readonly(draft_dir, false)?;
    let mut draft_value: Value = serde_json::from_slice(&fs::read(draft_path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Invalid legacy draft JSON: {e}"))?;
    let documents_dir = draft_dir.join("documents");
    fs::create_dir_all(&documents_dir).map_err(|e| e.to_string())?;

    if let Some(documents) = draft_value.get_mut("documents").and_then(Value::as_array_mut) {
        for doc in documents {
            let Some(id) = doc.get("id").and_then(Value::as_str).map(ToOwned::to_owned) else { continue; };
            let name = doc.get("fileName").and_then(Value::as_str).unwrap_or("document").to_string();
            let safe_name = sanitize_file_name(&name);
            let target_relative = format!("documents/{}-{}", id, safe_name);
            let target = draft_dir.join(&target_relative);
            if !target.exists() {
                if let Some(source) = doc.get("filePath").and_then(Value::as_str) {
                    let source_path = Path::new(source);
                    if source_path.is_file() {
                        fs::copy(source_path, &target).map_err(|e| format!("Unable to migrate document '{}': {e}", name))?;
                    }
                }
            }
            if target.is_file() {
                if let Some(obj) = doc.as_object_mut() {
                    obj.insert("filePath".to_string(), Value::String(target_relative));
                }
            }
        }
    }
    if let Some(obj) = draft_value.as_object_mut() {
        obj.insert("draftId".to_string(), Value::String(draft_id.to_string()));
        obj.insert("rowStoreId".to_string(), Value::String(draft_id.to_string()));
    }
    let bytes = serde_json::to_vec_pretty(&draft_value).map_err(|e| e.to_string())?;
    fs::write(draft_path, &bytes).map_err(|e| format!("Unable to migrate legacy draft JSON: {e}"))?;
    let manifest = build_integrity_manifest(conn, draft_dir, draft_id, Some(draft_id))?;
    fs::write(draft_dir.join("integrity.json"), serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Unable to write draft integrity manifest: {e}"))?;
    fs::write(draft_dir.join("draft.lock"), format!("INSIGHT-DRAFT-LOCK v1\nDraft: {draft_id}\nManaged by Insight Data Uploads Utility.\n"))
        .map_err(|e| format!("Unable to write draft lock: {e}"))?;
    set_tree_readonly(draft_dir, true)?;
    Ok(())
}

fn save_draft_blocking(app: tauri::AppHandle, request: SaveDraftRequest) -> Result<SavedDraftSummary, String> {
    let _save_guard = SAVE_DRAFT_MUTEX
        .lock()
        .map_err(|_| "Another draft save is already in progress.".to_string())?;

    let root = storage_root(&app)?;
    let conn = connection(&app)?;

    let draft: Value = serde_json::from_str(&request.draft_json)
        .map_err(|e| format!("Invalid draft JSON: {e}"))?;
    if draft.get("version").and_then(Value::as_i64).unwrap_or_default() != 1 {
        return Err("Unsupported draft version.".to_string());
    }

    let reference_number = draft.get("packet")
        .and_then(|p| p.get("referenceNumber"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let step = draft.get("step").and_then(Value::as_i64).unwrap_or(1);
    let mut row_count = draft.get("rowCount")
        .and_then(Value::as_u64)
        .or_else(|| draft.get("rows").and_then(Value::as_array).map(|r| r.len() as u64))
        .unwrap_or(0);

    let timestamp = draft.get("savedAt")
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(now_iso);

    let draft_id = request.draft_id
        .filter(|id| !id.trim().is_empty())
        .or_else(|| draft.get("draftId").and_then(Value::as_str).map(ToOwned::to_owned))
        .unwrap_or_else(|| format!("draft-{}", uuid_like()));

    let working_store_id = draft.get("rowStoreId")
        .and_then(Value::as_str)
        .filter(|id| !id.trim().is_empty())
        .map(ToOwned::to_owned);

    if let Some(store_id) = working_store_id.as_deref() {
        // The native import/delete/edit paths keep dataset_metadata.row_count current.
        // Read that metadata row instead of scanning the 500K-row table during save.
        let current_count: i64 = conn.query_row(
            "SELECT COALESCE(row_count,0) FROM dataset_metadata WHERE dataset_id=?1",
            params![store_id],
            |r| r.get(0),
        ).optional()
        .map_err(|e| e.to_string())?
        .unwrap_or(0);
        row_count = current_count.max(0) as u64;
    }

    let drafts_root = root.join("drafts");
    fs::create_dir_all(&drafts_root)
        .map_err(|e| format!("Unable to create drafts directory: {e}"))?;

    let final_dir = draft_dir(&root, &draft_id);
    let staging_dir = drafts_root.join(format!(
        ".staging-{}-{}",
        safe_draft_dir_name(&draft_id),
        uuid_like()
    ));
    fs::create_dir_all(staging_dir.join("documents"))
        .map_err(|e| format!("Unable to create draft staging directory: {e}"))?;

    let mut draft_value = draft;
    if let Some(obj) = draft_value.as_object_mut() {
        obj.insert("draftId".to_string(), Value::String(draft_id.clone()));
        // Keep the physical SQLite dataset ID stable. Renaming a 500K-row store by
        // updating dataset_id on every row was one of the main sources of draft latency
        // and database contention.
        obj.insert(
            "rowStoreId".to_string(),
            Value::String(working_store_id.clone().unwrap_or_else(|| draft_id.clone())),
        );
        obj.insert(
            "rowCount".to_string(),
            Value::Number(serde_json::Number::from(row_count)),
        );
        if let Some(rows) = obj.get_mut("rows") {
            *rows = Value::Array(Vec::new());
        }
    }

    let documents_by_id: std::collections::HashMap<String, StoredDraftFile> = request.files
        .into_iter()
        .map(|file| (file.id.clone(), file))
        .collect();

    if let Some(documents) = draft_value.get_mut("documents").and_then(Value::as_array_mut) {
        for doc in documents {
            let Some(id) = doc.get("id").and_then(Value::as_str).map(ToOwned::to_owned) else {
                continue;
            };
            let Some(file) = documents_by_id.get(&id) else {
                continue;
            };

            let safe_name = sanitize_file_name(&file.name);
            let relative_path = format!("documents/{}-{}", id, safe_name);
            let destination = staging_dir.join(&relative_path);

            if let Some(source) = file.file_path.as_ref().filter(|v| !v.trim().is_empty()) {
                let source_path = Path::new(source);
                let metadata = fs::metadata(source_path)
                    .map_err(|e| format!("Unable to access document '{}': {e}", file.name))?;
                if !metadata.is_file() {
                    return Err(format!("Document '{}' is not a regular file.", file.name));
                }
                fs::copy(source_path, &destination)
                    .map_err(|e| format!("Unable to copy document '{}': {e}", file.name))?;
            } else {
                fs::write(&destination, &file.bytes)
                    .map_err(|e| format!("Unable to save document '{}': {e}", file.name))?;
            }

            if let Some(obj) = doc.as_object_mut() {
                let file_size = if let Some(source) = file.file_path.as_ref().filter(|v| !v.trim().is_empty()) {
                    fs::metadata(source).map(|m| m.len()).unwrap_or(file.bytes.len() as u64)
                } else {
                    file.bytes.len() as u64
                };
                obj.insert("filePath".to_string(), Value::String(relative_path));
                obj.insert(
                    "fileSize".to_string(),
                    Value::Number(serde_json::Number::from(file_size)),
                );
                obj.insert("fileType".to_string(), Value::String(file.r#type.clone()));
                obj.insert(
                    "lastModified".to_string(),
                    Value::Number(serde_json::Number::from(file.last_modified)),
                );
            }
        }
    }

    let draft_file = staging_dir.join("draft.json");
    fs::write(
        &draft_file,
        serde_json::to_vec_pretty(&draft_value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("Unable to write draft file: {e}"))?;

    if final_dir.exists() {
        set_tree_readonly(&final_dir, false)?;
        fs::remove_dir_all(&final_dir)
            .map_err(|e| format!("Unable to replace saved draft files: {e}"))?;
    }

    // IMPORTANT: Do not rename the SQLite row store here. Keeping dataset_id stable
    // turns a 500K-row UPDATE into a single metadata write.
    fs::rename(&staging_dir, &final_dir)
        .map_err(|e| format!("Unable to finalize saved draft: {e}"))?;

    let manifest = build_integrity_manifest(
        &conn,
        &final_dir,
        &draft_id,
        working_store_id.as_deref(),
    )?;
    fs::write(
        final_dir.join("integrity.json"),
        serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("Unable to write draft integrity manifest: {e}"))?;
    fs::write(
        final_dir.join("draft.lock"),
        format!(
            "INSIGHT-DRAFT-LOCK v2\nDraft: {draft_id}\nManaged by Insight Data Uploads Utility.\n"
        ),
    )
    .map_err(|e| format!("Unable to write draft lock: {e}"))?;
    set_tree_readonly(&final_dir, true)?;

    conn.execute(
        "INSERT INTO saved_draft(id,file_path,reference_number,row_count,step,created_at,updated_at,row_store_id)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8)
         ON CONFLICT(id) DO UPDATE SET file_path=excluded.file_path,
             reference_number=excluded.reference_number,row_count=excluded.row_count,
             step=excluded.step,updated_at=excluded.updated_at,row_store_id=excluded.row_store_id",
        params![
            draft_id,
            final_dir.join("draft.json").to_string_lossy(),
            reference_number,
            row_count as i64,
            step,
            timestamp,
            timestamp,
            working_store_id.as_deref().unwrap_or(draft_id.as_str()),
        ],
    )
    .map_err(|e| format!("Unable to save draft index in SQLite: {e}"))?;

    if let Err(error) = conn.execute(
        "INSERT INTO upload_draft(
             id,upload_type,local_status,selected_file_name,selected_file_path,
             selected_file_size,created_at,updated_at
         )
         VALUES(?1,'LOCAL_DRAFT','SAVED',NULL,?2,NULL,?3,?3)
         ON CONFLICT(id) DO UPDATE SET local_status='SAVED',
             selected_file_path=excluded.selected_file_path,updated_at=excluded.updated_at",
        params![draft_id, final_dir.join("draft.json").to_string_lossy(), timestamp],
    ) {
        eprintln!("Unable to update auxiliary upload_draft state: {error}");
    }

    Ok(SavedDraftSummary {
        id: draft_id.clone(),
        reference_number,
        updated_at: timestamp,
        row_count: row_count as usize,
        step,
        file_path: Some(final_dir.join("draft.json").to_string_lossy().to_string()),
        row_store_id: working_store_id.or_else(|| Some(draft_id.clone())),
    })
}

#[tauri::command(rename_all = "camelCase")]
async fn save_draft(app: tauri::AppHandle, request: SaveDraftRequest) -> Result<SavedDraftSummary, String> {
    tauri::async_runtime::spawn_blocking(move || save_draft_blocking(app, request))
        .await
        .map_err(|e| format!("Unable to save draft: {e}"))?
}

fn list_drafts_blocking(app: tauri::AppHandle) -> Result<Vec<SavedDraftSummary>, String> {
    let conn = connection(&app)?;
    let mut stmt = conn
        .prepare("SELECT id,reference_number,updated_at,row_count,step,file_path,row_store_id FROM saved_draft ORDER BY updated_at DESC")
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
                row_store_id: row.get(6)?,
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
async fn list_drafts(app: tauri::AppHandle) -> Result<Vec<SavedDraftSummary>, String> {
    tauri::async_runtime::spawn_blocking(move || list_drafts_blocking(app))
        .await
        .map_err(|e| format!("Unable to list drafts: {e}"))?
}

#[tauri::command(rename_all = "camelCase")]
async fn verify_draft_integrity_command(app: tauri::AppHandle, draft_id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || verify_draft_integrity_blocking(app, draft_id))
        .await
        .map_err(|e| format!("Unable to verify draft: {e}"))?
}

fn verify_draft_integrity_blocking(app: tauri::AppHandle, draft_id: String) -> Result<(), String> {
    let root = storage_root(&app)?;
    let conn = connection(&app)?;
    let draft_path: Option<String> = conn
        .query_row("SELECT file_path FROM saved_draft WHERE id=?1", params![draft_id], |row| row.get(0))
        .optional()
        .map_err(|e| e.to_string())?;
    let Some(path) = draft_path else { return Err("Saved draft could not be found.".to_string()); };
    let draft_dir_path = PathBuf::from(path)
        .parent()
        .ok_or_else(|| "Invalid saved draft path.".to_string())?
        .to_path_buf();
    let drafts_root = root.join("drafts").canonicalize().map_err(|e| e.to_string())?;
    let canonical_dir = draft_dir_path.canonicalize().map_err(|e| e.to_string())?;
    if !canonical_dir.starts_with(&drafts_root) {
        return Err("Saved draft is outside the managed drafts directory.".to_string());
    }
    if !canonical_dir.join("integrity.json").exists() {
        migrate_legacy_draft(&conn, &canonical_dir.join("draft.json"), &draft_id)?;
    }
    verify_draft_integrity(&conn, &canonical_dir, &draft_id)
}

fn load_draft_blocking(app: tauri::AppHandle, draft_id: String) -> Result<Option<LoadedDraft>, String> {
    let root = storage_root(&app)?;
    let conn = connection(&app)?;
    let draft_path: Option<String> = conn
        .query_row("SELECT file_path FROM saved_draft WHERE id=?1", params![draft_id], |row| row.get(0))
        .optional()
        .map_err(|e| e.to_string())?;
    let Some(draft_path) = draft_path else { return Ok(None); };
    let path = PathBuf::from(draft_path);
    if !path.exists() { return Ok(None); }

    let drafts_root = root.join("drafts").canonicalize().map_err(|e| e.to_string())?;
    let draft_dir_path = path.parent().ok_or_else(|| "Invalid saved draft path.".to_string())?;
    let canonical_dir = draft_dir_path.canonicalize().map_err(|e| e.to_string())?;
    if !canonical_dir.starts_with(&drafts_root) {
        return Err("Saved draft is outside the managed drafts directory.".to_string());
    }

    if !canonical_dir.join("integrity.json").exists() {
        migrate_legacy_draft(&conn, &canonical_dir.join("draft.json"), &draft_id)?;
    }
    verify_draft_integrity(&conn, &canonical_dir, &draft_id)?;

    let draft_json = fs::read_to_string(canonical_dir.join("draft.json"))
        .map_err(|e| format!("Unable to read saved draft: {e}"))?;
    let draft_value: Value = serde_json::from_str(&draft_json)
        .map_err(|e| format!("Invalid saved draft JSON: {e}"))?;
    let mut files = Vec::new();

    if let Some(documents) = draft_value.get("documents").and_then(Value::as_array) {
        for doc in documents {
            let Some(id) = doc.get("id").and_then(Value::as_str) else { continue; };
            let Some(name) = doc.get("fileName").and_then(Value::as_str) else { continue; };
            let Some(relative) = doc.get("filePath").and_then(Value::as_str).filter(|v| !v.trim().is_empty()) else { continue; };
            let document_path = canonical_dir.join(relative);
            let bytes = fs::read(&document_path).map_err(|e| format!("Unable to read saved document '{name}': {e}"))?;
            files.push(StoredDraftFileResponse {
                id: id.to_string(),
                name: name.to_string(),
                r#type: doc.get("fileType").and_then(Value::as_str).unwrap_or("").to_string(),
                last_modified: doc.get("lastModified").and_then(Value::as_i64).unwrap_or(0),
                bytes,
            });
        }
    }

    Ok(Some(LoadedDraft { draft_json, files }))
}

#[tauri::command(rename_all = "camelCase")]
async fn load_draft(app: tauri::AppHandle, draft_id: String) -> Result<Option<LoadedDraft>, String> {
    tauri::async_runtime::spawn_blocking(move || load_draft_blocking(app, draft_id))
        .await
        .map_err(|e| format!("Unable to load draft: {e}"))?
}

fn delete_draft_blocking(app: tauri::AppHandle, draft_id: String) -> Result<(), String> {
    let root = storage_root(&app)?;
    let conn = connection(&app)?;

    let draft_path: Option<String> = conn
        .query_row(
            "SELECT file_path FROM saved_draft WHERE id=?1",
            params![draft_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    let row_store_id: Option<String> = conn
        .query_row(
            "SELECT row_store_id FROM saved_draft WHERE id=?1",
            params![draft_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    conn.execute(
        "DELETE FROM saved_draft WHERE id=?1",
        params![draft_id],
    ).map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM upload_draft WHERE id=?1",
        params![draft_id],
    ).map_err(|e| e.to_string())?;

    let dataset_id = row_store_id.as_deref().unwrap_or(&draft_id);
    clear_dataset(&conn, dataset_id)?;

    if let Some(path) = draft_path {
        if let Some(parent) = PathBuf::from(path).parent() {
            if parent.exists() {
                set_tree_readonly(parent, false)?;
                fs::remove_dir_all(parent)
                    .map_err(|e| format!("Unable to remove draft files: {e}"))?;
            }
        }
    } else {
        let dir = draft_dir(&root, &draft_id);
        if dir.exists() {
            set_tree_readonly(&dir, false)?;
            fs::remove_dir_all(dir)
                .map_err(|e| format!("Unable to remove draft files: {e}"))?;
        }
    }
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
async fn delete_draft(app: tauri::AppHandle, draft_id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || delete_draft_blocking(app, draft_id))
        .await
        .map_err(|e| format!("Unable to delete draft: {e}"))?
}

#[tauri::command]
fn get_storage_location(app: tauri::AppHandle) -> Result<String, String> {
    Ok(storage_root(&app)?.to_string_lossy().to_string())
}

#[tauri::command(rename_all = "camelCase")]
async fn save_export_file(suggested_name: String, content: String) -> Result<Option<String>, String> {
    let Some(handle) = rfd::AsyncFileDialog::new()
        .set_title("Save file")
        .set_file_name(&suggested_name)
        .save_file()
        .await
    else {
        return Ok(None);
    };

    let path = handle.path().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        fs::write(&path, content.as_bytes())
            .map_err(|e| format!("Unable to save file: {e}"))?;
        Ok::<Option<String>, String>(Some(path.to_string_lossy().to_string()))
    })
    .await
    .map_err(|e| format!("Save operation failed: {e}"))?
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

#[tauri::command]
fn open_government_website(url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("Configured government website URL must use http or https.".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &url])
            .spawn()
            .map_err(|e| format!("Unable to open the Government of India website: {e}"))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&url)
            .spawn()
            .map_err(|e| format!("Unable to open the Government of India website: {e}"))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&url)
            .spawn()
            .map_err(|e| format!("Unable to open the Government of India website: {e}"))?;
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        return Err("Opening an external browser is not supported on this operating system.".to_string());
    }

    Ok(())
}

#[tauri::command]
fn open_external_url(url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("Authentication URL must use http or https.".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &url])
            .spawn()
            .map_err(|e| format!("Unable to open the default browser: {e}"))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&url)
            .spawn()
            .map_err(|e| format!("Unable to open the default browser: {e}"))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&url)
            .spawn()
            .map_err(|e| format!("Unable to open the default browser: {e}"))?;
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        return Err("Opening the system browser is not supported on this operating system.".to_string());
    }

    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
fn prepare_oauth_callback(app: tauri::AppHandle, path: String) -> Result<String, String> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|e| format!("Unable to open OAuth loopback listener: {e}"))?;
    let address = listener
        .local_addr()
        .map_err(|e| format!("Unable to resolve OAuth callback port: {e}"))?;
    let callback_path = if path.starts_with('/') { path } else { format!("/{path}") };

    tauri::async_runtime::spawn_blocking(move || {
        let Ok((mut stream, _peer)) = listener.accept() else { return; };
        let mut buffer = [0u8; 8192];
        let Ok(size) = stream.read(&mut buffer) else { return; };
        let request = String::from_utf8_lossy(&buffer[..size]);
        let request_line = request.lines().next().unwrap_or_default();
        let mut parts = request_line.split_whitespace();
        let _method = parts.next();
        let target = parts.next().unwrap_or("/");
        let callback_url = if target.starts_with("http://") {
            target.to_string()
        } else {
            format!("http://{}{}", address, target)
        };

        let body = "<html><body><h3>Login complete</h3><p>You can return to the Insight Data Upload Utility.</p><script>window.close()</script></body></html>";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(), body
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
        let _ = app.emit("oauth-callback", OAuthCallbackPayload { url: callback_url });
    });

    Ok(format!("http://{}{}", address, callback_path))
}

#[tauri::command(rename_all = "camelCase")]
fn store_oauth_tokens(
    access_token: String,
    refresh_token: Option<String>,
    expires_at: Option<i64>,
    token_type: Option<String>,
) -> Result<(), String> {
    let tokens = StoredOAuthTokens { access_token, refresh_token, expires_at, token_type };
    let serialized = serde_json::to_string(&tokens).map_err(|e| e.to_string())?;

    #[cfg(target_os = "windows")]
    {
        let entry = Entry::new(OAUTH_CREDENTIAL_SERVICE, OAUTH_CREDENTIAL_USER)
            .map_err(|e| format!("Unable to access Windows Credential Manager: {e}"))?;
        entry
            .set_password(&serialized)
            .map_err(|e| format!("Unable to store OAuth tokens in Windows Credential Manager: {e}"))?;
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        // TODO: Add platform-native secure storage (macOS Keychain / Linux Secret Service).
        // For the POC the token remains memory-only on other OSes; never write it to files/localStorage.
        let _ = serialized;
        Ok(())
    }
}

#[tauri::command]
fn get_oauth_tokens() -> Result<Option<StoredOAuthTokens>, String> {
    #[cfg(target_os = "windows")]
    {
        let entry = Entry::new(OAUTH_CREDENTIAL_SERVICE, OAUTH_CREDENTIAL_USER)
            .map_err(|e| format!("Unable to access Windows Credential Manager: {e}"))?;
        match entry.get_password() {
            Ok(value) => serde_json::from_str(&value).map(Some).map_err(|e| format!("Stored OAuth credential is invalid: {e}")),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(format!("Unable to read OAuth tokens from Windows Credential Manager: {e}")),
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        // TODO: Add platform-native secure storage (macOS Keychain / Linux Secret Service).
        Err("Secure OAuth token storage is currently implemented for Windows only.".to_string())
    }
}

#[tauri::command]
fn clear_oauth_tokens() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let entry = Entry::new(OAUTH_CREDENTIAL_SERVICE, OAUTH_CREDENTIAL_USER)
            .map_err(|e| format!("Unable to access Windows Credential Manager: {e}"))?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(format!("Unable to clear OAuth tokens from Windows Credential Manager: {e}")),
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        // TODO: Add platform-native secure storage removal for macOS/Linux.
        Ok(())
    }
}

fn uuid_like() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    format!("{:x}", nanos)
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn_blocking(move || {
                println!("[RUST] Initializing database schema on startup...");
                if let Err(e) = connection(&handle) {
                    eprintln!("[RUST] Database initialization error: {}", e);
                } else {
                    println!("[RUST] Database schema initialized successfully.");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            pick_file,pick_supporting_documents,read_file_bytes,pick_csv_file,import_csv_to_store,get_csv_import_status,get_row_page,get_row_by_case_id,
            commands::data_commands::get_runtime_config,commands::data_commands::get_row_window,commands::data_commands::stream_row_window,
            upsert_row,delete_rows,set_rows_status,clear_row_store,clone_row_store,rename_row_store,seed_row_store,export_csv_file,
            save_draft,list_drafts,verify_draft_integrity_command,load_draft,delete_draft,get_storage_location,save_export_file,cleanup_temp_files,get_app_version,open_government_website,open_external_url,prepare_oauth_callback,store_oauth_tokens,get_oauth_tokens,clear_oauth_tokens
        ])
        .run(tauri::generate_context!())
        .expect("error while running Insight Data Upload Utility");
}
