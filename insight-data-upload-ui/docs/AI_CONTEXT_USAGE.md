# AI Context Usage

## Exact repository placement

```text
<repository-root>/AI_CODING_STANDARDS.yml
<repository-root>/docs/AI_CONTEXT.md
<repository-root>/docs/FRONTEND_ARCHITECTURE.md
<repository-root>/docs/contracts/openapi.yaml

<repository-root>/src/app/features/data-upload/CONTEXT.md
<repository-root>/src/app/features/virtual-table/CONTEXT.md
<repository-root>/src/app/core/CONTEXT.md
<repository-root>/src-tauri/src/CONTEXT.md
```

## What the AI should read for a normal task

Always:

```text
AI_CODING_STANDARDS.yml
        +
docs/AI_CONTEXT.md
```

Then load exactly one local context when possible:

```text
Data upload change       -> src/app/features/data-upload/CONTEXT.md
Virtual table change     -> src/app/features/virtual-table/CONTEXT.md
Core/API/config change   -> src/app/core/CONTEXT.md
Rust/Tauri change        -> src-tauri/src/CONTEXT.md
```

Read `docs/contracts/openapi.yaml` only when the task changes or depends on a backend API contract.

## Why this structure exists

The YAML contains the non-negotiable engineering rules.
`docs/AI_CONTEXT.md` contains the system map and context-routing rules.
Feature `CONTEXT.md` files contain only local invariants and relevant files.
The OpenAPI file is the contract source rather than a prose summary.

This keeps normal AI context small and reduces the chance that an agent invents architecture by scanning unrelated code.

## When context must expand

The AI may read additional files only when:

- a required symbol is defined elsewhere
- a public contract is being changed
- a build/test error requires dependency tracing
- a security-sensitive flow crosses boundaries
- a performance regression requires root-cause analysis

After expanding context, the AI should still avoid reading unrelated modules.
