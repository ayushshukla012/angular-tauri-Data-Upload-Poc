# CSV Import Performance Fix

## Root cause found

The previous desktop import path had four performance problems:

1. `import_csv_to_store` was a synchronous Tauri command. Tauri documents that commands
   without `async` execute on the main thread. A long-running CSV parse + SQLite write on that
   thread can make the Windows application show `Not Responding`.
2. The import wrote every row into a temporary SQLite dataset and then copied the rows into
   the final dataset. That doubles the database write path.
3. Four secondary indexes (`serial`, `PAN`, `name`, `email`) were maintained while millions
   of rows were inserted. Every row therefore updated multiple B-trees.
4. Page reads repeatedly used `COUNT(*)` and `CAST(serial_no AS INTEGER)`. The optimized path
   stores a numeric `row_order` and a dataset row-count metadata record.

## New flow

```text
Angular
  |
  | invoke(import_csv_to_store)
  v
Tauri async command
  |
  | returns immediately
  v
Rust spawn_blocking worker
  |
  +--> buffered CSV parser
  +--> validate current record
  +--> bounded SQLite transaction
  +--> update dataset_metadata
  +--> emit csv-import-progress
  |
  v
first ready batch
  |
  +--> create row-order index
  +--> Angular opens the utility screen
  |
  v
remaining rows continue importing
  |
  v
completed=true
```

## Current Angular memory contract

The native path never uses `File.text()` for the selected large CSV.

Angular receives only the current configured page from `get_row_page`.

The table therefore remains bounded even when `nativeTotalRows` reaches 2,000,000+.

## Runtime tuning

`src/assets/config/app-config.json` controls:

- `csvImport.batchRows`
- `csvImport.readyRows`
- `csvImport.progressRows`
- `csvImport.sqliteCacheMb`
- `csvImport.maxFileBytes`
- `pagination.defaultPageSize`
- `pagination.pageSizeOptions`

Do not hard-code these values in Rust or Angular.

## Important SLA note

The client SLA is:

- 2,000,000+ records
- target initial data readiness: <= 8 seconds
- UI must stay responsive

The updated code removes the architectural main-thread freeze and substantially reduces
SQLite write amplification.

The sandbox used for this update does not have a Rust toolchain, so a real Tauri release build
and a measured 2,000,000-row benchmark could not be honestly claimed here.

The supplied `samples/sample-data-100k.csv` is approximately 25 MB for 100,000 records.
That means a 2,000,000-row dataset of the same shape is approximately 500 MB. The final SLA
must therefore be measured on the client's target SSD/CPU/RAM configuration, because file size,
storage latency and CPU characteristics materially affect the absolute import time.

## Acceptance test

On the client build machine:

1. Build the Tauri application.
2. Import a 500,000-row CSV.
3. Import a 2,000,000-row CSV.
4. Record:
   - time to first visible page
   - time to 2,000,000 imported
   - rows/sec
   - UI responsiveness
   - process working-set memory
   - page navigation latency
   - search latency

The progress banner displays the measured rows/sec and final import duration, making the
actual SLA result visible rather than inferred.
