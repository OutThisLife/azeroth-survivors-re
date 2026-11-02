#!/usr/bin/env bash
# Fetch the game data — a World of Warcraft 1.12.1 (build 5875) client's Data folder — from
# archive.org. This repo ships no Blizzard content; the data is downloaded at setup time and only
# the MPQs are extracted.
#
# Re-runnable: skips when the MPQs are already present, and resumes an interrupted download.
# Requires: curl, unzip.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DATA_DIR="${WOW_DATA_DIR:-$ROOT/wow-data}"
DATA="$DATA_DIR/Data"

# --- Data source ---------------------------------------------------------------
# Where the game data is downloaded from. Override per-run by setting WOW_DATA_URL, or edit the
# default below if the archive.org item ever moves. It must be a direct link to a zip that holds
# the client's Data/*.MPQ files.
DEFAULT_DATA_URL="https://archive.org/download/WoW_1.12.1-5875/1.12.1-5875_enUS.zip"
# Tried when the default fails: archive.org's whole-item zip of the same item, built on the fly.
# It cannot be resumed (no fixed size), and it may hold the client zip inside it rather than the
# Data folder itself; both are handled below. WOW_DATA_FALLBACK_URL= (empty) turns it off. An
# explicit WOW_DATA_URL gets no fallback: a mirror you named failing should stop the run.
DEFAULT_FALLBACK_URL="https://archive.org/compress/WoW_1.12.1-5875/formats=ZIP&file=/WoW_1.12.1-5875.zip"
URL="${WOW_DATA_URL:-$DEFAULT_DATA_URL}"
if [ -n "${WOW_DATA_URL:-}" ]; then
    FALLBACK_URL=""
else
    FALLBACK_URL="${WOW_DATA_FALLBACK_URL-$DEFAULT_FALLBACK_URL}"
fi
ZIP="$DATA_DIR/$(basename "${URL%%\?*}")"
FALLBACK_ZIP="$DATA_DIR/fallback-item.zip"

# The two MPQs the game cannot boot without are the canary: both present = data is in.
have_data() { [ -f "$DATA/terrain.MPQ" ] && [ -f "$DATA/patch.MPQ" ]; }
if have_data; then
    echo "game data already present at $DATA"
    echo "  --data $DATA"
    exit 0
fi

mkdir -p "$DATA_DIR"

# Pull every Data/*.MPQ out of a zip into $DATA, flattened, whatever folder the zip nests Data in
# (unzip's `*` crosses `/`; -C matches .mpq too). When the zip holds no MPQs but another zip — the
# whole-item download wraps the client zip — extract that, preferring an enUS one, and look again.
extract_mpqs() {
    local zip="$1" depth="${2:-0}" inner list
    # Listed once into a variable: `unzip -Z1 | grep -q` under pipefail can fail on a match, when
    # grep exits early and unzip dies writing to the closed pipe.
    list=$(unzip -Z1 "$zip") || return 1
    if grep -qiE '(^|/)Data/[^/]+\.MPQ$' <<<"$list"; then
        mkdir -p "$DATA"
        unzip -o -j -C "$zip" '*Data/*.MPQ' -d "$DATA"
        return 0
    fi
    [ "$depth" -lt 2 ] || return 1
    inner=$(grep -iE '\.zip$' <<<"$list" | grep -i 'enUS' | head -1 || true)
    [ -n "$inner" ] || inner=$(grep -iE '\.zip$' <<<"$list" | head -1 || true)
    [ -n "$inner" ] || return 1
    echo "  $zip holds no MPQs; unpacking the zip inside it: $inner"
    local tmp="$DATA_DIR/inner"
    rm -rf "$tmp"
    mkdir -p "$tmp"
    unzip -o -j "$zip" "$inner" -d "$tmp" || return 1
    # The outer zip is spent; drop it now so the disk holds two copies at most, not three.
    rm -f "$zip"
    extract_mpqs "$tmp/$(basename "$inner")" $((depth + 1))
    local ok=$?
    rm -rf "$tmp"
    return $ok
}

# The default source: resumable. A partial download sits at $ZIP.part until curl finishes, so a
# rerun resumes it, and a finished $ZIP is never mistaken for a broken one.
fetch_primary() {
    if [ -f "$ZIP" ]; then
        echo "using $ZIP"
        return 0
    fi
    echo "downloading the game data (a WoW 1.12.1 client, ~5.5 GB)…"
    echo "  $URL"
    [ -f "$ZIP.part" ] && echo "  resuming $ZIP.part"
    curl -fL -C - -o "$ZIP.part" "$URL" || return 1
    mv "$ZIP.part" "$ZIP"
}

# The fallback: generated on the fly, so no resume; a partial file from an earlier attempt is
# discarded rather than appended to.
fetch_fallback() {
    echo "trying archive.org's whole-item zip instead (it cannot resume; restart means from zero)…"
    echo "  $FALLBACK_URL"
    rm -f "$FALLBACK_ZIP.part"
    curl -fL -o "$FALLBACK_ZIP.part" "$FALLBACK_URL" || return 1
    mv "$FALLBACK_ZIP.part" "$FALLBACK_ZIP"
}

if fetch_primary && extract_mpqs "$ZIP"; then
    # The MPQs are what the game reads; drop the zip to reclaim ~5.5 GB.
    rm -f "$ZIP"
elif [ -n "$FALLBACK_URL" ]; then
    echo "the default download did not give a Data folder; falling back" >&2
    # A zip that downloaded but held no MPQs would only be reused on the next run; drop it.
    rm -f "$ZIP"
    if [ -f "$FALLBACK_ZIP" ] || fetch_fallback; then
        extract_mpqs "$FALLBACK_ZIP" || { echo "no Data/*.MPQ found in $FALLBACK_ZIP" >&2; exit 1; }
        rm -f "$FALLBACK_ZIP" "$ZIP.part"
    else
        echo "both downloads failed; set WOW_DATA_URL to a mirror, or WOW_DATA to a client you have" >&2
        exit 1
    fi
else
    echo "the download did not give a Data folder ($URL)" >&2
    exit 1
fi

have_data || { echo "extracted, but $DATA has no terrain.MPQ/patch.MPQ" >&2; exit 1; }
echo "game data ready at $DATA"
echo "  --data $DATA"
