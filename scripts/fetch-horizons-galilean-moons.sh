#!/usr/bin/env bash
#
# Regenerate `data/horizons_galilean_moons.csv` — the Lieske-E5 validation
# fixture for the `V-52b-E5` precision upgrade.
#
# Pulls geocentric apparent ICRF (RA, Dec, range) for Io / Europa / Ganymede /
# Callisto from JPL Horizons at three epochs spanning the ROADMAP ±100-yr
# budget (1900-01-01, 2000-01-01, 2100-01-01 — all 00:00:00 UT).
#
# Output columns:
#   moon_naif, epoch_utc, jd_utc, ra_hms, dec_dms, range_au, deldot_km_s
#
# Horizons API reference:
#   https://ssd.jpl.nasa.gov/horizons/manual.html
#   https://ssd-api.jpl.nasa.gov/doc/horizons.html
#
# Usage:
#   bash scripts/fetch-horizons-galilean-moons.sh
#
# Determinism: Horizons output for *past* epochs is reproducible to the
# digit; the JPL ephemeris kernel evolves only for *future* dates as new
# observations land. The 2100 row is therefore expected to drift by
# milliarcseconds across JPL kernel updates — this is intentional, since
# the whole point of the fixture is to track JPL's current best estimate.

set -euo pipefail

cd "$(dirname "$0")/.."

OUT="data/horizons_galilean_moons.csv"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

declare -a MOONS=("599:Jupiter" "501:Io" "502:Europa" "503:Ganymede" "504:Callisto")
declare -a EPOCHS=("1900-01-01" "2000-01-01" "2100-01-01")

printf 'moon_naif,moon_name,epoch_utc,jd_utc,ra_hms,dec_dms,range_au,deldot_km_s\n' > "$OUT"

for moon_entry in "${MOONS[@]}"; do
    naif="${moon_entry%%:*}"
    name="${moon_entry##*:}"
    for epoch in "${EPOCHS[@]}"; do
        # JD for 00:00 UT on the requested civil date. Horizons computes
        # this internally; we hard-code the standard Julian dates for the
        # CSV row so downstream tests can pin the epoch without a date-math
        # dependency. (1900-01-01 00:00 UT = JD 2415020.5,
        # 2000-01-01 00:00 UT = JD 2451544.5, 2100-01-01 00:00 UT = JD 2488069.5).
        case "$epoch" in
            "1900-01-01") jd="2415020.5" ;;
            "2000-01-01") jd="2451544.5" ;;
            "2100-01-01") jd="2488069.5" ;;
            *) echo "unknown epoch $epoch" >&2; exit 1 ;;
        esac

        raw="$TMP/${naif}_${epoch}.txt"
        curl -sS --fail --get "https://ssd.jpl.nasa.gov/api/horizons.api" \
            --data-urlencode "format=text" \
            --data-urlencode "COMMAND='${naif}'" \
            --data-urlencode "OBJ_DATA=NO" \
            --data-urlencode "MAKE_EPHEM=YES" \
            --data-urlencode "EPHEM_TYPE=OBSERVER" \
            --data-urlencode "CENTER=500@399" \
            --data-urlencode "START_TIME='${epoch} 00:00'" \
            --data-urlencode "STOP_TIME='${epoch} 00:01'" \
            --data-urlencode "STEP_SIZE='1 m'" \
            --data-urlencode "QUANTITIES='1,20'" > "$raw"

        # Extract the first ephemeris row between $$SOE and $$EOE.
        line=$(awk '/\$\$SOE/{flag=1; next} /\$\$EOE/{flag=0} flag{print; exit}' "$raw")
        if [ -z "$line" ]; then
            echo "no ephemeris row for $name @ $epoch" >&2
            exit 1
        fi

        # Sample Horizons row (geocentric / ICRF apparent / light-time corrected):
        #   2000-Jan-01 00:00     01 35 17.38 +08 34 23.3  4.61225879634782  42.5630470
        # Columns: <date> <UT HH:MM> <RA hh mm ss.ff> <Dec sdd mm ss.f> <range AU> <deldot km/s>
        # We strip the date/time prefix and keep the five astronomical fields.
        fields=$(echo "$line" | awk '{print $3" "$4" "$5","$6" "$7" "$8","$9","$10}')
        ra_hms="${fields%%,*}"
        rest="${fields#*,}"
        dec_dms="${rest%%,*}"
        rest="${rest#*,}"
        range="${rest%%,*}"
        deldot="${rest#*,}"

        printf '%s,%s,%s,%s,"%s","%s",%s,%s\n' \
            "$naif" "$name" "$epoch" "$jd" "$ra_hms" "$dec_dms" "$range" "$deldot" >> "$OUT"

        # Be polite to the public Horizons endpoint.
        sleep 0.3
    done
done

echo "wrote $(wc -l < "$OUT") lines (incl. header) to $OUT"
