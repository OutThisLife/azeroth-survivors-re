#!/usr/bin/env bash
#
# android-keystore.sh - make the release signing key, once, on your own machine, and print the
# GitHub secrets the release workflow reads it from (.github/workflows/android-release.yml).
#
#   scripts/android-keystore.sh                  # → android-release.jks beside the repo, never inside it
#   KEYSTORE=/path/to/my.jks scripts/android-keystore.sh   # somewhere else
#
# Why: an APK installs over an earlier one only when both are signed with the same key. The
# debug key Gradle makes differs per machine and per runner, so every build so far needed an
# uninstall, which also removes the 5.5 GB of game data in the app's folder. One key, kept in
# the repository's Actions secrets, lets each release install in place.
#
# Keep the .jks and its passwords somewhere safe: a lost key means one more uninstall for every
# player, since nothing signed with another key can replace what it signed. Do not commit it
# (android/.gitignore refuses *.jks). Requires: keytool (a JDK), and gh for the secrets step
# (or paste them by hand in Settings → Secrets and variables → Actions).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

KEYSTORE="${KEYSTORE:-$(pwd)/../android-release.jks}"
ALIAS="${KEY_ALIAS:-azeroth-survivors}"

# keytool ships with every JDK but is not always on the PATH (Oracle's Windows installer puts
# java there through a shim folder without it): JAVA_HOME's bin is the second place to look.
KEYTOOL=$(command -v keytool || true)
[ -n "${KEYTOOL}" ] || [ -z "${JAVA_HOME:-}" ] || [ ! -x "${JAVA_HOME}/bin/keytool" ] || KEYTOOL="${JAVA_HOME}/bin/keytool"
[ -n "${KEYTOOL}" ] || [ -z "${JAVA_HOME:-}" ] || [ ! -x "${JAVA_HOME}/bin/keytool.exe" ] || KEYTOOL="${JAVA_HOME}/bin/keytool.exe"
if [ -z "${KEYTOOL}" ]; then
  echo "keytool not found. It comes with a JDK (17 or newer): open a new terminal after installing one," >&2
  echo "or point at it: PATH=\"/c/Program Files/Java/jdk-25/bin:\$PATH\" $0 (the folder your install made)," >&2
  echo "or set JAVA_HOME to the JDK's folder." >&2
  exit 1
fi
if [ -f "${KEYSTORE}" ]; then
  echo "${KEYSTORE} exists; not touching it. To start over, move it away first." >&2
  echo "To print the secrets for it again: KEYSTORE=${KEYSTORE} $0 --secrets" >&2
  [ "${1:-}" = "--secrets" ] || exit 1
fi

if [ "${1:-}" != "--secrets" ]; then
  # One password for the store and the key (PKCS12 keystores cannot tell them apart anyway).
  # Typed, never passed on the command line, so it stays out of the shell history.
  read -r -s -p "New keystore password (12+ characters): " PASSWORD; echo
  read -r -s -p "Again: " AGAIN; echo
  [ "${PASSWORD}" = "${AGAIN}" ] || { echo "the passwords differ" >&2; exit 1; }
  [ "${#PASSWORD}" -ge 12 ] || { echo "12 characters at least" >&2; exit 1; }
  # keytool's `:env` reads the process environment, not the shell's variables.
  export PASSWORD
  # 10000 days: Android requires validity past 2033 for an app that is to be updated for years.
  "${KEYTOOL}" -genkeypair -v \
    -keystore "${KEYSTORE}" -storetype PKCS12 -storepass:env PASSWORD -keypass:env PASSWORD \
    -alias "${ALIAS}" -keyalg RSA -keysize 4096 -validity 10000 \
    -dname "CN=Azeroth Survivors, OU=Android, O=azeroth-survivors-re"
  unset PASSWORD
  echo
  echo "made ${KEYSTORE} (alias ${ALIAS})"
fi

# `-R` because this clone has two remotes (origin and the parent): gh must not guess.
REPO="${GITHUB_REPO:-songoku119/azeroth-survivors-re}"
echo
echo "The release workflow reads four repository secrets. With gh (github.com/cli/cli) logged in:"
echo
echo "  base64 < '${KEYSTORE}' | tr -d '\n' | gh secret set ANDROID_KEYSTORE_BASE64 -R ${REPO}"
echo "  gh secret set ANDROID_KEYSTORE_PASSWORD -R ${REPO}   # paste the password when asked"
echo "  gh secret set ANDROID_KEY_ALIAS -R ${REPO} --body '${ALIAS}'"
echo "  gh secret set ANDROID_KEY_PASSWORD -R ${REPO}        # the same password"
echo
echo "Then run the android-release workflow; its 'signing' step says which key signed the APK."
echo "Players on a debug-signed build (android-v0.2.5 and earlier) uninstall once more, which"
echo "removes the app's Data folder too; to keep it, move it aside first:"
echo "  adb shell mv /sdcard/Android/data/io.github.songoku119.azerothsurvivors/files/Data /sdcard/Download/Data"
echo "and back after the install. After that, every release installs over the last with"
echo "adb install -r and keeps the game data."
