#!/usr/bin/env bash
#
# android-setup.sh - one-time toolchain for the Android build (no sudo, nothing system-wide):
#   - rustup target aarch64-linux-android
#   - cargo-ndk (drives rustc through the NDK's clang and lays the .so out as jniLibs/)
#   - a check that ANDROID_NDK_HOME points at an NDK r28 or newer (16 KB page alignment is the
#     default there; r27 works only through the linker flag `.cargo/config.toml` adds)
#   - a check that Cargo.lock holds one `android-activity` (two versions crash at startup: the
#     Java side registers one `GameActivity` ABI, and winit expects the one bevy was built with)
#
# The NDK and the SDK themselves are not downloaded here: Android Studio's SDK Manager, or
# `sdkmanager "ndk;28.0.13004108" "platforms;android-34" "build-tools;34.0.0"`, and then
# `export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/<version>`.
#
# Re-runnable; each step is skipped when already done.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

echo "== rustup target"
rustup target add aarch64-linux-android

echo "== cargo-ndk"
if command -v cargo-ndk >/dev/null; then
  echo "cargo-ndk $(cargo ndk --version | awk '{print $2}') already installed"
else
  cargo install cargo-ndk --locked
fi

echo "== Android NDK"
ndk="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
if [ -z "${ndk}" ] && [ -n "${ANDROID_HOME:-}" ] && [ -d "${ANDROID_HOME}/ndk" ]; then
  # The newest installed NDK, when the SDK Manager put one beside the SDK.
  ndk=$(ls -d "${ANDROID_HOME}"/ndk/* 2>/dev/null | sort -V | tail -1)
fi
if [ -z "${ndk}" ] || [ ! -f "${ndk}/source.properties" ]; then
  echo "no Android NDK: set ANDROID_NDK_HOME to an NDK r28 or newer (sdkmanager \"ndk;28.0.13004108\")" >&2
  exit 1
fi
rev=$(awk -F' = ' '/^Pkg.Revision/{print $2}' "${ndk}/source.properties")
major="${rev%%.*}"
if [ "${major}" -lt 27 ]; then
  echo "NDK r${rev} at ${ndk} is too old: r28+ (r27 at the least, with the page-size flag in .cargo/config.toml)" >&2
  exit 1
fi
[ "${major}" -ge 28 ] || echo "  NDK r${rev}: 16 KB pages come from .cargo/config.toml's link-arg; r28+ aligns by default"
echo "NDK r${rev} at ${ndk}"

echo "== Cargo.lock: one android-activity"
n=$(grep -c '^name = "android-activity"$' Cargo.lock || true)
if [ "${n}" != 1 ]; then
  echo "Cargo.lock holds ${n} versions of android-activity; bevy_winit and the Java side need exactly one" >&2
  grep -A1 '^name = "android-activity"$' Cargo.lock >&2
  exit 1
fi
echo "android-activity $(awk '/^name = "android-activity"$/{getline; sub(/version = /,""); gsub(/"/,""); print; exit}' Cargo.lock)"

echo "== Gradle"
if [ -x android/gradlew ]; then
  echo "android/gradlew present"
elif command -v gradle >/dev/null; then
  echo "no android/gradlew; android-build.sh uses $(command -v gradle) ($(gradle --version 2>/dev/null | awk '/^Gradle/{print $2}'))"
else
  echo "  no gradle on PATH and no android/gradlew — install Gradle 8.x, or run 'gradle wrapper' in android/ once" >&2
fi
echo "ready: ANDROID_NDK_HOME=${ndk} scripts/android-build.sh"
