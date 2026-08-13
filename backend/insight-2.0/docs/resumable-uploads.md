# Resumable, chunked uploads

**Status:** Design spec + server-side contract (Option B — client-side chunking logic is
specified here but not implemented in this repo; any consuming app implements against this
contract).

## 1. Why: reuse S3/MinIO's native multipart upload, not a custom protocol

MinIO (and every S3-compatible object store — including NetApp StorageGRID/ONTAP S3, see §7)
already has a purpose-built mechanism for large, resumable, parallelizable uploads: **multipart
upload**. It fits the existing presigned-URL architecture
(`docs/PRESIGNED_UPLOADS.md`) exactly — the app server still never touches file bytes, it just
brokers permission per chunk instead of per whole file. No new infrastructure, no new protocol
server (e.g. tus.io) — just a few more methods on the `ObjectStorageClient` interface that's
already there.

## 2. The mechanics, S3-side

1. **`CreateMultipartUpload`** — server asks the object store to start a multipart upload for a
   key, gets back a storage-side multipart-upload ID (separate from our own `uploadId`).
2. The client splits the file into chunks and PUTs each one to its **own presigned URL**, per
   part number. Parts can upload **in parallel** — a real throughput win over the old single-PUT
   flow, not just a resumability one.
3. Each successful part PUT returns an **ETag** — the store's proof that part landed intact.
4. **`ListParts`** — the server can ask the store directly which parts already exist for this
   multipart upload, with their ETags and sizes. This is the source of truth for both
   resumability *and* for verifying completion — no separate bookkeeping table needed (see §5).
5. **`CompleteMultipartUpload`** — once all parts are present, the server tells the store to
   assemble them into the final object. A metadata operation — no bytes flow through the app here
   either.
6. **`AbortMultipartUpload`** — cleans up an abandoned attempt.

## 3. API contract

| Endpoint | Purpose |
|---|---|
| `POST /api/v1/uploads/initiate` | Now requires `fileSizeBytes` in addition to `fileName`. Server decides single-shot vs. multipart by threshold and returns which mode was chosen. |
| `POST /api/v1/uploads/{uploadId}/parts/{partNumber}/presign` | Returns a presigned URL for *one specific part*, generated on demand — not all parts upfront. Only valid for uploads that initiated in multipart mode. |
| `GET /api/v1/uploads/{uploadId}/parts` | Resume entry point — queries the store's `ListParts`, returns which part numbers already exist, with their sizes/ETags. |
| `POST /api/v1/uploads/{uploadId}/complete` | Existing endpoint, extended: for multipart uploads, verifies the sum of uploaded part sizes matches the declared `fileSizeBytes`, then calls `ListParts` → `CompleteMultipartUpload` before the existing status-flip/outbox logic. Unchanged for single-shot uploads. |

**Why presign per-part on demand, not all parts upfront:** a 15-minute presigned-URL TTL is fine
for a single whole-file PUT, but wrong for "resumable across a flaky connection or a multi-hour
session." Requesting part N's URL right before uploading part N means expiry is essentially never
an issue — this is why the part-count doesn't need to be known upfront either (see §4).

**`initiate` response, multipart mode**, returns everything the client needs to start its own
chunking decisions without another round trip:
```json
{
  "uploadId": "...",
  "multipart": true,
  "minPartSizeBytes": 8388608,
  "recommendedPartSizeBytes": 8388608,
  "expiresAt": "..."
}
```
(`uploadUrl` is present instead, and the three multipart-only fields absent, when
`multipart: false`.)

## 4. Why there's no `UploadPart` table

Tracking part status in our own database would create a second source of truth that can drift
from what's actually in the object store. `ListParts` already *is* that source of truth — the
server queries it directly rather than trusting anything the client reports. This mirrors the
same principle already used in the single-shot flow's `complete()`: verify against storage, don't
trust the caller's word (`docs/PRESIGNED_UPLOADS.md` §3).

**Completion safety check:** because part sizes are chosen adaptively by the client (§6), the
server can't know in advance how many parts to expect. Instead, at `complete()` time, it sums the
sizes of every part `ListParts` returns and compares that sum to the `fileSizeBytes` declared at
`initiate()` time. A mismatch (client called `complete` before finishing, or a part silently
failed) is rejected — using data the server already has, without requiring the client to report
anything new.

## 5. Resumability

If a client's upload session dies at part 40 of 100, it doesn't need to remember that itself — on
resume, it calls `GET /parts`, which does `ListParts` against the store and returns which part
numbers are already done. The client only re-requests presigned URLs and re-uploads whatever's
missing.

## 6. Adaptive part sizing — client-side algorithm (spec only)

This logic cannot live in any backend service: the bytes for each part travel client → object
store directly, so the server is never in that data path and cannot measure "how long did this
chunk take." Any consuming app/client library implements this against the contract in §3.

**AIMD-style** (same family as TCP congestion control):
1. **Start conservative** (slow start): first part uses `recommendedPartSizeBytes` from the
   `initiate` response — no throughput data exists yet.
2. **After each part completes**, compute `throughput = partSizeBytes / elapsedMillis`.
3. **Fast-connection signal** (e.g. 5MB in ≤200ms, i.e. ≥25MB/s): multiplicatively grow the next
   part's size (×1.5–2), up to a ceiling.
4. **Slow or failed part** (elapsed far above target, or a timeout/retry): shrink the next part
   size (halve it), down to `minPartSizeBytes` from the `initiate` response.
5. **In between:** hold steady.

**Bounds, and where each comes from:**
- **Floor: 5MB** — the S3/MinIO API's own minimum part size (except the final part of an upload,
  which may be smaller).
- **Ceiling:** a practical cap the client should apply (not an S3 rule) — e.g. 250MB — so a flaky
  connection never has to re-attempt something huge.
- **Part-count ceiling: 10,000 parts per upload** (S3/MinIO hard limit). For a very large file,
  this actually puts a *floor* on average part size: a 500GB file needs an average part size of
  at least 50MB just to stay under 10,000 parts, regardless of what the throughput-based
  algorithm alone would pick. The server computes this floor once, from the declared
  `fileSizeBytes`, and returns it as `minPartSizeBytes` — `max(5MB, ceil(fileSizeBytes / 10000))`.

**Concurrency, a related but separate lever:** real-world resumable uploaders (Drive, Dropbox)
often adapt how many parts upload in parallel, together with part size — but measuring
throughput on one part is muddied if several others are competing for the same bandwidth
simultaneously. Recommendation: keep concurrency fixed at a modest default (3–4 parallel parts)
for a first version; only make it adaptive too if size-only adaptation proves insufficient.
Bundling two interacting adaptive systems from day one makes both harder to reason about and tune.

## 7. Object store choice: MinIO locally, anything S3-compatible in real environments

Nothing above is MinIO-specific — it's the standard S3 multipart-upload API, which NetApp
StorageGRID and ONTAP S3 also implement. Because `ObjectStorageClient` is an interface owned by
the business logic (`docs/architecture-deep-dive.md` §6), and `S3ObjectStorageClient` is the only
class that knows which concrent store is on the other end, swapping MinIO for a real NetApp
endpoint in staging/production is a **configuration change, not a code change**:
```yaml
insight:
  storage:
    endpoint: https://your-netapp-storagegrid-endpoint
    access-key: ...
    secret-key: ...
```
Recommended split: keep MinIO in `docker-compose.yml` for local dev (free, trivial to run,
standard stand-in for "any S3-compatible store"); point the real NetApp endpoint in via the
environment-specific Helm values overlay (`values-prod.yaml`,
`docs/architecture-deep-dive.md` §7.0).

## 8. What does *not* change

- The saga is untouched — `complete()` remains the one moment that flips `PENDING → RECEIVED` and
  fires the outbox event, regardless of whether the upload went through the single-shot or
  multipart path.
- Small files still use the existing single `presignPut` flow unchanged — multipart only kicks in
  above `insight.storage.multipart-threshold-bytes` (default 100MB). No reason to pay multipart's
  extra round trips for a 2MB CSV.
- No new cleanup job in application code. An abandoned multipart upload is cleaned up by the
  object store's own lifecycle rule (`AbortIncompleteMultipartUpload` after N days) — storage-side,
  free, no polling job needed.
