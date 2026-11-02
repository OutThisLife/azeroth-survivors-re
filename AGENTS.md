# Working in this repository

wenilla is a fork of [samwhosung/benilla](https://github.com/samwhosung/benilla), a from-scratch
WoW 1.12.1 client in Rust + Bevy, that adds a browser build and a realm service to host it. The
fork rides on upstream: upstream is merged into `main` as it publishes, and our own work lives
on the same branch alongside it. This file is the map for a person or an agent who has to build,
fix or ship something here. The path from a merge to a running realm is documented in
[wenilla-realm/docs/RELEASE.md](https://github.com/Arnesen/wenilla-realm/blob/main/docs/RELEASE.md);
syncing upstream is [docs/UPSTREAM.md](docs/UPSTREAM.md). Upstream's own map and rules for the
client crates are `docs/METHOD.md`, `docs/MAP.md` and `docs/CONTRIBUTING.md`; read them before
changing client code, and treat this file as authoritative for the fork's crates and process.

## What lives where

| path | what | whose |
|---|---|---|
| `crates/benilla-*` | the client: formats, world, UI, protocol, app | upstream's, plus our `#[cfg]`-gated carries |
| `crates/benilla-app/src/survivors/` | the survivors mode: the pick screen, hero, enemies, cards, touch input (`input.rs`), lifecycle, frame cap, the no-data panel | this repository's own code, inside upstream's crate; cfg-free changes are fine |
| `crates/warcraft-survivors` | the native desktop launcher (`cargo survivors`) | this repository's |
| `crates/warcraft-survivors-android` | the Android launcher: `GameActivity` cdylib, env shim (`BENILLA_HOME`, `WOW_DATA`, `env.toml`), panic hook to logcat | this repository's; see docs/ANDROID.md |
| `android/` | the Gradle project that packages the cdylib into an APK (`MainActivity : GameActivity`) | this repository's; no Rust here |
| `scripts/android-setup.sh`, `scripts/android-build.sh`, `scripts/android-data.sh`, `scripts/android-env.default.toml` | toolchain and build for the APK, putting the game data on a phone over adb, and the `env.toml` preset the shim seeds on first run | this repository's |
| `crates/benilla-app/src/webbridge/` | the JavaScript bridge, wasm side | ours |
| `crates/wenilla` | the wasm entry crate (`#[wasm_bindgen(start)]`) | ours |
| `crates/wenilla-host` | local dev server: static files, `/data/*`, `/ws/*` relay. Loopback only. | ours |
| `crates/wenilla-realm` | the realm service: login, play page, admin panel, relay. This runs in prod. | ours |
| `crates/wenilla-realm/templates/play.html` | the realm's player page (Askama, compiled into the binary) | ours |
| `web/` | the page side: `index.html` (dev page), `boot.js`, `platform.js`, `bridge.js`, `examples/`, `wasi_stubs.js` | ours |
| `scripts/web-setup.sh`, `scripts/web-build.sh` | toolchain and build for the wasm | ours |
| `third_party/mlua-sys` | Lua 5.1's C build for wasm | ours (vendored) |
| `.github/workflows/check.yml` | the PR gate: wasm check, Android check, realm tests | ours |
| `.github/workflows/bump-realm-pin.yml` | tells wenilla-realm about every new `main` head | ours |

## Commands

```bash
scripts/web-setup.sh        # once per machine: wasm target, wasm-bindgen-cli at Cargo.lock's exact version, wasi-sdk, binaryen. No sudo.
scripts/web-build.sh        # → web/dist (WebGPU build). WEB_DEBUG=1 keeps symbols for readable stack traces.
cargo run --release -p wenilla-host -- --www web/dist --data /path/to/WoW/Data --upstream 127.0.0.1
                            # http://127.0.0.1:8090/ — needs a realmd:3724 and mangosd:8085 at --upstream
cargo check --target wasm32-unknown-unknown -p wenilla --no-default-features --features webgpu
                            # the wasm build without linking; what check.yml runs
cargo check --workspace --all-targets      # native (Debian/Ubuntu: libasound2-dev libudev-dev)
cargo ndk -t arm64-v8a -P 29 check -p warcraft-survivors-android --no-default-features
                            # the Android build without linking; what check.yml's android job runs (needs ANDROID_NDK_HOME, cargo-ndk)
scripts/android-setup.sh    # once per machine: the aarch64-linux-android target, cargo-ndk, NDK and lockfile checks. `make android-toolchain`
scripts/android-build.sh    # → android/app/build/outputs/apk/ via cargo-ndk and Gradle. `make android`; ANDROID_DEBUG=1 keeps symbols
scripts/android-data.sh     # fetch-data.sh on this machine, then adb push Data into the installed app's files folder. `make android-data`
cargo test -p wenilla-realm                # the realm service end to end, mock SOAP, sqlite; no game data needed
cargo test --workspace                     # ~320 tests need a 1.12.1 patch chain and fail without one, on pristine upstream too
```

A browser for testing needs WebGPU; the world does not run without it. Linux Chrome needs
`--enable-unsafe-webgpu`. Without an adapter the client aborts in
`wgpu::create_bind_group_layout` at world entry: that is the missing adapter, not your change.
A throwaway profile with a devtools port, for driving it from a script:

```bash
google-chrome --user-data-dir=/tmp/wenilla-chrome --enable-unsafe-webgpu --remote-debugging-port=9333 http://127.0.0.1:8090/
```

## Invariants

- **Carries are `#[cfg]`-gated or in our own files.** A change inside an upstream file goes
  under `#[cfg(target_arch = "wasm32")]` or `cfg(not(target_os = "macos"))`, or into a new
  file. The next upstream merge conflicts on every upstream line a carry touches, so touch few.
- **Upstream is merged, never copied.** A sync is a real merge commit with `upstream/main` as
  its second parent. A single-parent content copy loses the ancestry and re-raises every
  conflict at the next sync. See docs/UPSTREAM.md.
- **`main`'s head is what prod builds.** A push to `main` makes the pin bot write the sha into
  wenilla-realm's `upstreams.env`, and its CI builds the images. Do not merge to `main` what you
  would not ship. `main` accepts PRs only and cannot be force-pushed (repository ruleset).
- **Never press GitHub's "Sync fork" button.** On a diverged fork its "discard commits" path
  hard-resets `main`. Sync through a PR.
- **What ships is the `cp` line in `scripts/web-build.sh`.** A page-side file that is not on it
  never reaches `web/dist`, so never reaches the realm image. `play.html` imports page modules
  dynamically and never load-bearing, so a missing file is a console warning, not a black canvas.
- **Two pages, not one.** `web/index.html` is the dev page (`wenilla-host`); the realm's players
  see `crates/wenilla-realm/templates/play.html`. A page feature exists only where it was added.
- **`wasm-bindgen` crate and CLI versions must match exactly**; `web-setup.sh` enforces it.
- **No threads, no std time on wasm.** `std::time::Instant`, `SystemTime` and `env::vars_os`
  panic there; use `bevy::platform::time::Instant` or `web-time`. Upstream has no CI and never
  builds the wasm target, so a wasm-only break in an upstream change is caught only by
  `check.yml` or your own `cargo check --target wasm32…`.
- **`ring_reaction` returns 0..7**; every consumer adds `+ 1` for the Lua 1..8 scale.
- **Askama templates compile at build time.** A broken `play.html` fails `cargo build -p wenilla-realm`.
- **`gh pr create` needs `-R Arnesen/wenilla`.** Without it `gh` targets the fork's parent,
  where pull requests are closed.

## How a change ships

1. Branch from `main`, open the PR against `Arnesen/wenilla`. `check.yml` runs the wasm check
   and the realm tests.
2. Merge. Squash is fine for ordinary work; a sync PR must be merged with a merge commit.
3. Within a minute the pin bot commits `WENILLA_COMMIT=<sha>` to wenilla-realm. About 25
   minutes later `ghcr.io/arnesen/wenilla-realm:latest` is the new build and the smoke job has
   booted it.
4. The operator runs `./realmctl update` on the VM. Nothing deploys itself.

## Where to look when

| symptom | look at |
|---|---|
| the wasm panics at boot with `RuntimeError: unreachable` | rebuild with `WEB_DEBUG=1`; check for a WebGPU adapter first; `?bridge=0` rules the bridge out |
| a page feature works on the dev page and not on the realm | the `cp` line in `web-build.sh`, then `play.html` vs `index.html` |
| wasm-bindgen glue errors at load | version drift: rerun `scripts/web-setup.sh` |
| an upstream merge broke only the wasm build | `cargo check --target wasm32…`; the `cvars.rs` struct-vs-tuple case is the pattern |
| no sound in the browser | both pages resume the AudioContext on the first gesture; the per-target mixer backend is our carry in `sound/mixer.rs` |
| the world freezes ~200 ms at zone lines | the soundscape carry (`sound/zone.rs`, `sound/web_load.rs`), wasm-only |
| short stalls in the browser on the first play of a sound (combat, a new creature, footsteps on new ground) | the SFX carry (`sound/kit/deferred.rs`, the glue and cinematic arms of `sound/glue.rs`/`sound/cinematic.rs`), wasm-only |
| the prod image is not what `main` says | wenilla-realm/docs/RELEASE.md, "When it breaks" |
| anything on the Android build: the APK exits at once, no data found, no sound after returning from home | docs/ANDROID.md, "Where to look when" |
