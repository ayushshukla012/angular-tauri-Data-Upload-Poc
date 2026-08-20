# AI Context Index — Insight Data Upload

## Purpose

This file is the small, stable context entry point for Codex, AI IDEs, code assistants, and developers.
It exists to prevent agents from reading the entire repository before making a normal change.

## Mandatory reading order

1. `/AI_CODING_STANDARDS.yml`
2. `/docs/AI_CONTEXT.md`
3. `/docs/FRONTEND_ARCHITECTURE.md`
4. `/package.json`
5. `/angular.json`
6. `/src/app/app.config.ts`
7. `/src/app/core/config/app-config.service.ts`
8. `/src/app/app.component.ts` only when the requested area belongs to the current page
9. `/docs/contracts/openapi.yaml` only when an API contract is relevant
10. The smallest feature-local `CONTEXT.md` for the requested area

Do not read the complete repository unless a dependency, symbol, contract, build failure, or security flow requires it.

## System summary

This is a desktop Data Upload utility built with:

- Angular standalone components
- Angular zoneless change detection
- NgRx SignalStore
- Page-bounded native SQLite rendering for the current page-based UX
- Tauri 2.x
- Rust native data access
- Spring Boot REST APIs

The native Rust layer owns large local datasets. Angular owns UI state and only a bounded row window.

## Non-negotiable performance SLA

- Dataset target: `2,000,000+` rows.
- Initial data readiness target: `<= 8 seconds`.
- TTFR target: `<= 1.5 seconds`.
- UI must remain responsive while scrolling/filtering.
- Active DOM rows target: `<= 150` (current page size is bounded; continuous CDK scrolling is not the current UI contract).
- Angular must never materialize all dataset rows into DOM or permanent SignalStore state.
- Filtering/sorting of millions of rows must not run in Angular.
- Large data operations belong in Rust/native storage or the backend.
- Any performance regression must block merge until explicitly accepted.


## Current implementation status

This repository is in an incremental architecture migration.

Implemented:
- runtime `AppConfigService`
- Tauri 2 native row store
- asynchronous native CSV import worker
- first-100-row progressive handoff from import modal to table
- 10,000-row background import/progress checkpoints
- live loaded-record count and pagination growth while import continues
- bounded SQLite paging
- background close cleanup so Exit Without Saving is not blocked by SQLite deletion
- progress/error IPC events
- AI context files
- typed backend service

Still centralized in the current POC screen:
- `src/app/app.component.ts`
- `src/app/app.component.html`
- `src/app/services/*`

Do not pretend `SignalStore`, a CDK virtual table, or a separate feature component hierarchy is already wired into the current screen. Introduce those only as part of an explicit refactor.

## Architecture map

```text
Angular UI
  -> feature components
  -> feature SignalStore
  -> ApiClientService / TauriBridgeService

Tauri 2 / Rust
  -> native row store
  -> bounded queries
  -> chunk/event delivery

Spring Boot
  -> packet APIs
  -> case APIs
  -> presigned upload APIs
  -> upload status/parts APIs
```

## Configuration authority

All environment/capacity/runtime-varying values come from:

`/src/assets/config/app-config.json`

Access is only through:

`/src/app/core/config/app-config.service.ts`

Never hard-code:

- backend URLs
- ports
- retry delays/statuses
- pagination sizes
- virtual-scroll buffers
- upload thresholds
- feature flags
- runtime theme values

## Backend contract authority

The canonical frontend API contract is:

`/docs/contracts/openapi.yaml`

Before adding or changing an endpoint, request field, response field, or status handling:

1. Search the OpenAPI contract.
2. Verify the exact path/method/schema.
3. Update the typed API client.
4. Add/update tests.

Never invent a data-query endpoint simply because the UI needs one. Large local dataset reads use the Tauri/native path unless the backend contract explicitly provides the required query API.

## Large CSV import path

```text
CSV file
      |
      v
Tauri 2 async command
      |
      v
Rust worker / buffered csv parser
      |
      +--> bounded SQLite transactions
      +--> dataset metadata row_count
      +--> progress events (first 100, then 10,000-row checkpoints)
      +--> final row-order index after bulk import
      |
      v
native row store
      |
      | LIMIT/OFFSET page query
      v
current page only
      |
      v
Angular table
```

The current UX is page-based. Never bind a million-row array to `ngFor`.
Never send the full dataset through one IPC payload.
The import modal must close as soon as the first configured readiness batch (default 100 rows) is queryable.
While importing, the table shows currently loaded pages and the count expands at configured progress checkpoints (default 10,000 rows).
Exit Without Saving must destroy the window without awaiting large SQLite cleanup.
The import modal must close as soon as the first configured readiness batch (default 100 rows) is queryable.
While importing, the table shows currently loaded pages and the count expands at configured progress checkpoints (default 10,000 rows).
Exit Without Saving must destroy the window without awaiting large SQLite cleanup.

## Feature context routing

Use the smallest matching context file:

| Change area | Read first |
|---|---|
| Data Upload feature/state | `/src/app/features/data-upload/CONTEXT.md` |
| Virtual table / 2M rows | `/src/app/features/virtual-table/CONTEXT.md` |
| API/config/interceptors | `/src/app/core/CONTEXT.md` |
| Tauri IPC/Rust/native store | `/src-tauri/src/CONTEXT.md` |

## Change workflow for AI agents

1. Identify the owning feature.
2. Read this index and the feature-local context.
3. Search symbols rather than opening unrelated files.
4. Verify contracts before implementation.
5. Make the smallest coherent change.
6. Run focused tests/typecheck/lint.
7. For rendering/data-path changes, run the performance checks.
8. Update the relevant context file when architecture or command contracts change.
9. Do not silently weaken the SLA to make a test pass.

## Performance review checklist

For every large-data or rendering change, verify:

- TTFR
- first visible rows latency
- scroll smoothness/jank
- heap growth
- retained row count
- visible DOM count
- native query latency
- API latency when applicable
- absence of giant array copies/clones

## Security review checklist

For auth, upload, sensitive-data, or native filesystem changes, verify:

- RBAC/entitlement assumptions
- sensitive-data masking/visibility
- input validation
- safe IPC command arguments
- no secrets in Angular source or runtime config
- audit/event behavior where required

## Context maintenance rule

Keep this file under approximately 250 lines.
Keep feature context files short and operational.
Do not turn context files into full design documents.
Use links/path references to deeper documentation instead of duplicating prose.
