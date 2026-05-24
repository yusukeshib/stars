---
name: quality-pass
description: Sweep the entire stars codebase for design, crate-boundary, and code-quality issues, fix them all, run CI, and commit. Use when the user asks for "コードレビュー", "code review", "quality pass", or "tighten up the code".
user-invocable: true
allowed-tools:
  - Bash
  - Read
  - Edit
  - Write
  - Grep
  - Glob
  - TaskCreate
  - TaskUpdate
  - TaskList
---

# Quality Pass

Single-pass review-and-fix sweep of the stars repo. The codebase is small (~1700
lines of Rust + ~400 lines of TS), so read every source file with fresh eyes —
do not skim. The goal is to ship a tight, internally consistent commit, not a
report.

The user may run this skill multiple times in a session. Each invocation should
find and fix something; if you genuinely find nothing, say so and stop.

## Workflow

1. **Scan**. Read every file under `crates/`, `apps/`, plus the workspace
   `Cargo.toml`, each crate/app `Cargo.toml`, the `Makefile`, and the frontend
   under `apps/web/frontend/src/`. Use `grep` to find dead deps, unused
   re-exports, stale paths.
2. **List issues** in the response, grouped by category (see checklist below).
   Mark items you intend to skip with a one-line reason — premature
   optimization, scope creep, etc.
3. **Track with `TaskCreate`** when there are 3+ fixes. Mark each as
   `in_progress` before starting and `completed` immediately after.
4. **Fix systematically**, batching related edits but keeping each commit's
   intent clear. Prefer one commit at the end over many partial commits.
5. **Verify**:
   - `make ci` — must pass (fmt, clippy `-D warnings`, all tests, wasm32 check)
   - `cd apps/web/frontend && bun run tsc --noEmit` — when web/frontend touched
   - `wasm-pack build apps/web --target web --out-dir frontend/pkg` — when
     `apps/web/src/lib.rs` touched
   - Render a smoke test PNG to confirm visual output didn't regress:
     `cargo run -p stars-cli --release -- --lat 40.7359 --lng -73.9911 --time 2026-04-26T22:00:00Z --azimuth 200 --altitude 25 --fov 70 --catalog crates/catalog/data/hyg_v42.csv -o /tmp/quality-pass.png`
     then `Read` the PNG to eyeball it.
6. **Commit and push** with a multi-section message. Body should describe what
   changed by category (Architecture / Public API / Naming / etc.) — not a flat
   list of file edits.

## Checklist

### Crate boundaries

- Every `[dependencies]` entry must be actually `use`d somewhere in that crate's
  source. Run `grep -rn "use <dep>::\|<dep>::" crates/<x>/src` to verify.
  Common rot: `log`, `bytemuck`, `glam`, `js-sys` listed but unused.
- Concepts must live in the right crate: `Observer` (lat/lng/jd) belongs in
  `astronomy`, not `renderer`. `magnitude_to_render_params` belongs in
  `renderer`, not `catalog`. If you find a misplaced type, move it.
- `renderer` must NOT depend on `catalog` (only on `astronomy`).

### Public-API hygiene

- In `crates/renderer/src/lib.rs`, the `camera`/`pipeline`/`vertex`/`renderer`
  modules should be **private** (`mod`, not `pub mod`); everything reachable is
  re-exported at the crate root.
- A `pub fn` only used inside its own crate should be `pub(crate)` (or just
  `fn` for `impl` blocks where possible).
- A `pub struct` field that callers shouldn't touch (e.g. `RawStar`'s serde
  intermediate) shouldn't be public.
- `Result<(), JsValue>` returns from `#[wasm_bindgen]` methods that always
  return `Ok` should be unit. Same for any other dead-`Result` pattern.

### Naming

- A function returning a struct or tuple should have a name that reflects what
  it returns. `magnitude_to_size` returning `(size, brightness)` was wrong —
  fixed to `magnitude_to_render_params -> RenderParams`.
- A field measuring the falloff radius should be `radius_px`, not `size_px`.
  Reach for `radius`/`half_width`/`extent` deliberately.
- HUD/CLI flag names should match across `stars-cli` and `stars-viewer`
  (`--lat`, `--lng`, `--time`, `--azimuth`, `--altitude`, `--fov`).

### Dead code / dead branches

- `x.rem_euclid(positive)` already returns non-negative — the `if v < 0 { v +=
  tau }` follow-up is dead.
- Unused exports in `apps/web/frontend/src/observer.ts` (e.g. an old
  `julianDateNow`) should be removed if no consumer.
- `TODO`/`FIXME` left in: investigate; fix or convert to a tracked task.

### Magic values

- Render clear color, FOV clamps, magnitude cutoffs, time-speed presets —
  promote to named constants when used twice or when the meaning isn't obvious
  from context.

### Stale paths / docs

- Default `--catalog` paths must point at the current `crates/catalog/data/...`
  (not `crates/stars-catalog/...`). Refactors break these silently.
- Doc comments on functions with mixed units (e.g. `radec_to_cartesian(ra_hours,
  dec_degrees)`) must call out the units explicitly.

### Tests

- A test that just re-applies the function under test and asserts the trivial
  identity is a tautology — replace with one that compares against a known
  astronomical fact (e.g. north celestial pole projects to view altitude ≈
  observer's latitude).
- Every `astronomy` and `renderer` change should have at least one test
  exercising it. Tolerance on float comparisons should match the precision
  of the underlying model (1e-12 for time math, 1e-4 for `Mat4`-via-`f32`).

### Frontend

- React inputs must validate before applying: `Number("")` is `0`, which would
  silently snap latitude to the equator.
- `<label>` needs `htmlFor` linked to the input's `id` for accessibility.
- `useRef` declarations belong at the top of the component, not interleaved
  with `useEffect` blocks.
- The render-loop tick should pull from refs, not props directly, so the loop
  doesn't need to be torn down on every prop change.
- Don't duplicate Rust constants in TS when you can pass the raw input across
  the WASM boundary and convert there (e.g. pass `Date.now()` ms; convert to
  JD in Rust via `astronomy::julian_date_from_unix_seconds`).

## What NOT to do

- Don't rewrite for theoretical perf wins without measurement. The render loop
  crossing the WASM/JS boundary three times per frame is fine until proven
  otherwise.
- Don't add features. This skill cleans up what's there; it doesn't extend it.
- Don't create new `.md` documentation. The skill itself and code comments are
  sufficient.
- Don't split the cleanup across multiple commits unless they're genuinely
  independent. One coherent "second-pass cleanup" commit is fine.
