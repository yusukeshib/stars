#!/usr/bin/env python3
"""Notebook companion for reproducible stars CLI/session examples.

The functions in this file intentionally use only the Python standard library
plus the Rust CLI/example binaries. That keeps the notebook useful before the
Phase 3 PyO3 bindings exist.
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import subprocess
from pathlib import Path
from typing import Iterable

ROOT = Path(__file__).resolve().parents[2]
NOTEBOOK_DIR = Path(__file__).resolve().parent
EXPECTED_DIR = NOTEBOOK_DIR / "expected"
DEFAULT_SESSION_IDS = ("tokyo-tonight", "moonlit-night")
NUMERIC_COLUMNS = {
    "jd_utc",
    "ra_deg",
    "dec_deg",
    "alt_deg",
    "az_deg",
    "distance_au",
    "distance_km",
    "angular_radius_arcmin",
    "illuminated_fraction",
    "phase_angle_deg",
    "magnitude",
}


def session_path(session_id: str) -> Path:
    return ROOT / "docs" / "presets" / "sessions" / f"{session_id}.json"


def expected_table_path(session_id: str) -> Path:
    return EXPECTED_DIR / f"{session_id}-session-table.csv"


def load_session(session_id: str) -> dict:
    with session_path(session_id).open(encoding="utf-8") as f:
        return json.load(f)


def scene_summary(session_ids: Iterable[str] = DEFAULT_SESSION_IDS) -> list[dict]:
    """Load JSON sessions and return a compact table for display."""
    rows = []
    for session_id in session_ids:
        session = load_session(session_id)
        rows.append(
            {
                "session": session_id,
                "schemaVersion": session["schemaVersion"],
                "latDeg": session["observer"]["latitudeDeg"],
                "lngDeg": session["observer"]["longitudeDeg"],
                "jdUtc": session["time"]["jdUtc"],
                "jdTdb": session["time"]["jdTdb"],
                "projection": session["projection"]["projection"],
                "viewpoint": session["projection"]["viewpoint"],
                "atmosphere": session["atmosphere"]["preset"],
                "overlays": ",".join(session["overlays"]["layers"]),
            }
        )
    return rows


def run_session_table(session_id: str) -> list[dict]:
    """Run the Rust astronomy table example for a schema-versioned session."""
    command = [
        "cargo",
        "run",
        "-q",
        "-p",
        "stars-cli",
        "--example",
        "session-table",
        "--",
        str(session_path(session_id)),
    ]
    completed = subprocess.run(
        command,
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    )
    return list(csv.DictReader(completed.stdout.splitlines()))


def read_expected_table(session_id: str) -> list[dict]:
    with expected_table_path(session_id).open(encoding="utf-8", newline="") as f:
        return list(csv.DictReader(f))


def compare_tables(
    actual: list[dict],
    expected: list[dict],
    *,
    tolerance: float = 5e-6,
) -> list[str]:
    """Return human-readable differences between two astronomy CSV tables."""
    errors: list[str] = []
    if len(actual) != len(expected):
        errors.append(f"row count: actual={len(actual)} expected={len(expected)}")
        return errors
    for index, (got, want) in enumerate(zip(actual, expected), start=1):
        for column, want_value in want.items():
            got_value = got.get(column, "")
            if column in NUMERIC_COLUMNS and (want_value or got_value):
                try:
                    delta = abs(float(got_value) - float(want_value))
                except ValueError:
                    errors.append(
                        f"row {index} {column}: actual={got_value!r} expected={want_value!r}"
                    )
                    continue
                if not math.isfinite(delta) or delta > tolerance:
                    errors.append(
                        f"row {index} {column}: actual={got_value} expected={want_value} delta={delta:g}"
                    )
            elif got_value != want_value:
                errors.append(
                    f"row {index} {column}: actual={got_value!r} expected={want_value!r}"
                )
    return errors


def check_expected_tables(session_ids: Iterable[str] = DEFAULT_SESSION_IDS) -> dict[str, list[str]]:
    return {
        session_id: compare_tables(run_session_table(session_id), read_expected_table(session_id))
        for session_id in session_ids
    }


def render_session(
    session_id: str,
    output_path: Path | None = None,
    *,
    width: int = 640,
    height: int = 360,
) -> Path:
    """Render the same JSON session through the CLI to a PNG artifact."""
    output_path = output_path or (NOTEBOOK_DIR / "out" / f"{session_id}.png")
    output_path.parent.mkdir(parents=True, exist_ok=True)
    command = [
        "cargo",
        "run",
        "-q",
        "-p",
        "stars-cli",
        "--",
        "--session",
        str(session_path(session_id)),
        "--width",
        str(width),
        "--height",
        str(height),
        "-o",
        str(output_path),
    ]
    subprocess.run(command, cwd=ROOT, check=True)
    return output_path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check-tables",
        action="store_true",
        help="compare current Rust session tables with committed expected CSVs",
    )
    parser.add_argument(
        "--render",
        metavar="SESSION_ID",
        help="render one JSON session through apps/cli into examples/notebooks/out/",
    )
    args = parser.parse_args()

    if args.check_tables:
        failures = check_expected_tables()
        for session_id, errors in failures.items():
            if errors:
                print(f"{session_id}: FAIL")
                for error in errors:
                    print(f"  - {error}")
            else:
                print(f"{session_id}: ok")
        if any(failures.values()):
            raise SystemExit(1)

    if args.render:
        print(render_session(args.render))

    if not args.check_tables and not args.render:
        for row in scene_summary():
            print(row)


if __name__ == "__main__":
    main()
