# Insight Data Uploads Utility — Angular + Tauri 2 + SQLite

This is the updated working UI POC for the Data Uploads Utility. The existing Angular/Tauri UI and backend integration were preserved, and the requested row, draft, CSV and verification-status behaviors were added without implementing new backend business logic.

## What was changed

### 1. Open Existing File
`Open Existing File` now reads the locally saved draft index from SQLite and shows the saved drafts available on the current machine.

Each saved draft can be:

- Opened and restored into the UI.
- Deleted from local storage.
- Identified by reference/file number, row count, saved time and draft id.

An `Import exported draft JSON` fallback is still available so existing exported JSON drafts do not stop working.

### 2. Save as Draft — SQLite + File System
The desktop/Tauri application stores drafts in the application's OS-managed data directory.

The implementation uses:

- **SQLite:** local draft index/state and the required POC local tables.
- **File System:** draft JSON plus references to user-selected supporting documents. The updated desktop flow avoids copying/reading every supporting file into memory; the original selected path is retained and the file is read one-at-a-time only when the packet is submitted.

The Tauri app exposes the actual location through the Open Existing File dialog.

On Windows, because the Tauri application identifier is `com.insight.datauploads.utility`, the directory is under the Windows application-data area for that identifier (typically under `%LOCALAPPDATA%`). The UI displays the resolved path rather than relying on a hardcoded path.

Typical structure:

```text
<app-data>/
├── data-upload-poc.sqlite3
└── drafts/
    └── <draft-id>/
        └── draft.json
```

Legacy drafts that already contain locally copied document bytes may still contain a `documents/` directory. New drafts prefer storing the user-selected document path in the draft metadata and do not duplicate large files.

> POC note: local draft files may contain user-provided business data. Do not use this local persistence design as the production storage/security boundary. Production credential/secret storage must use an approved secure mechanism.

### 3. Edit button
The Packet Details `Edit` button now unlocks the packet fields after the screen has been made read-only for the next step. It no longer resets or destroys the entered packet data.

Row-level editing is also available through the pencil action in the table. Completed rows are locked.

### 4. Add Row
The Add Row form now requires all mandatory Person, Information and Verification fields before saving.

Information Details required fields:

- FY
- Information Type
- Findings
- Source
- Information Value

Verification Details required fields:

- Actionable AY
- Verification Result Type
- Statutory Reason
- Income Escaping Assessment Value
- Information Value

The bottom actions are now:

- **Cancel** — closes the form without adding/updating the row.
- **Save** — saves only when the required details are valid.

### 5. General Document
`General Document` is disabled until at least one row is selected.

The dialog now supports selecting **one or multiple documents at once** for the currently selected row(s). Each selected document becomes its own attached-document record. The selected-row/all-row behavior remains available.

For Tauri desktop mode:

- The native file picker returns the file path and metadata without loading every document into memory.
- Supporting documents are read one-at-a-time only when the packet is submitted.
- The existing backend presigned MinIO upload flow is reused; no new backend storage API is invented.
- Multiple documents can therefore be attached to one row, and the same design can be used across large row sets without keeping every document's bytes resident in Angular memory.

### 6. Verification Status
The row status arrow/action is now a status dropdown:

- Pending
- Approved
- Completed

Rules implemented in the UI:

- New rows start as `Pending`.
- `Pending` can be changed to `Approved`.
- Only `Approved` rows can be validated.
- Select one or more Approved rows and click **Validate** to convert them to `Completed`.
- Completed rows cannot be edited and their status control is disabled.
- Pending/Completed rows selected together with Approved rows do not get changed by validation; the user is told to select only Approved rows.

The backend remains responsible for final business validation and acceptance.

### 7. CSV Export
`Export CSV` exports the exact Add Row input format used by the UI.

The exported format contains:

```text
PAN
Name
DOB/DOI
Mobile
E-Mail
PIN Code
Address
State
FY
Information Type
Findings
Source
Information Value
Description
Actionable AY
Verification Result Type
Statutory Reason
Income Escaping Assessment Value
Verification Information Value
```

The Tauri desktop build opens a native save dialog. Browser mode falls back to a normal browser download.

### 8. CSV Import
`Import CSV` now:

1. Lets the user select a CSV file from the system.
2. Verifies the header row and required column order.
3. Streams CSV records from the loaded text instead of first building a complete 2-D CSV array.
4. Validates required values and the current UX formats for PAN/mobile/email/PIN.
5. Adds all rows in a single Angular signal update after successful parsing.
6. Keeps pagination available for large datasets.
7. Uses compact pagination controls instead of rendering thousands of page buttons.
8. Yields periodically while processing large imports so the UI can continue to respond.

The current UI file-size ceiling remains **25 MB**, matching the supplied Data Uploads Utility screen. The included `samples/sample-data-100k.csv` is a 100,000-row file designed to fit inside that limit.

Only the current page is rendered in the table; the application does not render 100,000 DOM rows at once.

## 100,000-row performance sample

The ZIP includes:

```text
samples/sample-data-100k.csv
```

It contains exactly 100,000 data rows plus the CSV header and uses the same 19-column format expected by `Import CSV`.

It is intentionally kept within the current 25 MB UI file-size limit so it can be selected directly from the application.

The sample can also be regenerated/modified independently; the application does not depend on it at runtime.

## Existing backend integration preserved

The existing Angular API service continues to use the backend contract already present in the project. No new backend endpoint has been invented.

The UI uses the existing upload flow:

```text
POST /api/v1/packets
POST /api/v1/cases
POST /api/v1/uploads/initiate
PUT presigned upload URL
POST /api/v1/uploads/{uploadId}/complete
```

Multipart/resumable upload support already present in the project remains unchanged.

## Tauri commands added/used

The desktop shell now provides:

- `pick_file()` (legacy/single-file command retained)
- `pick_supporting_documents()` (native multi-document selection)
- `read_file_bytes()` (reads one selected document only when needed for upload)
- `save_draft()`
- `list_drafts()`
- `load_draft()`
- `delete_draft()`
- `get_storage_location()`
- `save_export_file()`
- `cleanup_temp_files()`
- `get_app_version()`

These commands only handle local desktop concerns. They do not implement backend business workflow.

## SQLite POC tables

The Tauri layer initializes the required local POC tables:

- `app_config`
- `upload_draft`
- `api_retry_queue`

A small additional `saved_draft` index table is used to make the `Open Existing File` screen practical for multiple saved drafts. It only indexes the local draft files; it is not a business source of truth.

## Browser-only mode

Browser mode remains available with `npm start`.

Because a browser does not have the Tauri local filesystem bridge, `DraftStoreService` uses a local-storage fallback in browser mode. The Windows/Tauri application uses SQLite + File System as requested.

## Setup

### Prerequisites

- Node.js compatible with the Angular version in `package.json`.
- Rust stable toolchain.
- Tauri 2 Windows prerequisites.
- Windows WebView2 runtime for packaged Windows execution.

### Install

```powershell
npm install
```

### Run Angular in browser mode

```powershell
npm start
```

### Run the Tauri desktop application

```powershell
npm run tauri:dev
```

### Build Angular

```powershell
npm run build
```

### Build Windows installer

```powershell
npm run tauri:build
```

## Manual verification performed on the updated source

The source was manually reviewed after the requested changes and the relevant pure logic was exercised separately for:

- row required-field validation;
- Pending → Approved → Completed status transitions;
- completed-row edit protection;
- selected-row gating for General Document and Validate;
- exact CSV header validation;
- CSV quoting/escaping;
- streamed 100k-row CSV parsing logic;
- exact CSV header validation;
- large-row pagination behavior;
- single signal update after bulk import;
- one/multiple supporting-document attachment modeling;
- native document-path preservation and one-at-a-time materialization;
- draft JSON import structure and SQLite schema creation SQL.

A full Angular browser/Tauri binary build could not be executed in this sandbox because the environment does not provide the complete installed Angular package set and Rust/Cargo toolchain. The package therefore does not claim a binary build was completed here.

## Files updated for this change set

```text
src/app/app.component.ts
src/app/app.component.html
src/app/app.component.css
src/app/models.ts
src/app/services/draft-store.service.ts
src-tauri/src/main.rs
src-tauri/Cargo.toml
README.md
```


## Latest functional fixes

- `Save as Draft` invokes the Tauri `save_draft` command with the required outer `request` argument and preserves the existing SQLite + File System storage design.
- Tauri direct command arguments use the required camelCase keys (`draftId`, `suggestedName`) for load/delete/export operations.
- CSV export falls back to the WebView/browser download path if the native save dialog is unavailable; the user is never blocked by a native-dialog-only failure.
- Verification Status UI exposes only `Pending` and `Approved` as selectable values. `Completed` is shown as a locked state after validation.
- Completed rows cannot be selected and cannot be edited, deleted, validated, or used for General Document actions.
