# Scene presets

`stars` includes deterministic named scenes for demos, validation, teaching, and
bug reports. They are not a separate format: each preset is built as a normal
schema-versioned JSON session and can be exported, imported, or rendered by the
same host paths as user sessions.

List the preset IDs supported by the native hosts:

```bash
cargo run -p stars-cli -- --list-presets
cargo run -p stars-viewer -- --list-presets
```

Render one preset:

```bash
cargo run -p stars-cli --release -- --preset dark-sky -o dark-sky.png
```

Export portable JSON sessions for all presets:

```bash
./scripts/export-scene-presets.sh
```

The generated JSON files are written to `docs/presets/sessions/` by default and
can be replayed with `--session`, imported into the web UI through the settings
panel, or attached to a report.

## Built-in presets

| Preset ID | Purpose | Main validation focus |
|---|---|---|
| `tokyo-tonight` | Fixed Tokyo summer evening scene. The name is mnemonic; the timestamp is pinned for reproducibility. | Default local perspective, overlays, labels, star/planet composition. |
| `tokyo-bortle-8` | Same Tokyo evening framing as `tokyo-tonight`, pinned to V-39 Bortle 8 city-sky pollution + the hazy-urban atmosphere. | V-39 Bortle-8 artificial sky-glow: warm-orange sodium/LED tint, horizon-brighter Garstang fall-off, dimmed faint stars. |
| `dark-sky` | Mauna Kea high-altitude dark-sky view. | Dark-sky glow, extinction, Milky Way band, high-altitude atmosphere. |
| `dark-sky-bortle-1` | Same dark-sky scene pinned to V-39 Bortle 1 floor; equal to `dark-sky` by construction. | V-39 Bortle 1 default rendering pixel-identical to the pre-V-39 dark-sky pipeline. |
| `noon` | Tokyo clear-sky local-noon view. | Daylight scattering, solar disk, star suppression by sky radiance. |
| `sunset` | Tokyo western-horizon sunset. | Golden-hour colour, horizon haze, low-solar-altitude continuity. |
| `civil-twilight` | Early post-sunset twilight. | Civil twilight band and day/night blend. |
| `civil-twilight-antisolar-tokyo` | Tokyo civil twilight framed on the anti-solar horizon (V-27). | Belt of Venus arch and Earth-shadow band; R/G ratio at pinned ROI altitudes. |
| `nautical-twilight` | Deeper twilight with first bright stars. | Nautical twilight band and additive dark-sky transition. |
| `astronomical-twilight` | Late twilight approaching dark sky. | Astronomical twilight boundary and dark-sky continuity. |
| `moonlit-night` | Bright Moon night scene. | Moon disk, lunar illuminance, and moonlit sky term. |
| `eclipse-aid` | Narrow Moon-oriented scene near a lunar-eclipse date. | Moon phase and Earth-shadow aid path. |
| `solar-eclipse` | 2024-04-08 total solar eclipse from Mazatlán at greatest eclipse (V-51c). | Analytic Moon-on-Sun mask, Koomen 1952 daylight darkening, Baumbach 1937 corona. |
| `venus-transit` | 2012-06-06 Venus transit observed from Tokyo near greatest transit (V-51e). | Planet-on-Sun analytic-mask occluder, daylight-band sky unchanged outside the disk. |
| `jupiter-shadow-transit` | 2008-12-20 ~14:00 UT Io shadow transit on Jupiter from Roque de los Muchachos (V-52d). | Galilean-shadow analytic-mask occluder on the Jovian disk via the V-51b Planet-on-Planet path; V-52b moon sprites unaffected outside Jupiter. |
| `bright-comet` | 2024-10-17 ~18:45 UT dark-sky western-evening view of comet C/2023 A3 (Tsuchinshan-ATLAS) from 40°N / 0° (V-49). | Comet two-body position/magnitude, 1/ρ coma, and anti-solar ion / β=0.6 dust tails against a dark twilight sky. |
| `all-sky-hammer` | Full-sky Hammer map. | Hammer projection and full-sky overlay clipping. |
| `all-sky-mollweide` | Full-sky Mollweide map. | Equal-area map and Milky Way / coordinate-grid continuity. |
| `galactic-north` | Built-in external top-down Milky Way view. | External viewpoint, HYG distances, analytic Milky Way disk. |
| `custom-external` | Oblique custom external galactic-frame camera. | Custom origin/target/up serialization and orientation. |

## Object search and GoTo (V-56)

All three hosts can resolve a named object through the shared catalog
search index (bright named stars, Bayer / Flamsteed / HR / HD / HIP
designations, Messier / NGC / IC ids, the planets, the Sun / Moon, and
Japanese aliases) and centre the view on it.

| Capability | Web | CLI | Viewer |
|---|---|---|---|
| Free-text search box / prompt | search panel | `--goto <name>` | press `/` for a title-bar prompt |
| GoTo (slew/centre on target) | click a result | `--goto <name>` | type query + Enter |
| Info panel (mag, RA/Dec, Alt/Az, distance) | non-modal panel | printed `GoTo …` summary | window title bar |
| Startup target from a flag | — | `--goto <name>` | `--goto <name>` |
| Click / hover pick of any rendered body | yes | — | — (search-driven instead) |

Examples:

```bash
# Render centred on M31 from Tokyo.
cargo run -p stars-cli --release -- --lat 35.68 --lng 139.69 --goto m31 -o andromeda.png

# Launch the viewer already pointed at Saturn, then press '/' to search again.
cargo run -p stars-viewer --release -- --goto saturn
```

The CLI prints a one-line `GoTo …` summary (name, kind, magnitude,
RA/Dec, Alt/Az, distance) before rendering. In the viewer the same
summary is shown in the window title bar, which doubles as a
renderer-free info panel; `--goto` accepts the same query grammar in
both hosts. Click / hover picking of an arbitrary rendered body is a
web-only affordance.

## Precedence and portability

- `--session` takes precedence over `--preset` so an explicit JSON file always
  wins.
- CLI output size/path and catalog path remain host options; the scene fields
  come from the preset/session.
- `--write-session --write-session-only` exports the effective preset or scene
  without doing a GPU render.
