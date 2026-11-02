# Android port plan

Azeroth Survivors runs today as a wasm/WebGPU page (`crates/wenilla`) and as a native desktop
binary (`crates/warcraft-survivors`). This is the plan for a third launcher: an Android build of
the survivors mode. It is written for the people and agents who will do the work, in the same
spirit as [UPSTREAM.md](UPSTREAM.md): what to touch, what not to touch, how to know a phase is
done. Line numbers are as of `dd5e330`; re-check them after a sync.

## Status

What is on this branch, as of the first implementation pass. Everything below was written
without an NDK, an SDK or a device at hand, so "verified" means a type check, a host-side unit
test or a wasm check, never a link, an APK or a frame on a phone. The device-gated exits are
listed last and are still open; P2 is still the go/no-go.

**Implemented.**

| phase | item | where |
|---|---|---|
| P0 | viewport meta on the dev page | `web/index.html` |
| P1 | the launcher crate: `#[bevy_main]` cdylib (+ `rlib` for host tests), `run_survivors` as the desktop binary | `crates/warcraft-survivors-android/src/lib.rs` |
| P1 | env shim: `BENILLA_HOME`, `WOW_DATA` (external files dir, then files dir), `WGPU_BACKEND=vulkan`, `env.toml` lines; the android half with the directories and the logcat panic hook | `crates/warcraft-survivors-android/src/{env_shim,android}.rs` |
| P1 | `arboard` gate widened to `not(any(wasm32, android))` | `crates/benilla-app/Cargo.toml`, `textinput/clipboard.rs` |
| P1 | 16 KB page-size link flag for `aarch64-linux-android` | `.cargo/config.toml` |
| P1 | Gradle project: `MainActivity : GameActivity` (games-activity 4.4.0, android-activity 0.6.1's pin), keep-screen-on, Back kept from finishing the activity, `minSdk 29`/`targetSdk 34`, Vulkan 1.1 `uses-feature`, `sensorLandscape`, every runtime config change handled in place (`configChanges`), `arm64-v8a` only | `android/` |
| P1 | setup (target, cargo-ndk, NDK r27+/r28 check, one `android-activity` in the lock) and build (`cargo ndk … -P 29`, `libc++_shared` and `LOAD` alignment checks, strip, Gradle) scripts; `make android-toolchain` / `make android` | `scripts/android-setup.sh`, `scripts/android-build.sh`, `Makefile` |
| P2 | `WOW_MIP_DROP` on the decoded (no-BC) path, Android default 1 | `crates/benilla-assets/src/gpu_blp/mip_drop.rs`, one call in `gpu_blp.rs` |
| P2 | the Mali-gate preset the shim seeds into `env.toml` on first run (`WOW_FARCLIP`, `WOW_RENDER_SCALE`, `WOW_MSAA`, clutter, aniso, `WOW_GRADE=0`, `WOW_FPS_CAP=30`, `WOW_MIP_DROP=1`), compiled into the launcher (`env_shim::DEFAULT_ENV_TOML`); `spellEffectLevel` is a `config.toml` CVar and is not seeded yet | `scripts/android-env.default.toml`, `android.rs` |
| P2 | the frame cap, native only (`WOW_FPS_CAP`) | `crates/benilla-app/src/survivors/framecap.rs` |
| P3 | `SurvivorsInput`: keyboard/mouse plus floating joystick, tap/drag aim, two-finger pinch and orbit, Back as Esc; the touch HUD layout at 48 dp from `UiScale`; CONTROLS.md "Touch" | `survivors/input.rs`, `hud.rs`, `hero.rs`, `minimap.rs`, `mobility.rs`, `grade.rs`, `CONTROLS.md` |
| P3 | touch-first: on Android (or `WOW_TOUCH=1`, `?touch=1` on the web) the touch layout is on from launch and a key never switches it off; every key prompt has a touch wording (`input::prompt`, `input::Prompt`); a hidden (`Display::None`) HUD button never claims a touch | `survivors/input.rs`, `hud.rs`, `enemies.rs`, `CONTROLS.md` |
| P4 | Route A (`adb push` into the app's external files folder) through the shim; the "No game data" panel with **Validate** (`Chain::open`) | shim, `survivors/nodata.rs` |
| P5 | `WillSuspend` → pause, cameras off until the frame after `Resumed`, audio suspended; `Mixer::set_suspended` / `OutputBackend::set_suspended` | `survivors/lifecycle.rs`, `sound/mixer/suspend.rs`, `sound/output/suspend.rs` (+ a `suspended` field and one condition in `sound/output/mod.rs`) |
| P5 | Back: `OnBackPressedCallback` on the Kotlin side, `enableOnBackInvokedCallback=false` | `android/` |
| P6 | the `android` job in `check.yml` (setup-ndk r28c, cargo-ndk, the target check); README, AGENTS.md, UPSTREAM.md "Android carries" | `.github/workflows/check.yml`, docs |

**Verified, and how.** Host `cargo check` of the new crate and its `env.toml` unit tests
(including that the seeded preset parses); `cargo check -p warcraft-survivors` and the wasm check
(`-p wenilla … --features webgpu`) unchanged; `cargo test` of the `gpu_blp` mip-drop tests and of
`survivors::` (input math, frame cap cadence, lifecycle suspend/resume, minimap button
geometry); `cargo clippy` with no new warnings in the touched files; and an `aarch64-linux-android`
`cargo check` of `warcraft-survivors-android --no-default-features` with a stand-in for the NDK's
clang (a zig `cc` shim for `CC_aarch64_linux_android` and a patched `android-activity` where
the lock's version needed it), which types every crate for the target and links nothing. Not
done here: a link through a real NDK, a Gradle build (no SDK, no Google Maven), an APK, a frame
on a phone. The CI job is the first place the real NDK sees this code.

**Still device-gated.** The P1 exit (a rendered Elwynn pick screen on both reference devices,
the `blp:` support line in logcat); the whole of the P2 gate (30 fps for 10 minutes, RSS under
1.5 GB, three runs without an OOM on the Mali device: the ASTC sidecar fallback is unwritten);
P2's `onTrimMemory` log line; the P5 device checks (home → return ten times, a call mid-run, an
AAudio disconnect arriving as `DeviceNotAvailable`/`StreamInvalidated`, `KEYCODE_BACK` reaching
`input.rs` as `BrowserBack`); P4 Route B (all-files access and the on-phone folder picker);
P6 release signing and the `targetSdk 35` decision. Until a device has run it, treat the port as
unproven, and the README says so.

## Goal, scope, non-goals

**Goal.** A sideloaded APK that boots the survivors mode (`benilla_app::run_survivors`,
`crates/benilla-app/src/lib.rs:230`) on a mid-range Android phone, playable with touch, offline,
reading a WoW 1.12.1 `Data` folder the user supplies.

| in scope | out of scope (non-goals) |
|---|---|
| survivors mode only (`survivors::ACTIVE`, `lib.rs:231`; `connect: false`, `lib.rs:436`) | the full client: login, realm, FrameXML/Lua UI, chat, addons |
| offline, single player | the realm relay, WebBridge, any network use |
| sideloaded APK (`adb install`, or a file the user copies over) | Google Play, AAB, Play Asset Delivery, in-app updates |
| data-less app: the user supplies their own `Data` folder | shipping, downloading or mirroring game data from the app |
| landscape, phones and tablets, Vulkan GPUs | portrait, GLES-only devices, Chromebooks (untested, not blocked) |
| touch controls that leave keyboard/mouse working everywhere | gamepad (gilrs is compiled but disabled, `benilla-world/src/boot.rs:83`; could come later) |

**Why data-less and why no store.** The MPQ contents are Blizzard's copyright (README, "Legal").
This repository ships engine code only and reads a client the user already owns. An APK carrying
the data would distribute that content; a store listing would do so to the public under our name.
`scripts/fetch-data.sh` pulls from archive.org on the user's own machine and stays a desktop-side
step. The only defensible shape on a phone is the desktop's: our code, their data, their device.
With no data the app boots to a clear "no game data" screen with the import recipe, never to a
download button.

**Provenance.** `crates/benilla-app/src/survivors/` is this repository's own code living inside
upstream's `benilla-app` crate (README, "How it's built"), as are the `warcraft-survivors*`
launchers. Changes there are sync-free. Everything else under `crates/benilla-*` is upstream's and
takes only `#[cfg]`-gated arms or new files (AGENTS.md, "Invariants"). The plan is built to need
very few of those; the complete list is in "Upstream touches" below.

## Route: native, with a web spike in parallel

### Recommendation: native Android (cargo-ndk + Gradle + GameActivity)

A new launcher crate `crates/warcraft-survivors-android` (cdylib, `#[bevy_main]`), built with
`cargo-ndk` into `jniLibs/`, wrapped by a minimal Gradle project using `GameActivity`. This is
Bevy 0.18's own flow and nearly everything it needs is already in `Cargo.lock`: `android-activity
0.6.1`, `bevy_android 0.18.1`, `ndk 0.9`, `jni`. `default_platform` (`Cargo.toml:21-33`) already
enables `android-game-activity` and `android_shared_stdcxx`; Bevy 0.19 drops the former from the
defaults, so the next bump adds it by name. Audio needs no new backend: `cpal 0.17.1` talks AAudio
directly through `ndk 0.9` (API 26+, covered by `minSdk 29`); `oboe` appears in the lock only under
`cpal 0.15.3` ← `rodio` ← `bevy_audio`, which this workspace deliberately leaves off
(`Cargo.toml:19-21`).

Why native over web:

- **Rendering.** The world needs storage buffers in every WGSL shader (WebGL2 is glue-only,
  `benilla-world/Cargo.toml:21-25`), so the browser route needs WebGPU on Android Chrome, whose
  GPU allow-list we do not control. Native wgpu on Vulkan is ours to configure.
- **Data.** The wasm path fetches every file by synchronous XHR from a host that unpacks the chain
  (`benilla-formats/src/web.rs:74-79`). On a phone that is a local HTTP server in the app (then it
  is a native app anyway) or hosting 5 GB per user. Native reads MPQs straight off the filesystem
  by path (`benilla-mpq/src/lib.rs:228`), which is what the user's folder is.
- **Memory.** The wasm build is a 32-bit address space; the texture fallback below can push past
  it. Native aarch64 has the full heap.
- **Lifecycle, audio, back button, storage** exist as Android APIs on the native side and as
  workarounds on the web side.

### Alternative considered: mobile web / TWA

Serve `web/dist` plus `/data` from a wenilla-host-like service, open it in Chrome or wrap it as a
Trusted Web Activity. Zero new build infrastructure, and all touch work is shared Rust. Rejected
as the shipping route (hosted data is the legal problem again, WebGPU availability, 32-bit heap,
no lifecycle control), but kept as **P0**: the touch and UI-scaling work is `#[cfg]`-free Rust in
`survivors/`, and Chrome on Android 12+ ships WebGPU by default on supported Qualcomm and ARM
GPUs (Chrome 121+; the `--enable-unsafe-webgpu` flag is a Linux-desktop thing). The spike costs a
`<meta viewport>` line on `web/index.html` and `wenilla-host` reached over `adb reverse tcp:8090
tcp:8090` (it binds loopback only).

## Phases

Ordered by risk, not by layer: the port is not worth touch work until a phone has rendered a zone
inside its memory budget. P3 (touch) is `survivors/`-only code and runs in parallel from day one.
Sizes: S ≤ 1 day, M ≤ 1 week, L > 1 week, for one implementer. Every phase lands as its own PR
against `main`, so `check.yml` gates it and the pin bot sees nothing it should not ship. Android
code must not break the wasm or native targets, which is what the `cfg` discipline buys.

**Prerequisite, before P1 starts:** one Mali device (Pixel 7/8 class) and one Adreno device
(Snapdragon 7/8 Gen 1-2 class) in hand, plus a developer who owns the Kotlin/Gradle side. Without
the Mali device the P2 gate cannot be run, and P2 is the go/no-go for the whole port.

### P0 · Touch spike on the web page (S, in parallel)

| task | where |
|---|---|
| `<meta name="viewport" content="width=device-width, initial-scale=1, user-scalable=no">` on `web/index.html`; confirm Bevy emits `TouchInput` from the canvas | `web/index.html` (ours) |
| First slice of P3's `survivors/input.rs`: the resource and the virtual joystick, no cfg | `survivors/input.rs` (new) |

Exit: the hero walks with a thumb in Chrome on a phone; WASD unchanged on desktop; wasm check
green. Optional if no WebGPU phone is at hand; its Rust half is P3's first task either way.

### P1 · It builds and renders a zone (M)

The data path for P1 is `adb push`, nothing more: a rendered zone is the exit, not a log line.

| task | where | fork rule |
|---|---|---|
| New crate `crates/warcraft-survivors-android`: `[lib] crate-type = ["cdylib"]`, `#[bevy_main] fn main()` calling `run_survivors(BuildId{..})` as `warcraft-survivors/src/main.rs:5-16`; `build-dependencies = benilla-buildstamp` (runs on the host; fine when cross-compiling). It is a workspace member (`members = ["crates/*"]`), so `cargo check --workspace --all-targets` and `cargo test --workspace` build it on the Linux host too: the shim must compile there (`#[bevy_main]` expands to a plain `main` off-android; keep the JNI/env code under `cfg(target_os = "android")`) | new crate | ours |
| **Env shim** (the fork-invariant trick): before `run_survivors`, read `<filesDir>/benilla-config/env.toml` (`WOW_*` key/value lines) and `std::env::set_var` each, plus `BENILLA_HOME=<filesDir>/benilla-config`, `WOW_DATA=<externalFilesDir>/Data` (falling back to `<filesDir>/Data`), `WGPU_BACKEND=vulkan`. Edition 2021, no app thread exists yet, so `set_var` is sound. `local_state::home()` honours `BENILLA_HOME` (`local_state.rs:36-38`), `wow_data()` honours `WOW_DATA` first (`install.rs:72-76`), a release build without `dev` has no project folder, and every lever such as `WOW_NO_BC` reads `std::env::var` directly (`gpu_blp.rs:40`), so this one shim replaces arms in `install.rs`, `local_state.rs`, `webenv.rs` (`benilla-app/src/webenv.rs:16` is native-pass-through already), `crash.rs` and any `WgpuSettings` edit in `lib.rs` | shim | ours, zero upstream lines |
| Panic capture: Rust's default hook writes stderr, which Android discards. The shim chains a panic hook that calls `__android_log_write` (tag `wsurvivors`) directly, since the hook may fire before `LogPlugin` (`boot.rs:31`, which routes `log` to logcat once up). `crash::install` (`lib.rs:268`) stays as is: with `BENILLA_HOME` set it writes `Diagnostics/crash-*.txt` as on desktop | shim | ours |
| `arboard` has no Android backend: `cfg(not(target_arch = "wasm32"))` → `not(any(wasm32, android))` at `crates/benilla-app/Cargo.toml:84` and `textinput/clipboard.rs:24`; the `not(unix…android)` arms at `:92/:108` already exist | 2 sites | cfg arm |
| Window: no arm in `video.rs` unless the device shows a need; `boot_window_mode` reads `config.toml` and bevy_winit's android backend ignores size and mode anyway. Verify, do not pre-empt | — | none |
| Lua: `third_party/lua-src/src/lib.rs:182-240` special-cases only wasi/wasm32; android takes the generic `cc` path with NDK clang. Confirm in the build log | — | none |
| libc++: after linking, `llvm-readelf -d libwarcraft_survivors_android.so \| grep NEEDED`; if `libc++_shared.so` is listed, build with `cargo ndk --link-libcxx-shared` or package it | script | ours |
| 16 KB pages (Android 15+): NDK r28 (or r27 with `-C link-arg=-Wl,-z,max-page-size=16384` in `.cargo/config.toml` for `aarch64-linux-android`); verify `llvm-readelf -l` shows `LOAD` align `0x4000` and `zipalign -c -P 16` passes | `.cargo/config.toml`, script | ours |
| Gradle (`android/`): `GameActivity` subclass; `androidx.games:games-activity` pinned to the line `android-activity 0.6` expects (4.0.x; a mismatch crashes at startup); `android.app.lib_name` = the cdylib name; `minSdk 29`, `targetSdk 34` (35 enforces edge-to-edge and needs inset handling; revisit in P6); `screenOrientation=sensorLandscape`; `configChanges=orientation\|screenSize\|screenLayout\|keyboardHidden\|density`; `<uses-feature android.hardware.vulkan.version 0x401000 required>`; `FLAG_KEEP_SCREEN_ON`; `largeHeap` left out (it governs the Java heap; native allocations are not under it) | `android/` | ours |
| Lockfile: `android-activity` must stay at one version (0.6) for winit 0.30; `android-setup.sh` asserts it | script | ours |
| `scripts/android-setup.sh` (NDK, `cargo-ndk`, the target) and `scripts/android-build.sh` (`cargo ndk -t arm64-v8a -o android/app/src/main/jniLibs build --release -p warcraft-survivors-android --no-default-features`, then `./gradlew assembleDebug`); `ANDROID_DEBUG=1` keeps symbols for `ndk-stack` | `scripts/` | ours |

Exit: on both reference devices, `adb push Data /sdcard/Android/data/<pkg>/files/Data` then
launch renders Elwynn Forest's pick screen and a run starts; `adb logcat -s wsurvivors:* wgpu:*`
shows the build banner and the `blp:` support line (`gpu_blp.rs:46-51`); `cargo check --target
aarch64-linux-android -p warcraft-survivors-android --no-default-features` passes.

### P2 · On-device rendering and memory: the go/no-go (M)

| task | where | fork rule |
|---|---|---|
| Desktop baseline first: `WOW_NO_BC=1` reproduces the RGBA8 fallback exactly; take RSS and VRAM per zone (`WOW_FPS_JOURNAL`) before touching a phone | desktop | none |
| On the Mali device (no BC) and the Adreno device (many 6xx/7xx drivers do expose BC): a 10-minute run in Elwynn and in Naxxramas with `dumpsys meminfo`, the FPS journal, and `PowerManager.getThermalHeadroom` logged beside it | devices | none |
| Mip dropping: on android, drop the top 1-2 mips of chains wider than 512 in `for_upload`'s decode arm (`gpu_blp.rs:71-89`); `WOW_MIP_DROP=n` lever, android default 1 via `env.toml` | `gpu_blp.rs` | cfg arm (one) |
| Presets through existing cvars (`benilla-ui/src/script/cvars.rs`): `farclip`, clutter density, MSAA, `renderScale` 0.75-0.85, anisotropic, `spellEffectLevel`, `grade` off; an android default `config.toml` shipped in the APK's assets and copied to `benilla-config` on first run | `android/`, config | ours |
| 30 fps cap: `PresentMode::Fifo` plus a frame limiter in the survivors plugin, for thermals; High preset may lift it | `survivors/` | ours |
| `onTrimMemory`/`onLowMemory` → a log line through JNI, so an OOM kill has a preceding warning in the journal | `android/` | ours |

Exit (the gate): on the Mali device, 30 fps sustained for 10 minutes in both zones, RSS under
1.5 GB, no OOM across three runs. If mip drop plus presets miss it, the fallback is an ASTC
sidecar cache transcoded **on the desktop** and pushed with the data (see "Rendering and memory"),
not an on-phone encode. If that misses too, the port stops here with the numbers recorded.

### P3 · Touch input and UI scaling (L, parallel from day one)

See "Touch control design". Files: `survivors/input.rs` (new), `hero.rs:187-311` (`drive_hero`
reads `SurvivorsInput` instead of `ButtonInput<KeyCode>`), `hero.rs:148-183` (`aim_cursor` takes a
screen point from the resource), `hero.rs:313-380` (`place_camera`: pinch → `rig.dist`, `C` →
resource), `mobility.rs:13` (`KEY` stays, also fires on the touch button), `cards.rs:509-533`
(`choose_card` already takes `Interaction::Pressed`), `hud.rs:1054-1085` (Esc/Enter → resource),
`hud.rs:398-460` (the `MobilitySlot` node becomes a `Button`), `minimap.rs:265-300` and
`menu.rs:34-38` (hit-target sizes). All survivors code, so sync-free and cfg-free.

Exit: a full run (pick → play → level up → pause → die → results → menu) completes with touch
only on a 6" phone, no on-screen element under 48 dp (checked in logical px, which are dp on
Android); desktop and web play as before (`CONTROLS.md` unchanged).

### P4 · Data import (M)

v1 ships the two routes that need no copy, because `benilla-mpq` opens archives by real path and
a SAF copy of 5.5 GB into `filesDir` doubles the storage (11 GB peak) and needs a foreground
service with `.partial` staging. That becomes the follow-up.

| task | where |
|---|---|
| Route A, no permission: `adb push Data /sdcard/Android/data/<pkg>/files/Data`; `WOW_DATA` points there (the shim's first choice). `make android-data` (`scripts/android-data.sh`) runs `fetch-data.sh` on the desktop and pushes with `--sync` | shim, docs, script |
| Route B, on-phone: `MANAGE_EXTERNAL_STORAGE` ("All files access", a settings toggle the panel deep-links to), then a folder picker that returns a real `/storage/emulated/0/…` path, written to `env.toml` as `WOW_DATA` | `android/` (Kotlin), shim |
| "No game data" panel in `survivors/menu.rs` when `wow_data()` is `None`: what is needed, whose data it is, the two routes, a **Validate** button | `survivors/menu.rs` |
| Validation: `Chain::open` (`chain.rs:97-125`) on the folder and the first error shown; the needed names are `mount_order`'s (`chain.rs:77`, private), so the Kotlin side does not try to know them and validation stays in Rust | shim |
| Follow-up, not v1: SAF `ACTION_OPEN_DOCUMENT_TREE` copy into app-private storage with a foreground service and `StatFs` check; a traced subset (what the sixteen zones in `survivors/data.rs:1389+` touch, captured as `?boottrace=1` does on web, `web.rs:49`) to shrink what is copied | later |

Exit: fresh install → panel → either route → validated → pick screen; the pick round-trips
through `survivors-pick.txt` under `BENILLA_HOME`; uninstall leaves nothing the app created
outside its own folders.

### P5 · Lifecycle and audio (M)

| task | where | fork rule |
|---|---|---|
| `AppLifecycle::WillSuspend` (nothing handles it today): set `Phase::Paused`, stop the game clock, stop the output stream so a backgrounded app plays no audio and burns no battery | `survivors/lifecycle.rs` (new); a `cfg(android)` hook in `sound/output/mod.rs` if the stream has no public stop | new file + ≤ 1 arm |
| Audio resume: `sound/output/mod.rs:27` and `cpal.rs:164-171` already rebuild the stream on `DeviceNotAvailable`/`StreamInvalidated`. Verify on device that an AAudio disconnect arrives as one of those before writing anything | device | likely none |
| Surface loss: Bevy 0.18 recreates the surface on `Resumed`; `MsaaSupportPlugin` (`benilla-world/src/view.rs:72-76`) and `BlpGpuSupportPlugin` (`gpu_blp.rs:36-52`) cache adapter facts at `finish`, which hold because the adapter does not change. Guard: `Camera::is_active = false` between `WillSuspend` and the first frame after `Resumed` | `survivors/lifecycle.rs` | ours |
| Back: `GameActivity` may `finish()` on Back; register an `OnBackPressedCallback` in the Kotlin subclass so the key reaches native, `enableOnBackInvokedCallback=false` for now. winit 0.30 maps `KEYCODE_BACK` to `KeyCode::BrowserBack`; confirm on device, then map in `input.rs`: `Paused` ↔ `Playing` like Esc; on the menu a second press within 2 s exits (`AppExit`, as `hud.rs:1045`) | `android/`, `input.rs` | ours |

Exit: home → return ten times mid-run, no crash, sound back; a phone call mid-run resumes
paused; Back behaves as above.

### P6 · Packaging, CI, docs (S/M)

| task | where |
|---|---|
| `check.yml`: a separate `android` job (`ci.yml:35` deletes `/usr/local/lib/android` in its own job, so nothing preinstalled is assumed) using `nttld/setup-ndk` pinned, `cargo-ndk`, and `cargo check --target aarch64-linux-android -p warcraft-survivors-android --no-default-features`; the NDK clang is needed even for `check` because `lua-src`'s `cc` build runs (`CC_aarch64_linux_android`) | `.github/workflows/check.yml` |
| Release: `cargo ndk … --release` stripped, `assembleRelease`, `apksigner` v2+v3 with a keystore the operator keeps outside the repo; a debug-signed install cannot be upgraded by a release-signed one, so testers reinstall once | `scripts/android-build.sh`, docs |
| `targetSdk` decision: stay on 34 for v1 (35 enforces edge-to-edge); if raised, `layoutInDisplayCutoutMode=shortEdges` and safe-area padding on the HUD's edge anchors | `android/` |
| `make android`; README "Play it on Android"; `CONTROLS.md` touch table; AGENTS.md table rows for `crates/warcraft-survivors*`, `survivors/` and `android/`; UPSTREAM.md's carry list gets "Upstream touches" below | docs |

Exit: an Android-only break fails `check.yml`; `make android` builds an installable APK from a
clean clone with the NDK installed.

## Upstream touches

The complete list of upstream lines the port changes; anything beyond it needs the reviewer's
sign-off and a line here.

| site | change |
|---|---|
| `crates/benilla-app/Cargo.toml:84`, `textinput/clipboard.rs:24` | `arboard` gate: `not(any(wasm32, android))` |
| `crates/benilla-assets/src/gpu_blp.rs:71-89` | `cfg(android)` mip-drop arm in the decode path |
| `crates/benilla-app/src/sound/output/mod.rs` | at most one `cfg(android)` stop hook, if P5 finds no existing way to stop the stream |
| `Cargo.toml` (bevy features) | `android-game-activity` by name at the 0.19 bump |

What landed differs from the table in two places, signed off in the review below and recorded
in UPSTREAM.md "Android carries" for the sync: the mip drop is a cfg-free `WOW_MIP_DROP` lever
whose *default* is the `cfg(android)` (so the desktop A/B needs no rebuild), and the P5 stop is a
cfg-free `OutputBackend::set_suspended` plus `Mixer::set_suspended` rather than a `cfg(android)`
hook, since the browser takes the gate half of it too. The logic of both lives in new files
(`gpu_blp/mip_drop.rs`, `sound/mixer/suspend.rs`, `sound/output/suspend.rs`); the upstream files
carry only `mod` lines, the one call in `for_upload`, and the `suspended` field with its one
condition in `OutputBackend::service`. All of it is inert off Android.

A cfg-audit sweep (see roles) confirms before P1 that no other `cfg(not(target_arch = "wasm32"))`
site assumes a desktop filesystem or `current_exe()` beyond what `BENILLA_HOME`/`WOW_DATA` cover.

## Touch control design

The desktop controls stay the reference (`CONTROLS.md`). Touch adds a second way to produce the
same intents, and the intents are what the game reads.

### Input abstraction

A `survivors/input.rs` with one resource and one system that runs first in `Update`:

```rust
#[derive(Resource, Default)]
pub(super) struct SurvivorsInput {
    pub mov: Vec2,                 // -1..1 per axis, world-relative like WASD
    pub aim: Option<Vec2>,         // a screen point to raycast, else the cursor
    pub jump: bool, pub mobility: bool, pub pause: bool, pub confirm: bool,
    pub camera_toggle: bool, pub grade_toggle: bool,
    pub pick_card: Option<usize>,
    pub zoom: f32,                 // multiplicative notches, wheel or pinch
    pub orbit: Vec2,               // drag delta when the camera is unlocked
}
```

`collect_input` fills it from, in order: keyboard and mouse (the existing `KeyCode`/`MouseButton`
/`MouseWheel`/`AccumulatedMouseMotion` reads, moved verbatim out of `drive_hero`, `place_camera`,
`mobility`, `cards`, `hud`), then `Res<Touches>`. Widget hit-tests win over zone gestures: a touch
that lands on a bevy_ui `Button` (`Interaction::Pressed`) is claimed by it and never starts a
joystick, aim or pinch. Pinch uses a two-touch latch: it starts only when two touches are down in
the aim zone and ends when either lifts, so a thumb resting on the joystick cannot join one. Any
touch sets a `TouchSeen` flag that switches the HUD into touch layout; a key press switches it
back.

### Zones and widgets

| control | touch | desktop equivalent |
|---|---|---|
| move | virtual joystick under the first touch in the left 45 % of the screen, floating, 120 dp radius, 15 % dead zone; `mov` is the normalized offset | WASD |
| aim | right 55 %: a drag or tap sets `aim` to its screen point (`aim_cursor` raycasts it as it does the cursor, `hero.rs:157`); when no touch has aimed for 1.5 s, auto-aim the nearest enemy within 30 yd, else `None` (the hero then faces its move direction, `hero.rs:237-249`, graceful today) | cursor |
| mobility (F) | the mobility slot (`hud.rs:398-460`) becomes a `Button`, sized by the dp rule below; fires toward `aim` or forward (`mobility.rs:204-206`) | F |
| jump | a button above-right of the mobility slot | Space |
| pause / back | a button top-left; Android Back; both → `pause` | Esc |
| cards | tap (`Interaction::Pressed`, `cards.rs:520`; works today); the tilt keys off `RelativeCursorPosition` and simply stays flat | 1/2/3, hover |
| zoom | pinch: `zoom = (d1/d0 - 1) * k`, feeds `rig.dist` at `hero.rs:355` | wheel |
| orbit | two-finger drag when unlocked | mouse drag |
| camera lock, grade | two toggles in the pause panel (`hud.rs:2191` `show_panels`) | C, G |
| tooltips | long-press (500 ms) on an icon shows `icon_tooltip` (`hud.rs:2070`) until release; `xp_hover` (`hud.rs:1595`) likewise on the XP bar | hover |
| menu rows, minimap buttons | `ROW_H` (`menu.rs:37`) and the 32-unit map buttons (`minimap.rs:258`) grown by the dp rule; `HoverLabel` becomes tap-to-highlight | hover/click |

### UiScale and hit targets

`apply_ui_scale` sets `UiScale = window.height() / 768` (`hud.rs:1495-1504`), and
`Window::height()` is logical pixels, which on Android are dp. A 2400×1080 phone at DPR 2.6 is
923×415 dp, so `UiScale ≈ 0.54` and a 40-unit button is ≈ 22 dp. The scale itself is right (it is
what fits the 768-unit layout on the screen) and stays; raising it would push a 768-unit layout
onto a 415 dp screen. Touch widgets are instead sized **from the dp target**: on touch layout a
widget's edge is `units = 48.0 / UiScale.0` (≈ 89 units at 0.54, ≈ 48 at 1.0 on a tablet), read
from the `UiScale` resource in the same system that spawns the HUD. The exit criterion is checked
in logical px: every `Button` in touch layout measures ≥ 48 × 48 after layout.

## Rendering and memory

### Textures

`BlpGpuSupportPlugin` (`gpu_blp.rs:36-52`) tests `CompressedImageFormats::BC` at `finish`; without
it `for_upload` (`:71-89`) decodes BC1/2/3 to RGBA8 on the CPU with `texpresso`. Mali and PowerVR
report no BC; many Adreno 6xx/7xx Vulkan drivers do expose `textureCompressionBC`, so the Mali
device is the mandatory memory gate and the `blp:` log line (`gpu_blp.rs:46-51`) is recorded per
device in the matrix. The fallback costs 4× (BC3) to 8× (BC1) VRAM per texture plus decode time
per upload, in a `texture_2d_array` sized to `max_texture_array_layers`. Biggest risk on the port.

Options, in order:

1. **Measure first**, `WOW_NO_BC=1` on the desktop (P2).
2. **Mip dropping** (S): the one `gpu_blp.rs` arm; quarters the cost; the top-down camera never
   sees texel detail at 1:1.
3. **Presets via existing cvars** (S): `renderScale` and `farclip` are the big levers; grade and
   bloom off by default on android.
4. **ASTC sidecar cache** (L): decode BC → encode ASTC 6×6 **on the desktop** (every Vulkan 1.1
   Android GPU reads it), written as a cache keyed by MPQ name + file hash and pushed beside the
   data; a new `BlpTexels` arm reads it. Not an on-phone encode. Only if 2 + 3 miss the P2 gate.

### Minimum spec

| | minimum | target |
|---|---|---|
| Android | 10 (API 29: scoped storage, AAudio, Vulkan 1.1 required by the CDD for 64-bit) | 13-14 (API 33-34, `targetSdk 34`) |
| ABI | arm64-v8a only | — |
| GPU | Vulkan 1.1, storage buffers in vertex and fragment stages (every WGSL shader needs them), ≥ 256 texture array layers | Adreno 640+ / Mali-G77+ |
| RAM | 6 GB (RGBA8 fallback) | 8 GB |
| storage | 6 GB free (5.5 GB chain + headroom) | — |

GLES is not an option: `webgl2` is a glue-only feature. The shim sets `WGPU_BACKEND=vulkan` so a
GL fallback never half-works.

### Surface loss

On pause Android destroys the window; Bevy 0.18's winit backend recreates the surface on
`Resumed`. The adapter-derived caches (`MsaaFormats`, `view.rs:43-50`; the BC flag) are
per-adapter and hold; render-world textures are not on the surface and hold; anything holding a
surface `TextureView` must not. The cheap guard is the P5 `Camera::is_active` gate.

## Data import UX (v1)

1. No `Data` found: the "No game data" panel replaces the pick screen. Text: a WoW 1.12.1 `Data`
   folder from a client you own (~5.5 GB), two routes, a **Validate** button.
2. **With a PC**: `adb push Data /sdcard/Android/data/<pkg>/files/Data`; press Validate.
3. **On the phone**: **Allow all files access** opens the system toggle; **Choose folder** then
   returns a real path, stored as `WOW_DATA` in `env.toml`; press Validate. The folder stays
   where it is; nothing is copied.
4. Validate runs `Chain::open`; the first error is shown verbatim; success shows the pick screen.
5. Follow-up: SAF copy into app-private storage (foreground service, `.partial` staging, `StatFs`
   check) for users who want the data removed on uninstall; a traced subset to shrink it.

## Agents and roles

| workstream | agent / model | why | hand-off |
|---|---|---|---|
| plan review, P2 go/no-go, P5 design, every line in "Upstream touches" | Opus-class reviewer | architecture, risk, fork-invariant judgment | reviews each phase PR before merge; the P2 numbers go to the reviewer for the verdict |
| P1 shim/Gradle, P2 mip drop and presets, P3 input refactor, P4 panel and picker, P5 lifecycle | Fable-class implementer | implementation-heavy Rust and Kotlin, long sessions on one PR | opens the PR with the exit criteria as a checklist |
| Kotlin/Gradle owner with a physical device | one named person | GameActivity, signing, manifest, and the device-side half of every exit criterion | fills the device matrix; a red cell blocks the phase |
| cfg-gating audit (every `cfg(not(wasm32))` assuming a desktop fs or `current_exe`) producing the exact upstream-touch list; doc sweeps; lockfile single-version check; line-number refresh after a sync | Haiku/Sonnet-class or Explore agents | mechanical, broad, low judgment | a table the implementer acts on; the audit runs before P1 |
| ASTC sidecar, traced subset | Fable-class, Opus review of the on-disk format | formats are hard to walk back | design note here first, PR second |
| CI job, release signing | implementer; the operator holds the keystore | the keystore never enters the repo | operator step in P6 |

Review gates: every phase PR needs the reviewer's pass on fork rules and the device owner's matrix
row. P2 and P5 attach before/after `WOW_FPS_JOURNAL` csv and `dumpsys meminfo` to the PR.

## Risk register

| risk | likelihood | impact | mitigation |
|---|---|---|---|
| RGBA8 fallback blows the memory budget on Mali | high | high | P2 is the gate; desktop `WOW_NO_BC=1` first; mip drop; presets; desktop-side ASTC cache; stop if all miss |
| A Vulkan driver rejects a shader or layout (storage buffer in the vertex stage, array layers) | medium | high | pin Vulkan; both GPU families in P1; the matrix is the gate, CI cannot see it |
| GameActivity / `android-activity` version mismatch crashes at startup | medium | low | pin `games-activity` 4.0.x; assert one `android-activity` in the lock |
| Surface recreation on resume breaks a cached view | medium | medium | P5 camera gate; ten home/return cycles |
| AAudio disconnect not surfaced as `DeviceNotAvailable`/`StreamInvalidated` | low | medium | verify on device; one `cfg(android)` hook if needed |
| 16 KB page alignment missed, install refused on Android 15+ | medium | medium | NDK r28 / linker flag; `zipalign -c -P 16` in the build script |
| Thermal throttling kills a 10-minute run | medium | medium | 30 fps cap; presets; log thermal headroom beside the journal |
| Upstream sync conflicts on the cfg arms | low | low | four small sites, listed in UPSTREAM.md |
| Bevy 0.19 drops `android-game-activity` from defaults | certain at the bump | low | feature by name |
| Legal exposure from an APK that appears to ship Blizzard content | low with a data-less APK | high | no data, no download, no store; the panel's text says whose data it is |
| `benilla-buildstamp` fails cross-compiling without git | low | low | runs on the host for every target; stamps `unknown` |

### Open questions for the owner

1. Which Mali and which Adreno device? Both are needed before P1; the Mali one is the gate.
2. Is `MANAGE_EXTERNAL_STORAGE` acceptable for v1, or must on-phone import copy into app-private
   storage from the start (the SAF follow-up)?
3. Auto-aim when no touch is aiming: on (recommended) or strictly manual?
4. `--no-default-features` for the android launcher (no `dev` captures/probes, as the web build),
   or keep them behind `env.toml`?
5. 30 fps locked for v1, or 60 fps on High?

## Testing strategy

| layer | what | where |
|---|---|---|
| unit, host | `input.rs`: joystick normalization, dead zone, pinch latch and zoom notches, Back → pause transitions, widget-over-zone precedence; `install.rs` candidates via `candidates_from`; `local_state` with `BENILLA_HOME` pinned (`test_env::EnvGuard`); the shim's `env.toml` parser | `cargo test -p benilla-app --lib survivors::input`, no data; `check.yml` `client-carries` |
| compile, CI | the new `android` job: `cargo check --target aarch64-linux-android -p warcraft-survivors-android --no-default-features` with `nttld/setup-ndk`; catches a `current_exe`-style assumption only if it fails to compile, which is why the cfg audit exists | GitHub Actions |
| compile, CI | the existing wasm, native and `cargo test --workspace` paths must stay green with the new member | existing jobs |
| emulator | API 34 arm64 image for install, panel, picker, lifecycle and Back only. **Not for rendering or memory:** its SwiftShader/ANGLE Vulkan is slow, reports limits real GPUs lack, and reports BC support Mali lacks, so it hides the fallback cost | developer machine |
| device matrix | per phase exit criterion, by the device owner | phones |

| device | GPU | Android | RAM | `blp:` line | P1 renders | P2 fps / RSS | P3 touch run | P4 import | P5 resume ×10 |
|---|---|---|---|---|---|---|---|---|---|
| reference Mali (Pixel 7/8, Tensor) | Mali-G710/G715 | 14 | 8-12 GB | | | | | | |
| reference Adreno (Snapdragon 7/8 Gen 1-2) | Adreno 6xx/7xx | 13-14 | 8 GB | | | | | | |
| minimum-spec | Adreno 640-class | 10-11 | 6 GB | | | | | | |
| tablet | any | 13+ | 8 GB | | | | | | |

Performance captures use `WOW_FPS_JOURNAL=<csv>` through `env.toml` (written under
`BENILLA_HOME/Diagnostics`, pulled with `adb pull`), comparable with [PERFORMANCE.md](PERFORMANCE.md).

## Where to look when

| symptom | look at |
|---|---|
| the APK exits at once | `adb logcat -s wsurvivors:* wgpu:*`; a tombstone under `/data/tombstones` through `ndk-stack`; `games-activity` vs `android-activity` version; a duplicated `android-activity` in `Cargo.lock` |
| install refused on Android 15 | 16 KB page alignment: `llvm-readelf -l`, `zipalign -c -P 16` |
| "no install" after a push | `WOW_DATA` in the shim's log line; `adb shell ls /sdcard/Android/data/<pkg>/files/Data` |
| textures crawl in, then OOM | `blp: device reports no BC support`: the RGBA8 fallback; P2 options in order |
| no sound after returning from home | whether `cpal.rs:164-171` saw the disconnect; the P5 hook |
| touch works on the web spike, not in the APK | the Java side: `GameActivity` input filtering; the Rust half is shared |

## Review log

Reviewed by an Opus-model reviewer against `dd5e330`; verdict "approve with changes", all
applied: phases reordered risk-first (boot by `adb push` → on-device render/memory gate on Mali
and Adreno → touch in parallel → import → lifecycle → packaging); the planned `cfg(android)` arms
in `install.rs`, `local_state.rs`, `webenv.rs`, `crash.rs` and `lib.rs` replaced by an env-var
shim in the launcher (`BENILLA_HOME`, `WOW_DATA`, `WGPU_BACKEND`, `env.toml`), leaving four
upstream touches; the UiScale floor replaced by dp-derived widget sizes (`48 / UiScale`); the
oboe/AAudio description corrected (cpal 0.17 uses AAudio directly; existing stream rebuild noted);
file references fixed (`arboard` at `benilla-app/Cargo.toml:84`, `webenv.rs` in benilla-app,
`lua-src/src/lib.rs:182-240`, `mount_order` private at `chain.rs:77`, `video.rs:547/569` is
`display_session`); panic hook to logcat specified; 16 KB pages, GameActivity pin, Back callback,
manifest and signing tasks added; import v1 narrowed to `adb push` and all-files access, SAF copy
deferred; CI job given its own NDK setup; P0 WebGPU note corrected; optional suggestions folded in
(Kotlin owner and device procurement, widget-over-zone precedence and pinch latch, Back key
confirmation, desktop-side ASTC, provenance note).

**First execution pass, reviewed on `claude/android-conversion-plan-7fj17k`** (the four tracks:
launcher + Gradle + scripts, touch input and UI, lifecycle/no-data/frame cap/mip drop, CI and
docs). Verdict "approve with changes", all applied in the review commit:

- `input::collect` read Android's Back through `keys.just_pressed(..) || keyboard.read().any(..)`,
  which short-circuits and leaves the `KeyboardInput` reader undrained on the frames the first
  test matches; the same press was read again next frame, a double Back that exits at the menu.
  The reader now drains every frame.
- The Mali preset was documented as seeded into `env.toml` on first run, but nothing wrote it.
  The launcher now compiles it in (`env_shim::DEFAULT_ENV_TOML`) and writes it when the file is
  absent; a host test parses it. The `spellEffectLevel` seed is not done and is marked open.
- The upstream-file churn was cut down: the mip-drop helpers and tests, `Mixer::set_suspended`
  and `OutputBackend::set_suspended` moved into new child-module files, leaving `mod` lines, the
  `for_upload` call, the `suspended` field and one condition in upstream files. The resume's
  `Instant::now() - REOPEN_EVERY` became a `checked_sub`.
- `crates/warcraft-survivors/Cargo.toml`'s added bevy `default_font` dependency was reverted:
  the workspace's `default_platform` feature already carries `default_font`.
- The manifest's `configChanges` widened to every change the native side can live through
  (`uiMode`, `keyboard`, `smallestScreenSize`, `fontScale`, `navigation`, `locale`,
  `layoutDirection`), so a keyboard, dark mode or font-scale change no longer recreates the activity.
- Checked and kept: desktop input unchanged (WASD, wheel, drag-orbit, `F`, `C`, `G`, Esc, Enter
  all route through `SurvivorsInput` with the same semantics; the joystick `pace` is 1 for keys
  and for the normalized auto-runner), the frame cap native-only on `bevy::platform` time, the
  lifecycle tests, the mip-drop math, `lib_name` = the cdylib, games-activity 4.4.0 matching
  android-activity 0.6.1's README, the 16 KB flag and the build script's `LOAD` check, the CI job.
