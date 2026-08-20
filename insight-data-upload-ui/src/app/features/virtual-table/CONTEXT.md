# Large Dataset Table Context

The current UX is page-based, not continuous scrolling.

Performance contract:
- Never render all records with `*ngFor`.
- Native page size is runtime-configured.
- Angular state contains only the page currently visible.
- Native SQL uses deterministic `row_order`.
- Filtering of the full dataset must not run in Angular.
- No `JSON.stringify`, deep clone, `map`, `filter`, or `slice` across millions of records.
- Continuous CDK virtual scrolling is a future option only if the UX changes from pagination to continuous scrolling.
