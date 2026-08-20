# Tauri / Rust Context

The Rust layer owns heavy local CSV processing and SQLite storage.

Critical rule:
- Heavy commands must be `async` and schedule blocking CPU/IO work through a background worker.
- A synchronous heavy Tauri command can execute on the main thread and make the Windows app appear as `Not Responding`.

CSV import:
- `import_csv_to_store` is an async Tauri 2 command.
- It schedules a blocking worker with `tauri::async_runtime::spawn_blocking`.
- CSV is parsed with a buffered streaming reader.
- SQLite writes use bounded transactions.
- Secondary indexes are not maintained during the initial bulk load except the configured row-order index after the first ready batch.
- Progress is emitted with `csv-import-progress`.
- Failure is emitted with `csv-import-error`.

Do not use Tauri v1 APIs.
