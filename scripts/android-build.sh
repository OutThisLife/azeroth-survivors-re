#!/usr/bin/env bash
#
# android-build.sh - build the Android client: the arm64 cdylib into android/app/src/main/jniLibs,
# then the debug APK through Gradle.
#
#   scripts/android-build.sh              # android/app/build/outputs/apk/debug/app-debug.apk
#   ANDROID_DEBUG=1 scripts/android-build.sh   # keep the symbols (ndk-stack reads them)
#   ANDROID_PROFILE=ship scripts/android-build.sh   # fat LTO, as the desktop `ship` profile
#   ANDROID_GRADLE_TASK=assembleRelease scripts/android-build.sh   # the release build type; signed
#       with the release key when ANDROID_KEYSTORE_FILE/PASSWORD/KEY_ALIAS are set (what the
#       release workflow does from the repository's secrets; scripts/android-keystore.sh makes
#       the key), else with the debug key
#
# Then: adb install -r android/app/build/outputs/apk/debug/app-debug.apk
#       adb push /path/to/WoW/Data /sdcard/Android/data/io.github.songoku119.azerothsurvivors/files/Data
#       adb logcat -s wsurvivors:* wgpu:* RustStdoutStderr:*
#
# Prerequisites: scripts/android-setup.sh (rustup target, cargo-ndk, ANDROID_NDK_HOME); an
# Android SDK at ANDROID_HOME (platform 34, build-tools) for the Gradle step.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

PROFILE="${ANDROID_PROFILE:-release}"
case "${PROFILE}" in
  release|ship) ;;
  *) echo "ANDROID_PROFILE must be release or ship" >&2; exit 1 ;;
esac
JNI=android/app/src/main/jniLibs
SO="${JNI}/arm64-v8a/libwarcraft_survivors_android.so"

command -v cargo-ndk >/dev/null || { echo "cargo-ndk not found — run scripts/android-setup.sh" >&2; exit 1; }
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
[ -d "${ANDROID_NDK_HOME}" ] || { echo "ANDROID_NDK_HOME not set — run scripts/android-setup.sh" >&2; exit 1; }
# The NDK's own binutils: readelf for the checks below, strip for the shipping .so.
PREBUILT=$(ls -d "${ANDROID_NDK_HOME}"/toolchains/llvm/prebuilt/*/bin 2>/dev/null | head -1)
READELF="${PREBUILT}/llvm-readelf"; command -v "${READELF}" >/dev/null || READELF="$(command -v llvm-readelf || true)"
STRIP="${PREBUILT}/llvm-strip"; command -v "${STRIP}" >/dev/null || STRIP="$(command -v llvm-strip || true)"

# minSdk 29 is the manifest's; cargo-ndk's default platform is 21, which would link against
# older libc symbols than the app ever runs on.
cargo ndk -t arm64-v8a -P 29 -o "${JNI}" build --profile "${PROFILE}" -p warcraft-survivors-android --no-default-features
[ -f "${SO}" ] || { echo "cargo ndk produced no ${SO}" >&2; exit 1; }

if [ -n "${READELF}" ]; then
  # libc++_shared: a C dependency built against the shared C++ runtime would need the .so
  # packaged (`cargo ndk --link-libcxx-shared`); the tree builds against the static one.
  if "${READELF}" -d "${SO}" | grep -q 'NEEDED.*libc++_shared'; then
    echo "${SO} NEEDS libc++_shared.so: rebuild with cargo ndk --link-libcxx-shared, or package it" >&2
    exit 1
  fi
  # 16 KB pages (Android 15+): every LOAD segment aligned to 0x4000, from NDK r28 or
  # .cargo/config.toml's `max-page-size=16384`.
  if "${READELF}" -l "${SO}" | awk '/^ *LOAD/{print $NF}' | grep -qv '^0x4000$'; then
    echo "${SO} has a LOAD segment not aligned to 0x4000 (16 KB pages); NDK r28+, or the flag in .cargo/config.toml" >&2
    "${READELF}" -l "${SO}" | grep LOAD >&2
    exit 1
  fi
  echo "NEEDED: $("${READELF}" -d "${SO}" | awk '/NEEDED/{gsub(/[\[\]]/,"",$NF); printf "%s ", $NF}')"
else
  echo "llvm-readelf not found — skipping the libc++_shared and page-alignment checks"
fi

# cargo-ndk 4 copies the .so as linked, debug info and all (~hundreds of MB); strip it unless the
# symbols are wanted. The original stays under target/ for ndk-stack either way.
if [ "${ANDROID_DEBUG:-0}" = 1 ]; then
  echo "ANDROID_DEBUG=1 — keeping symbols in ${SO}"
elif [ -n "${STRIP}" ]; then
  "${STRIP}" --strip-unneeded "${SO}"
else
  echo "llvm-strip not found — shipping the unstripped library"
fi
ls -la "${SO}"

# Gradle: the wrapper when it has been generated (`gradle wrapper` in android/, once), else the
# system install; gradle-wrapper.jar is a binary and not committed.
TASK="${ANDROID_GRADLE_TASK:-assembleDebug}"
if [ -x android/gradlew ]; then
  (cd android && ./gradlew --no-daemon "${TASK}")
elif command -v gradle >/dev/null; then
  (cd android && gradle --no-daemon "${TASK}")
else
  echo "no gradle: install Gradle 8.x or run 'gradle wrapper' in android/ once" >&2
  exit 1
fi
ls -la android/app/build/outputs/apk/*/*.apk
