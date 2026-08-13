# Offline-resilient uploads: running and testing it locally

`insight-ui` (the Angular frontend, a sibling repo at `../insight-ui`) tracks every upload's
progress in the browser's IndexedDB and watches real connectivity to `upload-service`, so an
upload survives a dropped network — pausing automatically and resuming once the connection is
back — and survives the tab/browser closing outright, by picking up again once you reselect the
same file. See `docs/resumable-uploads.md` in this repo for the server-side multipart contract
this builds on; this doc covers the client-side pieces and how to run/exercise them.

## 1. Start infra + upload-service

Only `upload-service` and its infra are needed to run and test this feature — you don't need
the full 5-service stack (see `docs/RUNNING_LOCALLY.md` if you want that too).

```bash
cd insight-data-upload-platform
docker compose up -d postgres kafka minio minio-init
docker compose ps          # postgres and minio should show (healthy)

mvn -pl common-library,protos,upload-service -am install -DskipTests
mvn -pl upload-service -Dspring-boot.run.fork=false spring-boot:run
```

Confirm it's up:
```bash
curl http://localhost:8081/actuator/health   # {"status":"UP"}
```

## 2. Start insight-ui — **must be port 4200**

```bash
cd ../insight-ui
npm install
npm start
```

(`npm start` runs `ng serve` on the default port 4200 — see `package.json`. The Angular CLI
here is only a local dependency, not installed globally, so a bare `ng serve` will fail with
`command not found: ng` unless you have the CLI installed globally yourself. `npx ng serve`
works too, if you want CLI flags like `--port`.)

Open `http://localhost:4200`. The port matters, not just as a convention: both MinIO's CORS
(`docker-compose.yml`, `MINIO_API_CORS_ALLOW_ORIGIN`) and `upload-service`'s CORS
(`WebConfig.java` and the `management.endpoints.web.cors` block in `application.yml`) are
hardcoded to allow only `http://localhost:4200`. Serving on any other port — `ng serve --port
4300`, for example — will silently fail: the API calls get CORS-blocked and the app just shows
"offline" forever.

## 3. Sanity check

With both up, the page should load with **no offline banner** and no "Incomplete uploads"
section (empty IndexedDB on a fresh browser profile). Pick a file and click **Start upload**:
- Files under the multipart threshold (`insight.storage.multipart-threshold-bytes`, 10MB
  locally) go single-shot — one PUT, done.
- Larger files go multipart — you'll see the **Parts** table fill in and the part size grow
  adaptively (visible in the **Log** panel).

## 4. Exercising the offline/interrupt scenarios

**Manual pause/resume** (fastest to test, exercises the same code path as an offline pause):
pick a file large enough to have several parts, click **Pause** mid-upload, watch phase become
`paused (manual)`, click **Resume** — it re-asks the server (`GET /parts`) what's already landed
before continuing, rather than trusting anything held in the browser.

**Real network drop, same tab** — DevTools → Network tab → set throttling to **Offline**:
- The banner appears within ~1s (browser's own `offline` event).
- An in-flight part PUT aborts immediately rather than hanging.
- Progress is safe — check IndexedDB (DevTools → Application → IndexedDB →
  `insight-uploads` → `sessions`) and you'll see `nextOffsetBytes`/`nextPartNumber` reflecting
  exactly the last completed part.
- Set throttling back to **Online** — the upload resumes on its own, no click needed.

**Backend unreachable but network "up"** (what `navigator.onLine` alone can't catch) — stop
`upload-service` (`Ctrl+C` in its terminal) while the browser stays connected to the internet:
the banner still appears, because `ConnectivityService` backs the browser event with an active
poll of `/actuator/health` every ~3–10s. Restart `upload-service` and the banner clears and any
paused-offline upload resumes.

**Tab/browser closed or crashed mid-upload:**
1. Start a multipart upload, pause it (manually or by going offline).
2. Reload the page (or close and reopen the tab) — this is what makes the in-memory `File`
   handle disappear; IndexedDB is what survives it.
3. The "Incomplete uploads" panel lists the session with its last-known progress %.
4. Click **Resume**, reselect the *same* file. The app checks the name and size match, then
   resumes from wherever `GET /parts` (i.e. what's actually in MinIO) says you left off — not
   from the IndexedDB record, which is a hint, not the source of truth.

Completing an upload (`/complete` succeeds) removes its IndexedDB record — an "Incomplete
uploads" entry means exactly that: incomplete.

## 5. Where the pieces live

| Piece | File |
|---|---|
| IndexedDB session store | `insight-ui/src/app/upload-store.service.ts` |
| Connectivity detection | `insight-ui/src/app/connectivity.service.ts` |
| Upload driver (persistence, pause/retry/backoff) | `insight-ui/src/app/upload-client.service.ts` |
| UI (banner, incomplete-uploads list, resume flow) | `insight-ui/src/app/app.component.ts` / `.html` |
| Session record shape | `UploadSessionRecord` in `insight-ui/src/app/models.ts` |
| Actuator CORS (backend) | `upload-service/src/main/resources/application.yml`, `management.endpoints.web.cors` |

**Design notes, briefly** (IndexedDB stores metadata only, never file bytes — see the record
shape above):
- Same-tab network blips resume fully automatically — the `File` object never left memory.
- A closed tab/crash needs the file reselected, since a `File` handle can't be reconstructed
  from disk without the browser's own picker. Name+size match is a heuristic, not a checksum.
- No cross-tab locking — driving the same upload from two tabs isn't guarded against.
- IndexedDB can be evicted under storage pressure; since it's metadata-only, the worst case is
  losing the resume shortcut, not data loss (MinIO/`GET /parts` is always the real source of
  truth).

## Troubleshooting

- **Banner says offline even though `curl http://localhost:8081/actuator/health` works.**
  `curl` isn't subject to CORS — the browser is. Actuator endpoints are served by a separate
  handler mapping in Spring Boot that does **not** consult `WebConfig`'s
  `WebMvcConfigurer#addCorsMappings` (that only covers regular `@RestController` routes) — they
  need their own `management.endpoints.web.cors.*` config, which is what's in `application.yml`.
  If you ever add another actuator-exposed endpoint the browser needs to reach, it's covered by
  the same block — no per-endpoint CORS registration needed.
- **Banner flickers or won't clear even with the backend up.** Check you're serving `insight-ui`
  on port 4200 (see §2) — anything else gets CORS-blocked identically.
