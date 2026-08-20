# Insight Data Upload Utility — production frontend architecture

## Current million-row ingestion architecture

The desktop CSV path is intentionally native-first:

```text
CSV file
  |
  v
Tauri 2 async command
  |
  v
Rust worker (spawn_blocking)
  |
  +--> streaming CSV parser
  +--> bounded SQLite transactions
  +--> row-order index
  +--> progress events
  |
  v
SQLite native row store
  |
  +--> get_row_page(LIMIT/OFFSET)
  |
  v
Angular keeps only the current page (default 50)
```

A synchronous Rust command was the primary reason the previous build could become
"Not Responding": Tauri documents that commands without `async` execute on the main
thread, while async commands are scheduled separately and are preferred for heavy work.

The import command therefore **starts** a native worker and returns immediately. The worker
emits progress/complete/error events. The first configured batch is committed and indexed so
the utility can show the first page while the remaining CSV continues importing.

## Performance rules

- Never read a multi-million-row CSV into a JavaScript string.
- Never create a JavaScript array containing the entire dataset.
- Never send the complete dataset through IPC.
- Never run long CSV parsing / SQLite writes in a synchronous Tauri command.
- Keep one bounded page in Angular state.
- Maintain only the row-order index required for deterministic paging during import.
- Do not maintain multiple secondary indexes while inserting millions of rows.
- Use WAL + NORMAL synchronous mode for the production import transaction.
- Emit progress at configured row intervals.
- The import batch size, progress interval, SQLite cache budget and maximum CSV size
  are runtime-configured in `src/assets/config/app-config.json`.

## Table rendering

The current screen uses **bounded native pagination**, not a two-million-row `ngFor`.
This is deliberate: the current UX is page-based and each page is limited to the configured
page size. A CDK virtual-scroll viewport can be added only if the product changes from
pagination to continuous scrolling.

## Runtime configuration

`src/assets/config/app-config.json` is the runtime source for API endpoints, CSV import
capacity, pagination, feature flags and other environment/capacity controls.

## Backend compatibility

The API client must map only to the supplied OpenAPI contract. No undocumented
Spring Boot endpoint should be invented.

Large file upload uses the backend presigned-upload flow whenever the supplied API contract
provides a signed upload URL, so large file bytes do not pass through the Spring application tier.

## Tauri 2

Angular IPC uses Tauri 2 APIs:

- `@tauri-apps/api/core` for `invoke`
- `@tauri-apps/api/event` for bounded progress events
- `@tauri-apps/api/window` for window lifecycle integration

Do not use Tauri v1 imports.

## Verification

Use the actual client build environment to run:

```text
npm ci --legacy-peer-deps
npm run build
npm run tauri:build
```

The sandbox used for this update does not contain the Rust toolchain and could not complete
a fresh npm dependency installation, so a native release build could not be honestly claimed here.
TypeScript source syntax was parsed successfully with the available TypeScript compiler.
