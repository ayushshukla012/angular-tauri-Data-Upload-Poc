# Data Upload Feature Context

The current Data Upload page is still implemented in `src/app/app.component.ts` and its template.
Do not split/rewrite the whole screen merely for a small bug fix.

Critical dataset rule:
- Desktop CSV data belongs in the native SQLite store.
- Angular keeps only the current configured page.
- A million-row dataset must never become a JS array.

CSV import lifecycle:
1. User selects a CSV path using the native picker.
2. Angular invokes `import_csv_to_store`.
3. The Tauri command returns immediately after scheduling the Rust worker.
4. Rust parses/writes in bounded batches.
5. Rust emits `csv-import-progress`.
6. The first configured ready batch makes the first page available.
7. Rust emits `csv-import-progress` with `completed=true` when the whole import finishes.
8. `csv-import-error` reports failure.

Do not revert to `File.text()` for desktop large files.
