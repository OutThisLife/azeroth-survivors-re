<img width="1920" height="1080" alt="brag" src="https://github.com/user-attachments/assets/fa574bb6-7bd4-4e78-8b72-14c58e3c716e" />

<div align="center">

# Azeroth Survivors

**A free fan-made WoW roguelike by [Aethus](https://wow.aethus.pl/), run locally on a
from-scratch WoW 1.12.1 client engine (Rust + [Bevy](https://bevy.org)).**

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20%2F%20Apache--2.0-blue.svg)](LICENSE-MIT)

</div>

> **Not an original project.** *Azeroth Survivors* is a free, fan-made WoW roguelike by
> **Aethus** — play the original in the browser at **[wow.aethus.pl](https://wow.aethus.pl/)**
> ([r/wowservers announcement](https://www.reddit.com/r/wowservers/comments/1x0yidl/azeroth_survivors_roguelike_wow_singleplayer_game/)
> · [Discord](https://discord.gg/UzxaZuSAk)). This repository is a from-source local build of the
> game, running on the [benilla](#how-its-built) / [wenilla](#how-its-built) engine, for personal use.

Pick a class and a battleground, then survive: WASD to move, your abilities cast themselves,
enemies swarm in waves, you level up and pick upgrades, and a boss shows up every couple of
minutes. **No server, no login, no account** — fully offline and self-contained.

<table>
  <tr>
    <td width="50%"><img src="docs/screenshots/menu.jpg" alt="The pick screen: race, gender and class on the left, sixteen zones on the right, the hero in between"></td>
    <td width="50%"><img src="docs/screenshots/combat.jpg" alt="A Mage holding off Elwynn Forest's wolves with frost, fire and arcane"></td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/level-up.jpg" alt="Level up: three spell cards, coloured by rarity, each showing exactly what its next rank changes"></td>
    <td><img src="docs/screenshots/results.jpg" alt="The results screen after a death in Redridge Mountains"></td>
  </tr>
</table>

### In this build

- **A character-create pick screen** built from the client's own glue art: eight races, both
  genders, every class the race can be, and **sixteen zones**, from the starting zones to
  Naxxramas and the Gates of Ahn'Qiraj. Each zone fights with its own creatures and bosses, taken
  from the zone's real spawns. Your pick is remembered across reloads.
- **Level-up cards** that deal in, tilt toward the cursor and lift on hover. Their WoW item-quality
  colour says how strong a pick is, and each card lists exactly what changes, like
  `Damage 20 » 26` or `Targets 2 » 3`.
- **A class movement spell on F** from level 5, aimed at the cursor: Blink, Charge, Sprint, Dash,
  Fade, Ghost Wolf, Aspect of the Cheetah, Blessing of Freedom, Howl of Terror.
- **A camera you can free:** locked top-down by default with the hero facing the cursor. **C**
  unlocks it to swing and tilt, and the wheel zooms from over the shoulder out to 70 yards.
- A light sharpen-and-colour pass over the picture (**G** toggles it).

## Play it

```bash
git clone https://github.com/OutThisLife/azeroth-survivors-re
cd azeroth-survivors-re
make build     # fetch game data + install toolchain + build the wasm (~5.5 GB download)
make start     # → open http://127.0.0.1:8090/
```

`make build` chains three re-runnable scripts, each skipping what's already done:

| Step | What it does |
| --- | --- |
| `scripts/fetch-data.sh` | downloads the WoW 1.12.1 client and extracts its MPQs |
| `scripts/web-setup.sh` | installs the wasm toolchain (wasi-sdk, wasm-bindgen, binaryen) |
| `scripts/web-build.sh` | builds the client into `web/dist/` |

**Prerequisites:** [Rust](https://rustup.rs/), `make`, `curl`, `unzip`, and a bash shell. On Windows, run everything from Git Bash; PowerShell and cmd can't run the scripts.

Run `make` (or `make help`) to list every target.

## Controls

See [CONTROLS.md](CONTROLS.md). The short version: **WASD** to move, **Space** to jump, **F**
for your class's movement spell, the **mouse wheel** to zoom, **C** to unlock the camera, **Esc**
to pause, **1/2/3** to pick a level-up card.

## Play it on Android

**Experimental, untested on a device.** The Android build (docs/ANDROID.md) is a sideloaded
APK of the survivors mode, touch-controlled (CONTROLS.md, "Touch"), that reads a WoW 1.12.1
`Data` folder you put on the phone yourself. It ships no data and is on no store. Nothing here
has yet been run on a real phone; the Rust side type-checks for the Android target in CI, the
APK itself is yours to build and try.

```bash
export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/<r28 or newer>   # Android Studio's SDK Manager installs one
make android-toolchain   # rustup target, cargo-ndk; checks the NDK and the lockfile
make android             # → android/app/build/outputs/apk/debug/app-debug.apk (needs an SDK, platform 34)
adb install -r android/app/build/outputs/apk/debug/app-debug.apk
make android-data        # fetches the data on this machine if needed, then adb pushes it to the phone
```

The app looks for `Data` in its own external files folder, where `make android-data` puts it (no
permission needed; the download runs on your machine, never in the app), and says what is missing
when it finds none. Vulkan 1.1 and Android 10+ (arm64) are required; `adb logcat -s wsurvivors:*
wgpu:*` is where it reports.

## The game data (not in this repo)

This repo ships **no Blizzard content** — no art, models, sounds, maps, MPQ contents, or
FrameXML. The game reads a WoW 1.12.1 client's `Data` folder at runtime, which you supply.

`make build` (via `scripts/fetch-data.sh`) downloads the **English WoW 1.12.1 (build 5875)**
client from [archive.org](https://archive.org/details/WoW_1.12.1-5875) (~5.5 GB) and extracts the
MPQs into `wow-data/Data/` (git-ignored, resumable, skipped when already present). Or point at a
client you already have with `--data` / `WOW_DATA`.

### Changing the download source

If the archive.org item moves or you have your own mirror, override the URL without editing
anything:

```bash
make build WOW_DATA_URL=https://your-mirror.example/client.zip
```

Or set it permanently by editing the single clearly-marked `DEFAULT_DATA_URL` line at the top of
`scripts/fetch-data.sh`. Either way the URL must be a direct link to a zip holding the client's
`Data/*.MPQ` files.

## How it's built

The survivors mode is `crates/benilla-app/src/survivors/`, running on two upstream projects:

- [benilla](https://github.com/samwhosung/benilla) — a from-scratch WoW 1.12.1 client.
- [wenilla](https://github.com/Arnesen/wenilla) — benilla compiled to WebAssembly, with a local
  host and realm service.

## Legal

*Azeroth Survivors* is a free fan game by **Aethus**, not affiliated with or endorsed by
Blizzard Entertainment, and it ships no Blizzard game files. The screenshots in `docs/screenshots/`
show World of Warcraft art rendered from a user-supplied client; that art is © Blizzard
Entertainment. World of Warcraft is a trademark of Blizzard Entertainment, Inc. The engine code in
this repository is licensed under
[MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE), at your option; the vendored components under
`third_party/` keep their own upstream licenses.
