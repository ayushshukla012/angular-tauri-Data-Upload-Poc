# Core Frontend Context

Owns runtime configuration, shared Angular infrastructure, window lifecycle, and cross-cutting services.

Read this file before changing:
- app-wide configuration
- HTTP/REST integration
- Tauri IPC wrappers
- global lifecycle behavior

Rules:
- Standalone components only.
- Use `inject()`, not constructor injection.
- Runtime-varying values come from `src/assets/config/app-config.json`.
- Do not read or parse the JSON directly from feature components.
- Tauri 2 imports: `@tauri-apps/api/core`, `@tauri-apps/api/event`, `@tauri-apps/api/window`.
- Long-running native work must be asynchronous/non-blocking.
