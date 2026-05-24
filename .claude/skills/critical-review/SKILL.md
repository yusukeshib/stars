---
name: critical-review
description: Critically review the entire stars codebase along three axes — (1) architecture and crate boundaries, (2) Rust code quality, (3) academic / scientific correctness — then fix every finding and ship a PR. Use when the user asks for "critical review", "thorough review", "厳しいレビュー", "学術的レビュー", or otherwise asks for a deep, skeptical pass that goes beyond engineering hygiene into IAU-grade correctness.
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

# Critical Review

A three-axis, defensible review pass for the `stars` repo. The codebase is
small (~1700 lines of Rust + ~400 lines of TS); read every file with fresh
eyes and a hostile mindset. The output is a structured report **followed by
a commit that fixes everything actionable** — not just the report.

This is distinct from `quality-pass`:

| Skill | Lens |
|---|---|
| `quality-pass` | Engineering hygiene: dead deps, naming, boundaries, magic numbers |
| `critical-review` | All of the above **plus** academic correctness vs IAU/SOFA, numerical precision, citability |

Run `critical-review` when you want a Phase-2/Phase-3-ROADMAP-aware sweep. Run
`quality-pass` for a quicker mechanical cleanup.

## When to invoke

Triggers in the user's request include:
- "very critical", "thorough", "academic", "rigorous"
- "学術的", "厳しく", "徹底的に", "アカデミックに"
- Mentions of "IAU", "SOFA", "precision", "arcsec", "Phase 2", "citation"
- Any request that pairs "architecture" + "code quality" + "correctness"

If the user just says "review the code" without those signals, prefer
`quality-pass` instead.

## Workflow

1. **Scan everything.** Read every file under `crates/`, `apps/`, plus the
   workspace `Cargo.toml`, each crate/app `Cargo.toml`, `ROADMAP.md`,
   `USAGE.md`, the `Makefile`, `.github/workflows/*.yml`, and the frontend
   under `apps/web/frontend/src/`. **Do not skim.** Open shaders
   (`crates/renderer/src/shaders/*.wgsl`) — they are part of the math.

2. **Cross-reference ROADMAP.** Anything the ROADMAP marks as Phase 2/3 is
   acceptable to leave as a doc-comment note ("ROADMAP Phase 2"), not a fix.
   Anything *not* in ROADMAP that you find is either a finding or a ROADMAP
   addition.

3. **Report first, in the user's language.** Produce a structured review with
   the three sections below. Tag every finding 🔴 / 🟡 / 🟢 (must-fix /
   discussable / nit). Map every finding to a file:line. Then proceed to
   fix all 🔴 and as many 🟡 as scope allows; leave 🟢 fixes for the
   `quality-pass` skill unless trivially co-located.

4. **Track with `TaskCreate`** when there are 3+ fixes. Mark each
   `in_progress` before starting, `completed` immediately after.

5. **Fix systematically.** Group edits by category; keep the commit's intent
   clear. Prefer one well-structured commit at the end. **Do not** add
   features — this skill cleans and clarifies, never extends.

6. **Verify.** All of:
   - `cargo fmt --all -- --check`
   - `cargo clippy --all-targets -- -D warnings`
   - `cargo test --workspace`
   - `cargo check --target wasm32-unknown-unknown --manifest-path apps/web/Cargo.toml`
   - `make ci` is equivalent if it exists.
   - Smoke-render PNG and `Read` it to confirm no visual regression:
     ```
     cargo run -p stars-cli --release -- \
       --lat 40.7359 --lng -73.9911 --time 2026-04-26T22:00:00Z \
       --azimuth 200 --altitude 25 --fov 70 \
       --catalog crates/catalog/data/hyg_v42.csv \
       -o /tmp/critical-review.png
     ```
   - If `apps/web/frontend/` was touched:
     `cd apps/web/frontend && bun run tsc --noEmit`
   - If `apps/web/src/lib.rs` was touched:
     `wasm-pack build apps/web --target web --out-dir frontend/pkg`

7. **Commit + push + PR.** Multi-section message grouped by axis (Architecture
   / Rust quality / Academic correctness / Docs). Include the original report
   in the PR description so the reviewer sees the audit trail.

## The three axes

### (1) Architecture & crate boundaries

- The split is fixed by `USAGE.md`:
  - `crates/{astronomy,catalog,renderer}` = **engine** (no clap, no chrono, no winit, no wasm-bindgen).
  - `crates/common` = **host helpers** for native binaries only (clap, chrono).
  - `apps/{cli,viewer,web}` = **hosts** (UI / CLI / WASM).
- `renderer` must **not** depend on `catalog`. `catalog` must not depend on `renderer`.
- Any concept that shows up in *two* hosts identically is a candidate for
  `crates/common` (string ↔ enum maps, time parsing, default flag values).
- `USAGE.md` and its "5-step recipe" are the API contract. Any API drift
  between code and `USAGE.md` is a 🔴 finding.
- Avoid creating new top-level `.md` files; extend `USAGE.md` or
  `ROADMAP.md` instead.

### (2) Rust code quality

- Public surface: `pub fn` only used inside its own crate must be `pub(crate)`.
  Module visibility in `lib.rs` is `mod`, not `pub mod`, unless the module
  itself is the public entry point.
- **Mixed-unit function signatures are 🔴.** `fn(ra_hours, dec_degrees)` either
  encodes the units in the name (`radec_hours_deg_to_cartesian`) or uses
  newtypes.
- `#[repr(C)]` struct field offsets exposed to `wgpu::VertexAttribute` must be
  computed via `std::mem::offset_of!`, never hardcoded.
- Magic numbers in clamp / threshold positions need a named constant and a
  comment explaining the derivation (e.g. `ALT_LIMIT = π/2 − 0.01`: why 0.01?).
- `match` arms that fall through to `_ => return` on a host-driver type
  (e.g. `wgpu::CurrentSurfaceTexture`) must log the unexpected variant.
  Silent skips have masked real OS-level failures.
- `f64 → f32` casts on angles measured in radians-near-2π lose ~0.15″.
  Evaluate trig in f64, cast to f32 only at the matrix-element level.

### (3) Academic / scientific correctness

This is what separates `critical-review` from `quality-pass`. For every
formula or constant, ask: *would I be embarrassed if a referee cited this?*

Mandatory checks:

- **Constants**: every named astronomical constant must cite its IAU
  resolution / source and its epoch. `OBLIQUITY_RAD` must say "IAU 2006,
  J2000.0" — not just "IAU 2006".
- **Frames**: doc must state whether a quantity is in J2000 (catalog epoch)
  or of-date. The renderer currently draws everything at J2000; the doc must
  not promise more.
- **Latitude**: confirm whether the formula uses *geodetic*, *geocentric*, or
  *astronomical* latitude. For star directions the distinction is invisible;
  for the Moon and planets (Phase 2) it is not. Doc the assumption.
- **Time**: UT1 ≈ UTC is the current approximation. DUT1 can be ~0.9 s
  ≈ 13.5″ on the sky. Acceptable for Phase 1; doc it.
- **Photometry**: Pogson's law `L = 10^(−0.4(m − m_ref))` is the only
  defensible magnitude → linear-flux mapping. Any "size scales with brightness"
  star rendering is a 🔴 finding (this codebase has already exiled it; keep
  it exiled).
- **Color**: B−V → RGB needs an attributed source (Ballesteros 2012 →
  blackbody → CIE 1931 → sRGB is the citable path). A piecewise polynomial
  fit is fine if its limitations are documented.
- **Atmosphere**: refraction and extinction are *not* applied. Doc this
  loudly anywhere the user might mistake the rendered altitude for the
  observed altitude.
- **Catalog filtering**: HYG uses `dist = 100000` as a sentinel for
  unknown / negative parallax. Filters keyed on that value must say so in
  the comment.

For every finding in this axis, decide:
- **Fix the doc** (most common — the math is right but under-described)
- **Fix the math** (rare — usually only when the doc claims a precision the
  code can't deliver)
- **Add to ROADMAP** (when the fix needs new infrastructure)

## Severity legend

- 🔴 **Must fix this pass.** API drift, mixed-unit signatures, dead-branch
  silent failures, claims the code doesn't back up.
- 🟡 **Fix or document this pass.** Magic numbers, missing citations, host
  duplication, doc-vs-comment-vs-code disagreement.
- 🟢 **Nit / next pass.** Style, minor optimization, additional tests.

## What NOT to do

- Don't add features. This skill clarifies what's there.
- Don't rewrite working math "more elegantly" — the current formulas are
  pinned by tests; preserve them.
- Don't promote 🟢 to 🟡 to inflate the report.
- Don't create new `.md` docs. Extend `USAGE.md` / `ROADMAP.md` / inline
  comments.
- Don't conclude "nothing found" without genuinely scanning every file. If
  the codebase really is clean, say so explicitly and stop — running the
  skill must always either fix something or certify the absence of
  findings.

## Reference: first-pass findings template

When invoked, output in this shape (translate headers if the user is using
Japanese):

```
## (1) Architecture & crate boundaries
- 🔴 …  (file:line)
- 🟡 …  (file:line)

## (2) Rust code quality
- 🔴 …
- 🟡 …

## (3) Academic / scientific correctness
- 🔴 …
- 🟡 …
- 🟢 …

## Fix plan
1. …
2. …
```

Then execute the plan, verify, and commit.
