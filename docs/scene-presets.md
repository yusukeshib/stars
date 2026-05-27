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
| `dark-sky` | Mauna Kea high-altitude dark-sky view. | Dark-sky glow, extinction, Milky Way band, high-altitude atmosphere. |
| `noon` | Tokyo clear-sky local-noon view. | Daylight scattering, solar disk, star suppression by sky radiance. |
| `sunset` | Tokyo western-horizon sunset. | Golden-hour colour, horizon haze, low-solar-altitude continuity. |
| `civil-twilight` | Early post-sunset twilight. | Civil twilight band and day/night blend. |
| `nautical-twilight` | Deeper twilight with first bright stars. | Nautical twilight band and additive dark-sky transition. |
| `astronomical-twilight` | Late twilight approaching dark sky. | Astronomical twilight boundary and dark-sky continuity. |
| `moonlit-night` | Bright Moon night scene. | Moon disk, lunar illuminance, and moonlit sky term. |
| `eclipse-aid` | Narrow Moon-oriented scene near a lunar-eclipse date. | Moon phase and Earth-shadow aid path. |
| `solar-eclipse` | 2024-04-08 total solar eclipse from Mazatlán at greatest eclipse (V-51c). | Analytic Moon-on-Sun mask, Koomen 1952 daylight darkening, Baumbach 1937 corona. |
| `venus-transit` | 2012-06-06 Venus transit observed from Tokyo near greatest transit (V-51e). | Planet-on-Sun analytic-mask occluder, daylight-band sky unchanged outside the disk. |
| `all-sky-hammer` | Full-sky Hammer map. | Hammer projection and full-sky overlay clipping. |
| `all-sky-mollweide` | Full-sky Mollweide map. | Equal-area map and Milky Way / coordinate-grid continuity. |
| `galactic-north` | Built-in external top-down Milky Way view. | External viewpoint, HYG distances, analytic Milky Way disk. |
| `custom-external` | Oblique custom external galactic-frame camera. | Custom origin/target/up serialization and orientation. |

## Precedence and portability

- `--session` takes precedence over `--preset` so an explicit JSON file always
  wins.
- CLI output size/path and catalog path remain host options; the scene fields
  come from the preset/session.
- `--write-session --write-session-only` exports the effective preset or scene
  without doing a GPU render.
