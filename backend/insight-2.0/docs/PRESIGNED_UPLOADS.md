# Presigned uploads — what, why, and how the code implements it

Written for someone new to Java/Spring Boot — it explains both the concept and the Spring
mechanics used to build it, not just "call this endpoint."

## The problem this solves

The naive way to build a file upload API is: client sends the file as part of the HTTP
request body (`multipart/form-data`), your server receives it, your server writes it to
storage. That's what this project did at first — `POST /api/v1/uploads` took a `MultipartFile`
straight into the controller.

That falls over for large files (the original requirement was "millions of records," so
files can be huge):

1. **The file has to pass through your app server twice** — once from client to app (Tomcat
   buffers it to a temp file on disk), then again from app to object storage (MinIO/S3). A
   1 GB file means 2 GB of I/O and a slow, long-held HTTP connection, for no benefit — your
   app never actually needs to look at the bytes.
2. **It's easy to accidentally do slow work inside a database transaction.** The first version
   of this code called the (slow, network-bound) object-storage write *inside* the same
   `@Transactional` method as the database inserts — meaning a database connection sat
   checked out of the pool for as long as the upload took. A few large uploads at once would
   exhaust the connection pool.

## The fix: presigned URLs

A **presigned URL** is a regular HTTP URL with a cryptographic signature baked into its query
string (`X-Amz-Signature=...`) that says, in effect, "whoever holds this URL is allowed to PUT
an object to this exact key, until this timestamp, without needing any AWS/MinIO credentials
of their own." The object storage server (MinIO here) verifies the signature itself — your
application isn't involved in that check at all.

That means the app server's job shrinks to: **generate the signed URL, and record what it
expects to happen.** The actual bytes flow client → MinIO directly.

## The 3-step flow

```
Client                     upload-service                  MinIO                 orchestrator/transformation
  |                              |                            |                              |
  |--POST /uploads/initiate----->|                            |                              |
  |                              |--(no bytes touched)        |                              |
  |                              |--generates presigned URL   |                              |
  |                              |--saves Upload{PENDING}     |                              |
  |<--{uploadId, uploadUrl}------|                            |                              |
  |                                                           |                              |
  |--PUT <uploadUrl> + file bytes---------------------------->|                              |
  |<--200 OK--------------------------------------------------|                              |
  |                                                                                          |
  |--POST /uploads/{id}/complete->|                            |                              |
  |                              |--HEAD check: does the      |                              |
  |                              |  object actually exist?--->|                              |
  |                              |<--yes---------------------|                              |
  |                              |--Upload -> RECEIVED         |                              |
  |                              |--writes outbox event                                       |
  |<--{status: RECEIVED}---------|                            |                              |
  |                              |                            (async, ~1s later)              |
  |                              |--outbox relay publishes--------------------------------->  |
  |                              |  upload.events.received                    saga starts --->|
```

**Step 1 — `POST /api/v1/uploads/initiate`** (`UploadController.initiate`, `UploadService.initiate`)
Client sends just the file name. The server:
- figures out the file type from the extension (`resolveFileType`)
- builds a storage key (`<uploadId>/<fileName>`)
- asks `ObjectStorageClient.presignPut(...)` for a signed URL, valid 15 minutes
- saves an `Upload` row with `status = PENDING` — this is a new status, not one of the 5
  client-facing ones (`RECEIVED`/`VALIDATING`/.../`FAILED`); it exists purely to represent
  "we handed out a URL, nothing has arrived yet."
- returns `{ uploadId, uploadUrl, expiresAt }`

**Step 2 — client PUTs the file directly to `uploadUrl`.**
This request goes straight to MinIO. `upload-service` is not involved and doesn't even know
this happened yet.

**Step 3 — `POST /api/v1/uploads/{uploadId}/complete`** (`UploadService.complete`)
The client calls this once its PUT succeeds. The server:
- looks up the `Upload` row
- if it's already past `PENDING`, just returns the current state (idempotent — calling
  `complete` twice, e.g. after a network retry, doesn't double-publish anything)
- calls `ObjectStorageClient.exists(...)` — a MinIO `HEAD` request — to confirm the object is
  really there before trusting the client's word for it
- flips `Upload.status` to `RECEIVED` and writes the outbox event that eventually starts the
  saga (see [docs/RUNNING_LOCALLY.md](RUNNING_LOCALLY.md) for what happens next)

Notice what's **not** here: no file bytes, no slow I/O, and (compared to the old code) no
object-storage call sitting inside the `@Transactional` block — `initiate` does call
`presignPut`, but that's just constructing and signing a URL locally (fast, no network call to
MinIO at all), unlike the old `put(...)` which streamed the whole file over the network.

## Spring/Java concepts worth understanding here

- **`interface ObjectStorageClient` + `class S3ObjectStorageClient implements ObjectStorageClient`**
  (`common-library/.../storage/`) — the rest of the code (in `upload-service`) only ever
  depends on the *interface*. It has no idea MinIO/S3 is involved. If you swapped MinIO for a
  different object store later, only `S3ObjectStorageClient` would change.
- **`@AutoConfiguration` + `@ConditionalOnProperty`** (`ObjectStorageAutoConfiguration`) — this
  is how a shared library (`common-library`) hands Spring Boot a ready-made bean (`S3Client`,
  `ObjectStorageClient`) without every consuming service having to wire it up by hand. The
  `@ConditionalOnProperty` guard means this only activates for services that actually set
  `insight.storage.endpoint` — `ocr-service` and friends don't get an unwanted `S3Client`
  forced on them.
- **`record`** (`InitiateUploadRequest`, `InitiateUploadResponse`) — Java's terse syntax for an
  immutable data-holder class. `record InitiateUploadRequest(@NotBlank String fileName) {}`
  gives you a constructor, getters (`fileName()`), `equals`/`hashCode`/`toString` for free.
- **`@Transactional`** (`UploadService.initiate`/`complete`) — wraps the method in a database
  transaction: either every DB write inside it commits together, or (on an exception) they all
  roll back together. The lesson from the bug we fixed: only put things that need that
  all-or-nothing guarantee inside it — not slow calls to other systems.
