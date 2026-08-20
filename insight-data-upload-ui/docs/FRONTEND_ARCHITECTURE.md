# Insight Data Upload Utility — production frontend architecture

The desktop path is designed around a bounded WebView heap. The native Rust row store remains the owner of million-row CSV data; Angular receives only a sliding window. Large binary uploads use the backend's presigned-upload contract, so file bytes go directly from the client to object storage.

## Runtime configuration
`src/assets/config/app-config.json` is loaded before application bootstrap. API endpoints, retry policy, virtual-scroll limits, upload thresholds, pagination defaults, feature flags and theme values live there.

## 2M+ row rendering
`CdkVirtualScrollViewport` is used with a custom strategy. No `cdkVirtualFor` is bound to a two-million-element array. The strategy sets the logical content size and renders a small index range. `DataRecordsStore` caches only a bounded native window.

## Backend compatibility
The API client maps only to the supplied OpenAPI endpoints: packets, cases, presigned upload initiation/parts, upload completion/status and case documents. No unverified backend data-query endpoint is invented.

## Tauri 2
Angular IPC is limited to `invoke` from `@tauri-apps/api/core` and `listen` from `@tauri-apps/api/event`. Rust exposes `get_runtime_config`, `get_row_window` and `stream_row_window`.

## Verification
Run `npm ci --legacy-peer-deps`, then `npm run build`, and for desktop `npm run tauri:build`. `npm run tauri:dev` generates the Tauri devUrl from `src/assets/config/app-config.json`, so the committed `tauri.conf.json` contains no hard-coded dev host/port. The provided sandbox may need a newer Node runtime and the Tauri platform toolchain before these commands can complete.

## Dependency lock note
The original lockfile was removed because the dependency graph was upgraded to Angular 22.1.x, Angular CDK 22.1.x and NgRx Signals 21.1.1, and the sandbox could not regenerate a trustworthy lockfile while the registry install was unavailable. Run `npm install --legacy-peer-deps` in the build environment to generate the lockfile before committing.
