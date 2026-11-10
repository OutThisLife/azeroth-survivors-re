#!/usr/bin/env bash
#
# android-data.sh - put the game data on a connected Android phone: fetch the WoW 1.12.1 client
# on this machine (scripts/fetch-data.sh, skipped when wow-data/Data is already there), then
# `adb push` its Data folder into the app's own external files folder, where the launcher looks
# first (crates/warcraft-survivors-android, `WOW_DATA`). The download happens here, on your
# machine, as on the desktop; the APK and its releases never carry or fetch game data
# (docs/ANDROID.md, "Why data-less").
#
#   scripts/android-data.sh                 # one phone connected over USB debugging
#   ANDROID_SERIAL=<serial> scripts/android-data.sh   # pick one of several (`adb devices`)
#   WOW_DATA_DIR=/path/to/WoW scripts/android-data.sh # push a client you already have (…/Data)
#
# Re-runnable: `adb push --sync` copies only files that are missing or older on the phone.
# Requires: adb (Android SDK platform-tools), and what fetch-data.sh needs (curl, unzip).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

PKG=io.github.songoku119.azerothsurvivors
DATA_DIR="${WOW_DATA_DIR:-wow-data}"
DATA="${DATA_DIR}/Data"
# The app's external files folder (`AndroidApp::external_data_path`); no permission needed to read
# it from the app, and adb's shell user can write it.
DEST="/sdcard/Android/data/${PKG}/files"

command -v adb >/dev/null || { echo "adb not found: install the Android SDK platform-tools" >&2; exit 1; }

# One device, ready: adb picks it (or ANDROID_SERIAL does). An unauthorized phone is the usual
# snag: accept the USB debugging prompt on its screen.
STATES=$(adb devices | awk 'NR > 1 && NF >= 2 { print $2 }')
if [ -z "${ANDROID_SERIAL:-}" ]; then
  case "$(printf '%s\n' "${STATES}" | grep -c .)" in
    0) echo "no phone found: connect one with USB debugging on (adb devices lists none)" >&2; exit 1 ;;
    1) ;;
    *) echo "several devices: pick one with ANDROID_SERIAL=<serial> (adb devices)" >&2; exit 1 ;;
  esac
fi
STATE=$(adb get-state 2>/dev/null || true)
[ "${STATE}" = device ] || { echo "phone not ready (state: ${STATE:-unknown}); accept the USB debugging prompt on it" >&2; exit 1; }

# The app must be installed: Android owns its Android/data folder and removes it on uninstall.
adb shell pm path "${PKG}" >/dev/null 2>&1 \
  || { echo "the app is not installed: adb install the APK first (make android, or a release)" >&2; exit 1; }

# The data, on this machine. fetch-data.sh honours WOW_DATA_DIR and returns at once when the
# MPQs are already in place.
WOW_DATA_DIR="${DATA_DIR}" ./scripts/fetch-data.sh
[ -f "${DATA}/terrain.MPQ" ] && [ -f "${DATA}/patch.MPQ" ] \
  || { echo "${DATA} has no terrain.MPQ/patch.MPQ: not a WoW 1.12.1 Data folder" >&2; exit 1; }

# Room on the phone: the MPQs plus a margin.
NEED_KB=$(du -sk "${DATA}" | awk '{ print $1 }')
FREE_KB=$(adb shell df -k /sdcard 2>/dev/null | awk 'NR == 2 { print $4 }' | tr -dc 0-9)
if [ -n "${FREE_KB}" ] && [ "${FREE_KB}" -lt $(( NEED_KB + 512 * 1024 )) ]; then
  echo "not enough space on the phone: $(( NEED_KB / 1024 )) MB needed, $(( FREE_KB / 1024 )) MB free" >&2
  exit 1
fi

# Push into the parent: `adb push dir dest` nests dir inside an existing dest, so pushing Data at
# …/files/Data a second time would make …/files/Data/Data.
echo "pushing ${DATA} ($(( NEED_KB / 1024 )) MB) to ${DEST}/Data …"
adb shell mkdir -p "${DEST}"
adb push --sync "${DATA}" "${DEST}/"

adb shell ls "${DEST}/Data/terrain.MPQ" >/dev/null 2>&1 \
  || { echo "push finished but ${DEST}/Data/terrain.MPQ is missing" >&2; exit 1; }
echo "game data is on the phone: start the app (or press Validate on its \"No game data\" screen)"
