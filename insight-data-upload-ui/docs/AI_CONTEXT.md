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
7. `/src/app/app.routes.ts`
8. `/src/app/core/config/app-config.service.ts`
9. `/docs/contracts/openapi.yaml` only when an API contract is relevant
10. The smallest feature-local `CONTEXT.md` for the requested area

Do not read the complete repository unless a dependency, symbol, contract, build failure, or security flow requires it.

## System summary

This is a desktop Data Upload utility built with:

- Angular standalone components
- Angular zoneless change detection
- NgRx SignalStore
- Angular CDK virtual scrolling
- Tauri 2.x
- Rust native data access
- Spring Boot REST APIs

The native Rust layer owns large local datasets. Angular owns UI state and only a bounded row window.

## Non-negotiable performance SLA

- Dataset target: `2,000,000+` rows.
- Initial data readiness target: `<= 8 seconds`.
- TTFR target: `<= 1.5 seconds`.
- UI must remain responsive while scrolling/filtering.
- Active DOM rows target: `<= 150`.
- Angular must never materialize all dataset rows into DOM or permanent SignalStore state.
- Filtering/sorting of millions of rows must not run in Angular.
- Large data operations belong in Rust/native storage or the backend.
- Any performance regression must block merge until explicitly accepted.

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

## Large dataset path

```text
CSV / native dataset
      |
      v
Rust local store
      |
      | LIMIT/OFFSET or keyset window
      v
bounded chunk
      |
      v
Tauri IPC/event
      |
      v
DataRecordsStore
      |
      | bounded cache only
      v
CDK viewport
      |
      v
small DOM window
```

Never send the full dataset through one IPC payload.

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
