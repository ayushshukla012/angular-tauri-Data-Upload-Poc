# Insight Data Uploads Utility — Angular 22 + Tauri 2

This project is a standalone frontend implementation of the Data Upload Module shown in the supplied UI references. It uses Angular 22 for the web UI and Tauri 2 for Windows desktop packaging. The same Angular build can run in a normal browser at `http://localhost:4200` or be packaged as a Windows installer/executable through Tauri.

The UI follows the supplied flow:

1. New Application / Open Existing File.
2. Packet Details + submitting-person details.
3. Person & Verification Details table.
4. Add Row with Person, Information and Verification accordion sections.
5. CSV import with sample CSV download.
6. General Document attachment to all rows or selected rows.
7. Local draft persistence with IndexedDB and JSON draft import/export.
8. Final Create Packet flow.
9. Java Spring Boot API integration for packets/cases.
10. Direct-to-MinIO presigned upload flow for documents.

## Backend contract used

The frontend was built against the API contract supplied in the request and the repository at:

`https://github.com/himanshu885986/insight-2.0`

The backend exposes:

- `POST /api/v1/packets`
- `GET /api/v1/packets/{batchNumber}`
- `POST /api/v1/cases`
- `GET /api/v1/cases/{caseId}`
- `GET /api/v1/cases/{caseId}/documents`
- `POST /api/v1/uploads/initiate`
- `POST /api/v1/uploads/{uploadId}/parts/{partNumber}/presign`
- `GET /api/v1/uploads/{uploadId}/parts`
- `POST /api/v1/uploads/{uploadId}/complete`
- `GET /api/v1/uploads/{uploadId}`

The upload service is designed to return a presigned URL and have the client send file bytes directly to MinIO. The backend documentation also describes multipart/resumable upload behavior.

## Important integration note: `designation`

The supplied `SubmitCaseRequest` API requires `designation`, while the supplied Add Row screens do not show a designation field. To avoid inventing a new UI field and to keep the reference UI unchanged, the frontend sends a configurable technical fallback from `src/environments/environment.ts`:

```ts
defaultCaseDesignation: 'Data Upload Utility'
```

This is an integration fallback, not a claimed business value. Change it to the value your backend/domain requires before production use.

## Prerequisites

Angular 22's current compatibility guidance requires Node.js `22.22.3+` for the Angular 22 line. The local environment used while preparing this package has Node.js 22.16.0, so the Angular/Tauri toolchain could not be executed here without upgrading Node. See the official Angular version compatibility table before installing dependencies.

For Tauri 2 on Windows you also need Rust and the Windows build prerequisites. Tauri's documentation recommends the standard `create-tauri-app`/CLI workflow, and production builds use the system WebView2 runtime.

Recommended versions for this POC:

- Node.js: 22.22.3 or newer
- npm: current npm shipped with that Node release
- Angular: 22.x
- Rust: current stable Rust toolchain supported by Tauri 2
- Visual Studio Build Tools: Desktop development with C++
- WebView2 Runtime: installed on the Windows machine

## Run the frontend in a browser

From this project folder:

```powershell
npm install
npm start
```

Open:

`http://localhost:4200`

The supplied backend configuration specifically allows `http://localhost:4200` for browser development, so keep that port unless you update the backend CORS configuration.

## Run the backend

The backend repository is the source of truth for the Java services. A minimal local run from the supplied repository is:

```powershell
cd insight-2.0
podman-compose up -d postgres kafka minio minio-init
mvn -pl common-library,protos,upload-service -am install -DskipTests
mvn -pl upload-service -Dspring-boot.run.fork=false spring-boot:run
```

The supplied backend configuration uses:

- Postgres: `localhost:5433`
- Kafka: `localhost:9092`
- MinIO API: `http://localhost:9000`
- MinIO console: `http://localhost:9001`
- Upload service REST API: `http://localhost:8081`
- Upload service gRPC: `9095`
- Kafka UI: `http://localhost:8090`

The expected local bucket is `insight-uploads`.

Health check:

```powershell
curl http://localhost:8081/actuator/health
```

## Browser CORS

The repository currently hardcodes API CORS to `http://localhost:4200` in `upload-service/src/main/java/com/insight/upload/WebConfig.java`, and the actuator CORS block also allows `http://localhost:4200`.

This project intentionally does not change the backend repository automatically. For browser development, keep the frontend on port 4200.

## Tauri development

After installing the Node dependencies and Rust prerequisites:

```powershell
npm install
npm run tauri:dev
```

Tauri will start Angular using the `beforeDevCommand` in `src-tauri/tauri.conf.json` and load `http://localhost:4200`.

## Package a Windows MSI / EXE installer

Run:

```powershell
npm run tauri:build
```

The Tauri configuration requests both Windows NSIS and MSI targets. The output is placed under `src-tauri/target/release/bundle/`.

Typical outputs include:

- NSIS installer: `src-tauri/target/release/bundle/nsis/*.exe`
- MSI installer: `src-tauri/target/release/bundle/msi/*.msi`

These are installers. The application executable itself is also produced in the Rust release target directory.

## Tauri production CORS requirement

A packaged Tauri app uses a local app origin rather than `http://localhost:4200`. Tauri 2 documents the production custom origin as `http://tauri.localhost` on Windows when the default HTTP scheme is used.

Because the backend's CORS currently permits only `http://localhost:4200`, direct calls from the packaged desktop app will require a backend CORS update.

Use the supplied patch example in:

`docs/TAURI_CORS_PATCH.md`

Do not broaden CORS to `*` for this application.

## File upload architecture

The frontend follows the backend contract instead of posting file bytes through Spring Boot:

1. `POST /api/v1/uploads/initiate`
2. PUT the file directly to the returned MinIO presigned URL.
3. `POST /api/v1/uploads/{id}/complete`

For multipart uploads, the frontend uses the server's part-presign endpoints and queries existing parts before continuing.

The backend documentation says the object store is the source of truth for multipart resume state. The local IndexedDB draft is metadata/state for the UI, not the authoritative copy of file bytes.

## Draft behavior

The `Save as Draft` button writes the current packet, row metadata, and document metadata to IndexedDB. The `Open Existing File` dialog supports:

- restoring the last local draft; and
- opening an exported JSON draft file.

A JSON draft export does not embed document bytes. This is deliberate: document content should stay in local storage or MinIO rather than being copied into a portable JSON record.

## CSV behavior

CSV import supports quoted fields and maps the following families of columns:

- PAN / Source PAN
- Name
- DOB/DOI
- Mobile
- E-Mail / Email
- PIN Code / Pincode
- Address
- State / State-UT
- Verification Status
- Information fields
- Verification fields

A sample file is available at:

`src/assets/sample-data-upload.csv`

The UI also provides a `Download sample CSV` action directly from the import dialog.

## Documents

The General Document dialog supports:

- Document Type
- Description
- file selection
- All Rows attachment
- Selected Rows attachment
- editing/removing the document metadata

When the packet is finally submitted, a document attached to multiple case rows is uploaded once per target case because the supplied backend contract does not expose an endpoint for linking an existing upload to a second case without a new upload.

## Concurrency

Final case submission uses a small concurrency pool (4 workers) so multiple case rows can be submitted and their associated documents uploaded in parallel without starting an unbounded number of requests.

Adjust the concurrency in `createPacket()` only after measuring the real backend/load limits.

## Project structure

```text
insight-data-upload-ui/
├─ src/
│  ├─ app/
│  │  ├─ app.component.ts
│  │  ├─ app.component.html
│  │  ├─ app.component.css
│  │  ├─ models.ts
│  │  └─ services/
│  │     ├─ api.service.ts
│  │     ├─ draft-store.service.ts
│  │     └─ toast.service.ts
│  ├─ assets/sample-data-upload.csv
│  ├─ environments/environment.ts
│  ├─ index.html
│  ├─ main.ts
│  └─ styles.css
├─ src-tauri/
│  ├─ Cargo.toml
│  ├─ build.rs
│  ├─ src/main.rs
│  └─ tauri.conf.json
├─ angular.json
├─ package.json
└─ README.md
```

## Validation performed in this environment

The package was assembled and statically reviewed against the API contract supplied in the request. GitHub repository contents were also inspected for the backend architecture, offline-upload design, and the current `WebConfig.java` CORS rules.

A full Angular/Tauri build could not be executed in this sandbox because:

- Node.js is `22.16.0`, below the current Angular 22 compatibility floor of `22.22.3`; and
- Rust/Cargo are not installed in the sandbox.

Therefore this package is not being presented as a fully binary-tested Angular/Tauri build. Install the current prerequisites above, run `npm install`, then `npm run build` and `npm run tauri:build` on the target Windows development machine.

A synthetic supporting document for exercising the attachment UI is also provided at `src/assets/sample-supporting-document.txt`.
