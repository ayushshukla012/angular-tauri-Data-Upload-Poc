#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use csv::StringRecord;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

static SCHEMA_INITIALIZED: AtomicBool = AtomicBool::new(false);
use tauri::{Emitter, Manager};
use tauri::path::BaseDirectory;

mod commands;


const MAX_FILE_SIZE: u64 = 25 * 1024 * 1024;

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
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_imported_rows_dataset_row_order ON imported_rows(dataset_id, row_order)",
        [],
    ).map_err(|e| e.to_string())?;
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

/// Lightweight connection: WAL mode + busy timeout only. No schema migrations.
/// Safe to call during active CSV import without blocking.
pub(crate) fn connection_fast(app: &tauri::AppHandle) -> Result<Connection, String> {
    let path = db_path(app)?;
    let conn = Connection::open(path).map_err(|e| format!("Unable to open SQLite database: {e}"))?;
    conn.execute_batch(
        "PRAGMA busy_timeout=5000;
         PRAGMA journal_mode=WAL;",
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
           updated_at TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS dataset_metadata (
           dataset_id TEXT PRIMARY KEY,
           row_count INTEGER NOT NULL DEFAULT 0,
           status TEXT NOT NULL DEFAULT 'READY',
           source_file_path TEXT,
           updated_at TEXT NOT NULL DEFAULT ''
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
    conn.execute(
        "INSERT OR REPLACE INTO imported_rows(dataset_id,row_order,serial_no,case_id,pan,name,dob_doi,mobile,email,pin_code,address,state,verification_status,information_fy,information_source_type,information_source_description,information_type,information_description,information_value,source,finding,actionable_ay,statutory_reason,verification_result_type,income_escaping_assessment_value,verification_information_value,result_description) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27)",
        params![dataset_id,row.serial_no.parse::<i64>().unwrap_or(0),row.serial_no,row.case_id,row.pan,row.name,row.dob_doi,row.mobile,row.email,row.pin_code,row.address,row.state,row.verification_status,row.information_details.information_fy,row.information_details.information_source_type,row.information_details.information_source_description,row.information_details.information_type,row.information_details.information_description,row.information_details.information_value,row.information_details.source,row.information_details.finding,row.verification_details.actionable_ay,row.verification_details.statutory_reason,row.verification_details.verification_result_type,row.verification_details.income_escaping_assessment_value,row.verification_details.information_value,row.verification_details.result_description],
    ).map_err(|e| format!("Unable to save row: {e}"))?;
    Ok(())
}

fn execute_upsert_tx(tx: &rusqlite::Transaction<'_>, dataset_id: &str, row: &NativePersonRowInput) -> Result<(), String> {
    tx.execute(
        "INSERT OR REPLACE INTO imported_rows(dataset_id,row_order,serial_no,case_id,pan,name,dob_doi,mobile,email,pin_code,address,state,verification_status,information_fy,information_source_type,information_source_description,information_type,information_description,information_value,source,finding,actionable_ay,statutory_reason,verification_result_type,income_escaping_assessment_value,verification_information_value,result_description) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27)",
        params![dataset_id,row.serial_no.parse::<i64>().unwrap_or(0),row.serial_no,row.case_id,row.pan,row.name,row.dob_doi,row.mobile,row.email,row.pin_code,row.address,row.state,row.verification_status,row.information_details.information_fy,row.information_details.information_source_type,row.information_details.information_source_description,row.information_details.information_type,row.information_details.information_description,row.information_details.information_value,row.information_details.source,row.information_details.finding,row.verification_details.actionable_ay,row.verification_details.statutory_reason,row.verification_details.verification_result_type,row.verification_details.income_escaping_assessment_value,row.verification_details.information_value,row.verification_details.result_description],
    ).map_err(|e| format!("Unable to save row: {e}"))?;
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
fn pick_csv_file() -> Result<Option<PickedCsvFile>, String> {
    let Some(path) = rfd::FileDialog::new().set_title("Select CSV file").add_filter("CSV files", &["csv"]).pick_file() else { return Ok(None); };
    let metadata = fs::metadata(&path).map_err(|e| format!("Unable to access selected CSV: {e}"))?;
    let file_name = path.file_name().and_then(|n| n.to_str()).ok_or_else(|| "The selected CSV has an invalid name.".to_string())?.to_string();
    Ok(Some(PickedCsvFile { file_name, file_path: path.to_string_lossy().to_string(), file_size: metadata.len() }))
}

#[allow(dead_code)]
fn record_value(record: &StringRecord, index: usize) -> String { record.get(index).unwrap_or("").trim().to_string() }

#[allow(dead_code)]
fn validate_and_map_record(record: &StringRecord, row_number: usize, dataset_index: usize, reference: &str) -> Result<NativePersonRowInput, String> {
    if record.len() != 19 { return Err(format!("Invalid CSV data at row {row_number}: expected 19 columns but found {}.", record.len())); }
    let pan=record_value(record,0); let name=record_value(record,1); let dob_doi=record_value(record,2); let mobile=record_value(record,3); let email=record_value(record,4); let pin_code=record_value(record,5);
    let required=[("PAN",pan.as_str()),("Name",name.as_str()),("DOB/DOI",dob_doi.as_str()),("Mobile",mobile.as_str()),("E-Mail",email.as_str()),("PIN Code",pin_code.as_str()),("Address",record.get(6).unwrap_or("")),("State",record.get(7).unwrap_or("")),("FY",record.get(8).unwrap_or("")),("Information Type",record.get(9).unwrap_or("")),("Findings",record.get(10).unwrap_or("")),("Source",record.get(11).unwrap_or("")),("Information Value",record.get(12).unwrap_or("")),("Actionable AY",record.get(14).unwrap_or("")),("Verification Result Type",record.get(15).unwrap_or("")),("Statutory Reason",record.get(16).unwrap_or("")),("Income Escaping Assessment Value",record.get(17).unwrap_or("")),("Verification Information Value",record.get(18).unwrap_or(""))];
    if let Some((field,_))=required.iter().find(|(_,v)|v.trim().is_empty()){return Err(format!("Invalid CSV data at row {row_number}: {field} is required."));}
    let pan_chars: Vec<char>=pan.chars().collect(); if pan_chars.len()!=10 || !pan_chars[..5].iter().all(|c|c.is_ascii_alphabetic()) || !pan_chars[5..9].iter().all(|c|c.is_ascii_digit()) || !pan_chars[9..10].iter().all(|c|c.is_ascii_alphabetic()){return Err(format!("Invalid PAN at CSV row {row_number}."));}
    if mobile.len()!=10 || !mobile.chars().all(|c|c.is_ascii_digit()){return Err(format!("Invalid Mobile at CSV row {row_number}."));}
    if pin_code.len()!=6 || !pin_code.chars().all(|c|c.is_ascii_digit()){return Err(format!("Invalid PIN Code at CSV row {row_number}."));}
    if !email.contains('@') || !email.contains('.') { return Err(format!("Invalid E-Mail at CSV row {row_number}.")); }
    let index=dataset_index.max(1); let reference=if reference.trim().is_empty(){"LOCAL"}else{reference.trim()};
    Ok(NativePersonRowInput{serial_no:format!("{:05}",index),case_id:format!("{}-{:05}",reference,index),pan:pan.to_ascii_uppercase(),name,dob_doi,mobile,email:email.to_ascii_lowercase(),pin_code,address:record_value(record,6),state:record_value(record,7),verification_status:"Pending".to_string(),information_details:NativeInformationDetailsInput{information_fy:record_value(record,8),information_source_type:String::new(),information_source_description:String::new(),information_type:record_value(record,9),information_description:record_value(record,13),information_value:record_value(record,12),source:record_value(record,11),finding:record_value(record,10)},verification_details:NativeVerificationDetailsInput{actionable_ay:record_value(record,14),statutory_reason:record_value(record,16),verification_result_type:record_value(record,15),income_escaping_assessment_value:record_value(record,17),information_value:record_value(record,18),result_description:String::new()}})
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

fn trim_csv(value: Option<&str>) -> &str {
    value.unwrap_or("").trim()
}

fn validate_csv_fast(record: &StringRecord, row_number: usize) -> Result<(), String> {
    if record.len() != 19 {
        return Err(format!(
            "Invalid CSV data at row {row_number}: expected 19 columns but found {}.",
            record.len()
        ));
    }

    let pan = trim_csv(record.get(0));
    let mobile = trim_csv(record.get(3));
    let email = trim_csv(record.get(4));
    let pin_code = trim_csv(record.get(5));

    for (field, value) in [
        ("PAN", pan),
        ("Name", trim_csv(record.get(1))),
        ("DOB/DOI", trim_csv(record.get(2))),
        ("Mobile", mobile),
        ("E-Mail", email),
        ("PIN Code", pin_code),
        ("Address", trim_csv(record.get(6))),
        ("State", trim_csv(record.get(7))),
        ("FY", trim_csv(record.get(8))),
        ("Information Type", trim_csv(record.get(9))),
        ("Findings", trim_csv(record.get(10))),
        ("Source", trim_csv(record.get(11))),
        ("Information Value", trim_csv(record.get(12))),
        ("Actionable AY", trim_csv(record.get(14))),
        ("Verification Result Type", trim_csv(record.get(15))),
        ("Statutory Reason", trim_csv(record.get(16))),
        ("Income Escaping Assessment Value", trim_csv(record.get(17))),
        ("Verification Information Value", trim_csv(record.get(18))),
    ] {
        if value.is_empty() {
            return Err(format!(
                "Invalid CSV data at row {row_number}: {field} is required."
            ));
        }
    }

    let pan_bytes = pan.as_bytes();
    let valid_pan = pan_bytes.len() == 10
        && pan_bytes[0..5].iter().all(u8::is_ascii_alphabetic)
        && pan_bytes[5..9].iter().all(u8::is_ascii_digit)
        && pan_bytes[9].is_ascii_alphabetic();

    if !valid_pan {
        return Err(format!("Invalid PAN at CSV row {row_number}."));
    }
    if mobile.len() != 10 || !mobile.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("Invalid Mobile at CSV row {row_number}."));
    }
    if pin_code.len() != 6 || !pin_code.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("Invalid PIN Code at CSV row {row_number}."));
    }
    if !email.contains('@') || !email.contains('.') {
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
        .from_reader(BufReader::with_capacity(8 * 1024 * 1024, file));

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

    // The existing POC created four secondary indexes before ingest. That multiplies SQLite
    // write work for every row. We keep only the one index required for deterministic page reads.
    conn.execute(
        "DROP INDEX IF EXISTS idx_imported_rows_dataset_row_order",
        [],
    )
    .map_err(|e| format!("Unable to prepare SQLite import indexes: {e}"))?;

    conn.execute_batch(&format!(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA temp_store=MEMORY;
         PRAGMA cache_size=-{};",
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

    let reference = if reference_number.trim().is_empty() {
        "LOCAL"
    } else {
        reference_number.trim()
    };

    let insert_sql = optimized_csv_insert_sql();
    let mut imported = 0usize;
    let mut ready_emitted = false;
    let mut next_progress_at = ready_rows;

    loop {
        let current_batch_size = if !ready_emitted {
            ready_rows
        } else {
            batch_rows.min(progress_rows.max(1))
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
                let Some(result) = reader.records().next() else {
                    break;
                };

                let record =
                    result.map_err(|e| format!("Unable to read CSV row {}: {e}", imported + 2))?;

                let row_number = imported + 2;
                validate_csv_fast(&record, row_number)?;

                let order = imported + 1;
                let serial_no = format!("{order:07}");
                let case_id = format!("{reference}-{serial_no}");
                let pan = trim_csv(record.get(0)).to_ascii_uppercase();
                let email = trim_csv(record.get(4)).to_ascii_lowercase();

                stmt.execute(params![
                    dataset_id,
                    order as i64,
                    serial_no,
                    case_id,
                    pan,
                    trim_csv(record.get(1)),
                    trim_csv(record.get(2)),
                    trim_csv(record.get(3)),
                    email,
                    trim_csv(record.get(5)),
                    trim_csv(record.get(6)),
                    trim_csv(record.get(7)),
                    "Pending",
                    trim_csv(record.get(8)),
                    "",
                    "",
                    trim_csv(record.get(9)),
                    trim_csv(record.get(13)),
                    trim_csv(record.get(12)),
                    trim_csv(record.get(11)),
                    trim_csv(record.get(10)),
                    trim_csv(record.get(14)),
                    trim_csv(record.get(16)),
                    trim_csv(record.get(15)),
                    trim_csv(record.get(17)),
                    trim_csv(record.get(18)),
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
        "CREATE INDEX IF NOT EXISTS idx_imported_rows_dataset_row_order
         ON imported_rows(dataset_id,row_order)",
        [],
    )
    .map_err(|e| format!("Unable to finalize the row-order index: {e}"))?;

    conn.execute(
        "UPDATE dataset_metadata
         SET row_count=?2,status='READY',source_file_path=?3,updated_at=?4
         WHERE dataset_id=?1",
        params![dataset_id, imported as i64, file_path, now_iso()],
    )
    .map_err(|e| format!("Unable to finalize import metadata: {e}"))?;

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

        let mut stmt = if term.is_empty() {
            conn.prepare(&format!(
                "{ROW_SELECT} WHERE dataset_id=?1 ORDER BY row_order LIMIT ?2 OFFSET ?3"
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
                 ORDER BY row_order LIMIT ?2 OFFSET ?3"
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
        execute_upsert(&conn,&dataset_id,&row)
    }).await.map_err(|e| e.to_string())?
}
#[tauri::command(rename_all = "camelCase")]
fn delete_rows(app: tauri::AppHandle,dataset_id:String,case_ids:Vec<String>)->Result<(),String>{let conn=connection(&app)?;let tx=conn.unchecked_transaction().map_err(|e|e.to_string())?;for id in case_ids{tx.execute("DELETE FROM imported_rows WHERE dataset_id=?1 AND case_id=?2",params![dataset_id,id]).map_err(|e|e.to_string())?;}tx.commit().map_err(|e|e.to_string())?;Ok(())}
#[tauri::command(rename_all = "camelCase")]
fn set_rows_status(app: tauri::AppHandle,dataset_id:String,case_ids:Vec<String>,status:String)->Result<(),String>{if !matches!(status.as_str(),"Pending"|"Approved"|"Completed"){return Err("Invalid verification status.".to_string());}let conn=connection(&app)?;let tx=conn.unchecked_transaction().map_err(|e|e.to_string())?;for id in case_ids{tx.execute("UPDATE imported_rows SET verification_status=?1 WHERE dataset_id=?2 AND case_id=?3",params![status,dataset_id,id]).map_err(|e|e.to_string())?;}tx.commit().map_err(|e|e.to_string())?;Ok(())}
#[tauri::command(rename_all = "camelCase")]
async fn clear_row_store(app: tauri::AppHandle,dataset_id:String)->Result<(),String>{
    tauri::async_runtime::spawn_blocking(move || {
        let conn = connection(&app)?;
        clear_dataset(&conn,&dataset_id)
    }).await.map_err(|e| format!("Unable to schedule row-store cleanup: {e}"))?
}
#[tauri::command(rename_all = "camelCase")]
fn clone_row_store(app: tauri::AppHandle,source_dataset_id:String,target_dataset_id:String)->Result<usize,String>{let conn=connection(&app)?;clear_dataset(&conn,&target_dataset_id)?;let count=conn.execute("INSERT INTO imported_rows(dataset_id,row_order,serial_no,case_id,pan,name,dob_doi,mobile,email,pin_code,address,state,verification_status,information_fy,information_source_type,information_source_description,information_type,information_description,information_value,source,finding,actionable_ay,statutory_reason,verification_result_type,income_escaping_assessment_value,verification_information_value,result_description) SELECT ?1,row_order,serial_no,case_id,pan,name,dob_doi,mobile,email,pin_code,address,state,verification_status,information_fy,information_source_type,information_source_description,information_type,information_description,information_value,source,finding,actionable_ay,statutory_reason,verification_result_type,income_escaping_assessment_value,verification_information_value,result_description FROM imported_rows WHERE dataset_id=?2",params![target_dataset_id,source_dataset_id]).map_err(|e|e.to_string())?;Ok(count)}
#[tauri::command(rename_all = "camelCase")]
fn rename_row_store(app: tauri::AppHandle,source_dataset_id:String,target_dataset_id:String)->Result<usize,String>{let conn=connection(&app)?;if source_dataset_id!=target_dataset_id{clear_dataset(&conn,&target_dataset_id)?;conn.execute("UPDATE imported_rows SET dataset_id=?1 WHERE dataset_id=?2",params![target_dataset_id,source_dataset_id]).map_err(|e|e.to_string())?;}conn.query_row("SELECT COUNT(*) FROM imported_rows WHERE dataset_id=?1",params![target_dataset_id],|r|r.get::<_,i64>(0)).map(|v|v as usize).map_err(|e|e.to_string())}
#[tauri::command(rename_all = "camelCase")]
fn seed_row_store(app: tauri::AppHandle,dataset_id:String,rows:Vec<NativePersonRowInput>)->Result<usize,String>{let conn=connection(&app)?;clear_dataset(&conn,&dataset_id)?;let tx=conn.unchecked_transaction().map_err(|e|e.to_string())?;for row in &rows{execute_upsert_tx(&tx,&dataset_id,row)?;}tx.commit().map_err(|e|e.to_string())?;Ok(rows.len())}
#[tauri::command(rename_all = "camelCase")]
fn export_csv_file(app: tauri::AppHandle,dataset_id:String,suggested_name:String)->Result<Option<String>,String>{let Some(path)=rfd::FileDialog::new().set_file_name(&suggested_name).add_filter("CSV files",&["csv"]).save_file() else{return Ok(None);};let conn=connection(&app)?;let file=File::create(&path).map_err(|e|format!("Unable to create CSV file: {e}"))?;let mut writer=BufWriter::with_capacity(1024*1024,file);writeln!(writer,"PAN,Name,DOB/DOI,Mobile,E-Mail,PIN Code,Address,State,FY,Information Type,Findings,Source,Information Value,Description,Actionable AY,Verification Result Type,Statutory Reason,Income Escaping Assessment Value,Verification Information Value").map_err(|e|e.to_string())?;let mut stmt=conn.prepare("SELECT pan,name,dob_doi,mobile,email,pin_code,address,state,information_fy,information_type,finding,source,information_value,information_description,actionable_ay,verification_result_type,statutory_reason,income_escaping_assessment_value,verification_information_value FROM imported_rows WHERE dataset_id=?1 ORDER BY row_order").map_err(|e|e.to_string())?;let mut rows=stmt.query(params![dataset_id]).map_err(|e|e.to_string())?;while let Some(row)=rows.next().map_err(|e|e.to_string())?{let mut fields=Vec::with_capacity(19);for i in 0..19{let value:String=row.get(i).map_err(|e|e.to_string())?;fields.push(format!("\"{}\"",value.replace('"',"\"\"")));}writeln!(writer,"{}",fields.join(",")).map_err(|e|e.to_string())?;}writer.flush().map_err(|e|e.to_string())?;Ok(Some(path.to_string_lossy().to_string()))}

#[tauri::command]
fn save_draft(app: tauri::AppHandle, request: SaveDraftRequest) -> Result<SavedDraftSummary, String> {
    let root=storage_root(&app)?; let conn=connection(&app)?; let draft:Value=serde_json::from_str(&request.draft_json).map_err(|e|format!("Invalid draft JSON: {e}"))?;
    if draft.get("version").and_then(Value::as_i64).unwrap_or_default()!=1{return Err("Unsupported draft version.".to_string());}
    let reference_number=draft.get("packet").and_then(|p|p.get("referenceNumber")).and_then(Value::as_str).unwrap_or("").to_string();
    let step=draft.get("step").and_then(Value::as_i64).unwrap_or(1); let row_count=draft.get("rowCount").and_then(Value::as_u64).or_else(||draft.get("rows").and_then(Value::as_array).map(|r|r.len() as u64)).unwrap_or(0);
    let updated_at=draft.get("savedAt").and_then(Value::as_str).unwrap_or("").to_string(); let draft_id=request.draft_id.filter(|id|!id.trim().is_empty()).or_else(||draft.get("draftId").and_then(Value::as_str).map(ToOwned::to_owned)).unwrap_or_else(||format!("draft-{}",uuid_like()));
    let working_store_id=draft.get("rowStoreId").and_then(Value::as_str).unwrap_or("").to_string();
    if !working_store_id.is_empty(){let current_count:i64=conn.query_row("SELECT COUNT(*) FROM imported_rows WHERE dataset_id=?1",params![working_store_id],|r|r.get(0)).map_err(|e|e.to_string())?;if current_count as u64!=row_count{return Err(format!("Draft row-store mismatch: expected {row_count} rows but found {current_count}."));}rename_row_store(app.clone(),working_store_id,draft_id.clone())?;}
    let dir=root.join("drafts").join(&draft_id);if dir.exists(){fs::remove_dir_all(&dir).map_err(|e|format!("Unable to replace saved draft files: {e}"))?;}fs::create_dir_all(dir.join("documents")).map_err(|e|format!("Unable to create draft directory: {e}"))?;
    let mut draft_value=draft;if let Some(obj)=draft_value.as_object_mut(){obj.insert("draftId".to_string(),Value::String(draft_id.clone()));obj.insert("rowStoreId".to_string(),Value::String(draft_id.clone()));obj.insert("rowCount".to_string(),Value::Number(serde_json::Number::from(row_count)));if let Some(rows)=obj.get_mut("rows"){*rows=Value::Array(Vec::new());}}
    let draft_file=dir.join("draft.json");fs::write(&draft_file,serde_json::to_string_pretty(&draft_value).map_err(|e|e.to_string())?).map_err(|e|format!("Unable to write draft file: {e}"))?;
    let mut first_file_name=None;let mut first_file_size=None;for file in request.files{let safe_name=sanitize_file_name(&file.name);let local_path=dir.join("documents").join(format!("{}-{}",file.id,safe_name));if let Some(source)=file.file_path.as_ref().filter(|v|!v.trim().is_empty()){if !Path::new(source).exists(){return Err(format!("Unable to save document reference {}: file no longer exists.",file.name));}if first_file_name.is_none(){first_file_name=Some(file.name.clone());first_file_size=fs::metadata(source).ok().map(|m|m.len());}}else{fs::write(&local_path,&file.bytes).map_err(|e|format!("Unable to save document {}: {e}",file.name))?;if first_file_name.is_none(){first_file_name=Some(file.name);first_file_size=Some(file.bytes.len() as u64);}}}
    let timestamp=if updated_at.is_empty(){now_iso()}else{updated_at.clone()};conn.execute("INSERT INTO saved_draft(id,file_path,reference_number,row_count,step,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(id) DO UPDATE SET file_path=excluded.file_path,reference_number=excluded.reference_number,row_count=excluded.row_count,step=excluded.step,updated_at=excluded.updated_at",params![draft_id,draft_file.to_string_lossy(),reference_number,row_count as i64,step,timestamp,timestamp]).map_err(|e|format!("Unable to save draft index in SQLite: {e}"))?;
    // upload_draft is auxiliary state used by the upload flow. A draft must not become
    // un-saveable merely because an older local POC database has a legacy upload_draft schema.
    // saved_draft + draft.json are the authoritative local draft state.
    if let Err(error) = conn.execute("INSERT INTO upload_draft(id,upload_type,local_status,selected_file_name,selected_file_path,selected_file_size,created_at,updated_at) VALUES(?1,'LOCAL_DRAFT','SAVED',?2,?3,?4,?5,?6) ON CONFLICT(id) DO UPDATE SET local_status='SAVED',selected_file_name=excluded.selected_file_name,selected_file_path=excluded.selected_file_path,selected_file_size=excluded.selected_file_size,updated_at=excluded.updated_at",params![draft_id,first_file_name,draft_file.to_string_lossy(),first_file_size.map(|v|v as i64),timestamp,timestamp]) {
        eprintln!("Unable to update auxiliary upload_draft state: {error}");
    }
    Ok(SavedDraftSummary{id:draft_id,reference_number,updated_at:timestamp,row_count:row_count as usize,step,file_path:Some(draft_file.to_string_lossy().to_string())})
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
    clear_dataset(&conn, &draft_id)?;
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
            pick_file,pick_supporting_documents,read_file_bytes,pick_csv_file,import_csv_to_store,get_row_page,get_row_by_case_id,
            commands::data_commands::get_runtime_config,commands::data_commands::get_row_window,commands::data_commands::stream_row_window,
            upsert_row,delete_rows,set_rows_status,clear_row_store,clone_row_store,rename_row_store,seed_row_store,export_csv_file,
            save_draft,list_drafts,load_draft,delete_draft,get_storage_location,save_export_file,cleanup_temp_files,get_app_version,open_government_website
        ])
        .run(tauri::generate_context!())
        .expect("error while running Insight Data Upload Utility");
}
