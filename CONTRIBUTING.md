# Contributing

Thanks for contributing to `stars`. This project combines rendering, astronomy,
and cross-platform host code, so contributions should keep crate boundaries,
numerical validation, and host parity in mind.

## Setup

Install the Rust toolchain from `rust-toolchain.toml`, `wasm-pack`, and Bun.
Then run:

```bash
make setup
```

`make setup` builds the web package, installs frontend dependencies, and
downloads the star catalog into `crates/catalog/data/hyg_v42.csv`.

## Common commands

```bash
make cli       # render a PNG via apps/cli
make viewer    # run the native desktop viewer
make web       # build WASM and start the web dev server
make scene-presets            # export deterministic preset JSON sessions
make validation-gallery       # render/update validation/demo PNGs
make validation-gallery-check # opt-in screenshot regression on stable adapters
make manifest-check           # verify data/manifest.toml against on-disk bytes
make ci        # run the full local check suite
make clean     # remove build artifacts
```

CLI arguments can be overridden:

```bash
make cli ARGS="--lat 35.68 --lng 139.69 --azimuth 180 --altitude 30 -o stars.png"
```

## Before opening a PR

Run:

```bash
make ci
```

This currently covers:

- `cargo fmt --all -- --check`;
- `cargo clippy --all-targets -- -D warnings`;
- `cargo test --workspace`;
- `make manifest-check` (verifies `data/manifest.toml` SHA-256s);
- `cargo check -p stars-web --target wasm32-unknown-unknown --manifest-path apps/web/Cargo.toml`;
- frontend typecheck.

If a change cannot reasonably pass one of these locally, explain why in the PR.

## Crate-boundary rules

Use [`ARCHITECTURE.md`](ARCHITECTURE.md) as the source of truth. In short:

- `crates/astronomy` owns scientific models and coordinate/time calculations.
- `crates/catalog` owns catalog parsing and catalog-space conversions.
- `crates/renderer` owns GPU-facing rendering and renderer state.
- `crates/common` owns native-host glue only.
- `apps/*` own platform lifecycle and UI.

Avoid adding host or UI dependencies to engine crates. Avoid putting core
scientific or rendering logic in host code.

## Numerical-change policy

Any change that can alter astronomical or photometric numerical output must add
or update tests that pin the intended value.

Examples of numerical changes:

- time-scale conversion;
- sidereal time;
- precession, nutation, aberration, proper motion, or refraction;
- Sun / Moon / planet apparent positions;
- rise / transit / set calculations;
- photometry, extinction, skyglow, daylight, twilight, tone mapping;
- catalog coordinate or colour conversion.

A good numerical PR should answer:

1. What model or approximation changed?
2. What reference is being followed?
3. What tolerance is expected?
4. Which tests would fail if the result silently drifted?
5. Which hosts are affected?

## Rendering-change policy

Rendering changes should be visually motivated and architecturally isolated.

For visual features, prefer small shippable slices:

- add CPU-side data model;
- add renderer state / uniforms;
- add shader or draw pass;
- wire one host;
- then wire remaining hosts.

When a feature changes output appearance, include at least one of:

- a deterministic unit test for the underlying model;
- a screenshot in the PR description, preferably generated from a named scene
  preset, `make validation-gallery`, or `scripts/generate-readme-images.sh` when updating README images;
- a visual-regression baseline or update when the change affects a covered
  gallery scene;
- a short before / after explanation;
- a documented limitation if the model is deliberately approximate.

## Host parity

If a renderer or astronomy feature is intended to be generally available, wire
it into all relevant hosts:

- CLI;
- desktop viewer;
- web.

It is acceptable to land a feature in one host first only if the roadmap or PR
makes that scope explicit.

## Data policy

When adding catalog or reference data:

1. Record the source in [`DATA_SOURCES.md`](DATA_SOURCES.md).
2. Record the license or redistribution terms.
3. Prefer scripts or documented preprocessing over hand-edited derived files.
4. Keep large data out of the repository unless there is a clear reason to
   embed it.
5. If the data affects numerical output, add a validation test.
6. For generated documentation images or gallery scenes, record the command or
   session used to regenerate them.
7. Append an entry to [`data/manifest.toml`](data/manifest.toml) with the
   correct `kind` (`embedded`, `generated`, or `runtime-service`), the
   SHA-256, and the regeneration `preprocessing` command. `make manifest-check`
   re-hashes every artifact and is part of `make ci`; CI fails if a data
   file's bytes drift from the recorded hash without updating the manifest in
   the same PR. See [`crates/manifest/src/lib.rs`](crates/manifest/src/lib.rs)
   for the full schema.

## Documentation policy

Update docs in the same PR as code when any of these change:

- public command or host behaviour;
- crate boundaries;
- coordinate or time conventions;
- data sources;
- scientific model choice;
- roadmap status;
- implemented feature list.

Use these files for their intended purpose:

- `README.md` and `README.ja.md`: short entry point.
- `ROADMAP.md`: what remains and why.
- `PROGRESS.md`: what has been completed.
- `ARCHITECTURE.md`: how the system is structured.
- `VALIDATION.md`: how scientific correctness is checked.
- `DATA_SOURCES.md`: where data comes from.

## Commit / PR checklist

Before asking for review:

- [ ] Code is formatted.
- [ ] Clippy passes with warnings denied.
- [ ] Tests pass.
- [ ] WASM check passes if web-facing code changed.
- [ ] Frontend typecheck passes if web UI changed.
- [ ] Numerical output changes have pinned tests.
- [ ] Visual output changes include screenshots, scene presets, or a clear
      before / after note.
- [ ] New data sources and generated artifacts are documented in
      `DATA_SOURCES.md` **and** `data/manifest.toml`.
- [ ] `make manifest-check` passes (re-hashing detects unrecorded data drift).
- [ ] README / roadmap / progress docs are updated if user-facing status changed.

## Good first areas

Useful self-contained areas include:

- text label infrastructure for stars, planets, constellations, cardinal
  directions, and degree marks;
- Visual-track full-sky projection plumbing (`V-40`);
- Messier-only first pass for deep-sky overlays;
- README / validation-gallery scene generation;
- documentation improvements and validation notes;
- additional tests around boundary cases in planning and time systems.
