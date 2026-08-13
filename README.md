# Data Upload System Application — Angular + Tauri POC

Desktop/system application POC for the Data Upload module. The frontend is implemented as an Angular application hosted by Tauri 2.x and uses Rust commands for native file selection and SQLite-backed local POC state.

## Reference-based implementation

### UI reference
The supplied screenshots were used as the visual reference for:
- Government of India blue header and footer
- Data Uploads Utility branding
- Three-step Packet Details → Person & Verification Details → Create Packet flow
- Person & Verification table
- General Document modal
- Add Row accordion/modal with Person, Information and Verification sections
- CSV/import/export action placement
- Status badges, warnings, pagination-style table layout and action buttons

The POC keeps the same information hierarchy and desktop-first visual language. It does not claim pixel-perfect reproduction of the screenshot because exact fonts, design tokens, icon assets and the original design system were not supplied.

### Backend repository reference
The supplied repository is public:
https://github.com/himanshu885986/insight-2.0

The inspected branch is `main`. The repository root POM currently declares **Java 21** and **Spring Boot 3.3.4**. This differs from the prompt's stated Spring Boot 3.2.4, so the actual repository values are treated as the reference.

The supplied repository's `upload-service` exposes these confirmed REST endpoints:

| Purpose | Confirmed endpoint |
|---|---|
| Initiate upload | `POST /api/v1/uploads/initiate` |
| Presign multipart part | `POST /api/v1/uploads/{uploadId}/parts/{partNumber}/presign` |
| List uploaded parts | `GET /api/v1/uploads/{uploadId}/parts` |
| Complete upload | `POST /api/v1/uploads/{uploadId}/complete` |
| Upload status | `GET /api/v1/uploads/{uploadId}` |
| Submit case metadata | `POST /api/v1/cases` |
| Get case | `GET /api/v1/cases/{caseId}` |
| Case documents | `GET /api/v1/cases/{caseId}/documents` |
| Submit packet | `POST /api/v1/packets` |
| Get packet | `GET /api/v1/packets/{batchNumber}` |

The supplied upload backend signs a storage PUT URL. For a non-multipart upload, the backend response supplies the exact `contentType` that must be used for the direct storage PUT.

## Responsibility split

### Angular
- UI, routing and Reactive Forms
- UX-only required-field validation
- API calls through `UploadApiService`
- Local draft/status presentation
- Backend response display

### Tauri/Rust
- Native desktop shell
- Native file picker
- Controlled native file access
- SQLite persistence for local POC state
- Optional direct PUT helper for a backend-provided presigned storage URL
- No backend business workflow

### SQLite / local File System
- Local backend URL configuration
- Local draft state
- Selected file metadata reference
- Local status/acknowledgement summary
- No production source of truth

### Backend
- Authentication/token validation
- RBAC
- Metadata persistence
- File upload initiation and storage signing
- Business validation
- Duplicate detection
- Workflow/approval
- Audit trail
- Profile linkage
- Final upload acceptance/status

## Important assumptions

1. The supplied backend repository is treated as the authoritative reference for endpoints that were actually found in the source.
2. The backend gateway URL is not specified by the supplied repository. The POC therefore accepts a configurable base URL. For the checked local `upload-service`, the configured value can be `http://localhost:8081`.
3. The prompt requests API Gateway/IAM, but no gateway or real IAM contract was found in the inspected repository files. Those parts remain placeholders.
4. `useMockBackend` is `true` by default so the UI can be demonstrated without a complete backend environment.
5. The screenshot's demo rows are static UI mock data only. They are not production records.

## TODOs

- Replace `useMockBackend: true` with `false` after the target environment is configured.
- Replace the token placeholder with the approved IAM/token flow and secure storage mechanism.
- Provide the real API Gateway base URL and authentication contract.
- Provide backend validation, acknowledgement, workflow, audit and profile-linkage endpoints if they differ from the supplied repository.
- Complete multipart upload orchestration for files above the backend's multipart threshold.
- Add approved production icon/font/design-system assets if available.
- Add approved production logging/telemetry policy.
- Add secure secret/token handling; never use plaintext SQLite/session storage for production secrets.
- Add cleanup policy for temporary files if the POC starts creating temporary copies.

## Project structure

```text
data-upload-system-app/
├── frontend/
│   ├── src/app/
│   │   ├── core/
│   │   │   ├── api/
│   │   │   ├── auth/
│   │   │   ├── models/
│   │   │   └── storage/
│   │   ├── shared/
│   │   └── features/
│   │       ├── login-token-placeholder/
│   │       ├── upload-dashboard/
│   │       ├── bulk-upload/
│   │       ├── single-upload/
│   │       ├── metadata-capture/
│   │       ├── file-selection/
│   │       ├── validation-results/
│   │       ├── upload-status/
│   │       └── acknowledgement/
│   ├── src-tauri/
│   │   ├── src/commands/
│   │   ├── src/db/
│   │   └── tauri.conf.json
│   └── package.json
├── backend/
│   ├── podman-compose.yml
│   ├── GET_FULL_REPO.ps1
│   ├── GET_FULL_REPO.sh
│   └── insight-2.0/
└── README.md
```

## Prerequisites

### Frontend / desktop
- Node.js compatible with the selected Angular version
- npm
- Rust stable toolchain
- Windows WebView2 for Windows desktop packaging
- Tauri CLI installed through the project dependency

### Backend
- JDK 21
- Maven
- Podman + Podman Compose (or Docker Compose if preferred)
- PostgreSQL/Kafka/MinIO dependencies from the supplied compose file

## Install frontend

```bash
cd frontend
npm install
```

## Run Angular browser-only mode

```bash
cd frontend
npm start
```

Browser-only mode uses fallback localStorage when Tauri SQLite/native commands are unavailable. This is only a development fallback and is not the production storage design.

## Run Tauri desktop mode

```bash
cd frontend
npm install
npx tauri dev
```

Tauri starts the Angular development server and renders it inside the native WebView.

## Build Angular

```bash
cd frontend
npm run build
```

## Build Windows installer

```bash
cd frontend
npx tauri build
```

The generated Windows installer/bundles are produced by Tauri under `frontend/src-tauri/target/release/bundle/`.

## Successful Execution & Build Commands

Below is the verified sequence of successful terminal commands used to setup, run, icon-generate, and package the application for frontend and backend:

### Frontend & Desktop App (Tauri + Angular)

```powershell
# 1. Navigate to frontend folder
cd frontend

# 2. Install NPM dependencies
npm install

# 3. Start Angular dev server (Browser-only mode)
npm start

# 4. Generate Tauri icons from source logo image (Creates icon.ico, icon.icns, PNG sizes)
npx tauri icon src-tauri\icons\app-icon.png

# 5. Run Tauri desktop application in development mode (Watches Angular frontend & Rust backend)
npx tauri dev

# 6. Build Angular production bundle
npm run build

# 7. Build Tauri production installers (Generates MSI installer & NSIS setup EXE)
npx tauri build
```

### Backend (Spring Boot & Microservices Infrastructure)

```powershell
# 1. Navigate to backend folder
cd backend

# 2. Download/Checkout full upstream repository (if not present)
.\GET_FULL_REPO.ps1

# 3. Spin up local infrastructure (PostgreSQL, Kafka, Kafka UI, MinIO)
podman-compose -f podman-compose.yml up -d

# 4. Navigate to spring boot project root
cd insight-2.0

# 5. Compile and package backend microservices
mvn clean install

# 6. Run the upload-service backend microservice (runs on port 8081)
mvn -pl upload-service spring-boot:run
mvn -pl upload-service spring-boot:run -Dspring-boot.run.jvmArguments="-Duser.timezone=UTC"
```

## Backend local infrastructure

The upstream repository's compose definition was inspected and copied into `backend/podman-compose.yml` for this package.

```bash
cd backend
podman-compose -f podman-compose.yml up -d
```

The supplied compose configuration provisions PostgreSQL, Kafka, Kafka UI and MinIO. The checked upload-service application configuration uses port `8081` and PostgreSQL on `5433`.

The actual complete upstream backend is not duplicated in this archive because the execution environment could not clone/download the full repository bytes. Run:

```powershell
cd backend
.\GET_FULL_REPO.ps1
```

or:

```bash
cd backend
./GET_FULL_REPO.sh
```

Then place the resulting `insight-2.0` checkout at `backend/insight-2.0/` and run the full Maven build from the upstream repository.

## Backend run/build

After the complete upstream repository is available:

```bash
cd backend/insight-2.0
mvn clean install
mvn -pl upload-service spring-boot:run
```

The POC upload-service application configuration indicates port `8081`.

## API integration

`frontend/src/app/core/api/upload-api.service.ts` is the single Angular API client surface.

It contains the required methods:

- `validateTokenPlaceholder()`
- `initiateUpload()`
- `saveMetadata()`
- `uploadFile()`
- `validateUpload()`
- `submitUpload()`
- `getUploadStatus()`
- `getAcknowledgement()`
- `getWorkflowStatus()`
- `getAuditTrail()`
- `getProfileLinkageStatus()`

Confirmed repository endpoints are used where the source provides them. Unknown endpoints remain explicit `TODO_*` placeholders.

## SQLite local POC model

The Rust layer creates:

- `app_config`
- `upload_draft`
- `api_retry_queue`

SQLite lives under the Tauri application data directory. It is never intended to be the final business data store.

## File-system behavior

- The native picker is opened only by explicit user action.
- The app does not scan user directories.
- Selected file metadata is persisted locally; the source file is not silently copied.
- A native helper exists for direct PUT to a backend-provided presigned URL.
- Temporary-copy cleanup is an explicit placeholder command.

## Security notes

- Do not put production tokens, passwords, private keys or credentials into SQLite.
- The visible token field is explicitly POC-only.
- Final authentication, authorization and token validation remain backend-owned.
- Local status/draft state must not be used to make business decisions.
- A backend response must be treated as the source of truth for validation and final status.

## Error handling covered

The UI has explicit handling paths for:

- backend unavailable
- invalid/unconfigured base URL
- unauthorized/token expiry
- missing file
- inaccessible file
- upload failure
- validation failure
- submit failure
- status failure
- SQLite/Tauri command failures

## Acceptance criteria

- [x] Angular UI and routing
- [x] Tauri 2.x shell configuration
- [x] Rust stable-oriented native command layer
- [x] Native file picker command
- [x] SQLite local POC schema
- [x] Draft persistence commands
- [x] Config persistence commands
- [x] Required screens/routes
- [x] Backend API client abstraction
- [x] Confirmed upload/case/packet API references from the supplied repo
- [x] Mock mode
- [x] Explicit TODOs for unknown backend contracts
- [x] Security responsibility split
- [x] README/setup/build instructions
- [ ] Full upstream backend bytes bundled in the zip — **not possible in the current execution environment; source retrieval scripts are included instead**
- [ ] Production IAM integration
- [ ] Production API Gateway integration
- [ ] Complete multipart direct-storage upload orchestration

## No-hallucination boundary

This POC intentionally does not invent backend validation rules, business workflows, audit semantics, profile linkage behavior, IAM implementation, or endpoint schemas that were not present in the supplied requirements or inspected backend source.
