#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[tauri::command]
fn pick_file() -> Result<Option<(String, Vec<u8>)>, String> {
    let Some(path) = rfd::FileDialog::new()
        .set_title("Select supporting document")
        .pick_file()
    else {
        return Ok(None);
    };

    let metadata = std::fs::metadata(&path).map_err(|error| error.to_string())?;
    const MAX_FILE_SIZE: u64 = 25 * 1024 * 1024;
    if metadata.len() > MAX_FILE_SIZE {
        return Err("Maximum allowed file size is 25 MB.".to_string());
    }

    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "The selected file has an invalid name.".to_string())?
        .to_string();
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;

    Ok(Some((name, bytes)))
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![pick_file])
        .run(tauri::generate_context!())
        .expect("error while running Insight Data Upload Utility");
}
