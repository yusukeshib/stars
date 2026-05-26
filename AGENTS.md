# Agent instructions

These instructions apply to AI coding agents working in this repository.

## Keep documentation in sync

Every PR that changes implementation behavior must update the relevant
documentation in the same PR. Do not leave docs for a later cleanup unless the
user explicitly scopes the task as code-only.

Use the docs according to their purpose:

- `README.md` / `README.ja.md` — short project entry points and current focus.
- `ROADMAP.md` — planned work, phase status, open items, and exit criteria.
- `PROGRESS.md` — implementation log for features that are complete enough to
  count as shipped.
- `ARCHITECTURE.md` — crate boundaries, data flow, coordinate conventions,
  renderer pipeline, and host integration.
- `CONTRIBUTING.md` — development workflow, checks, PR expectations, and review
  policy.
- `VALIDATION.md` — scientific / numerical validation policy, model limits, and
  external comparison expectations.
- `DATA_SOURCES.md` — catalog files, generated data, licenses, references, and
  preprocessing notes.
- `data/manifest.toml` — machine-readable provenance manifest. Every committed
  data artifact and every runtime web service has a row with SHA-256, source,
  license, and regeneration command. `make manifest-check` enforces it.

Before finishing a task, ask: “Did this change user-visible behavior,
scientific output, rendering output, public APIs, data sources, or project
status?” If yes, update docs.

## Numerical and rendering changes

- Any change to astronomical or photometric numerical output needs a pinned
  test or an explicit reason why a test is not practical.
- Rendering changes should include either model tests, screenshots in the PR,
  or a clear before / after explanation.
- New data sources must be recorded in `DATA_SOURCES.md` with source, version,
  license, local path, and preprocessing notes, **and** appended to
  `data/manifest.toml` with the correct `kind`, SHA-256, and regeneration
  command. `make manifest-check` re-hashes every artifact and is part of
  `make ci`.

## Project checks

Prefer the `Makefile` targets:

```bash
make fmt
make ci
```

Do not bypass hooks or skip checks unless the user explicitly asks.
