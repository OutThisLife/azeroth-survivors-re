# Syncing upstream benilla

Upstream is [samwhosung/benilla](https://github.com/samwhosung/benilla). Since September 2026 it
takes issues and pull requests (`docs/CONTRIBUTING.md`, `docs/METHOD.md`) and moves `main` only by
squash-merged pull requests; it still never builds the wasm target, so a wasm-only fix is still
ours to carry. A small, wasm-neutral fix is now worth offering upstream instead of carrying. This fork tracks it: `main` is upstream's head plus
our carries, and the intent is to stay current while our own work keeps living alongside.

## The model

- `main` is the only long-lived branch. Upstream is merged **into** it as an ordinary merge
  commit; our carries (the browser port, the bridge, the realm service, the wasm-only fixes) are
  ordinary commits on it. Nothing is rebased onto upstream, and nothing waits on a side branch
  "until upstream fixes it": a fix we need ships from `main`, and if upstream later ships its
  own, the carry is dropped in the sync that brings it.
- A carry is anything that makes `git diff upstream/main main -- crates` non-empty outside our
  own crates. Keep every carry `#[cfg]`-gated or in a new file. The conflict cost of a sync is
  proportional to how many upstream lines our carries touch, so touch few.
- Upstream cannot fix a wasm-only problem for us, because it does not build wasm. Such fixes are
  permanent carries: the zone soundscape loading off the frame, the AudioContext resume on the
  pages, the mixer's per-target backend. Drop one only when upstream ships an equivalent — as the
  2026-09-07 sync did to half of one: decision 1920 gave upstream's owned device layer a cpal
  half, so Linux and Windows now run it and the backend split narrowed from macOS-vs-rest to
  native-vs-wasm32. Narrowing a carry onto upstream's new code is the goal; keeping the old shape
  because it still compiles is how a carry rots.

## Procedure

```bash
git fetch upstream                          # remote "upstream" = https://github.com/samwhosung/benilla
git log --oneline main..upstream/main       # what is new; empty means there is nothing to sync
git switch -c sync-upstream main
git merge upstream/main                     # a real merge; conflicts are expected in the carry files
```

Resolve each conflict by keeping our structure and taking upstream's logic inside it: the
`dispatch()` extraction in `net/io.rs` carries upstream's exact match arms; the mixer keeps the
per-target split and takes upstream's macOS arm as is. Then verify:

```bash
cargo check --workspace --all-targets
cargo check --target wasm32-unknown-unknown -p wenilla --no-default-features --features webgpu
cargo test -p wenilla-realm
scripts/web-build.sh          # then boot it in a WebGPU browser: login, world entry, sound, cross a zone line
```

**The browser boot is not optional, and it is not a formality.** `std::time::Instant::now` and
`SystemTime::now` COMPILE on wasm32 and panic when called, so a sync that adds them passes
`cargo check --target wasm32` *and* the whole test suite and still dies at boot with
`RuntimeError: unreachable` and a blank canvas. Nothing but a browser catches it. Upstream does
not build wasm, so every sync tends to bring a fresh crop: the 2026-09-07 one brought seven, two
of them on per-frame systems that aborted the client in its first seconds. Before booting, this
sweep costs nothing and finds most of them:

```bash
# every file the sync added or touched that reaches a clock through std rather than the carry
git diff --stat main..HEAD --name-only -- '*.rs' | xargs grep -ln 'use std::time::.*Instant\|std::time::\(Instant\|SystemTime\)::now'
```

Upstream also moves code between files, and a moved `Instant` comes back as std's: the
2026-09-24 sync found the cast state carried out of `ui_cast.rs` (ours) into `spell/inflight.rs`
(std again). So also diff the hits against `origin/main` and read every *new* line, not only the
files the merge conflicted on.

`Duration` is fine — it is arithmetic, not a clock. `Instant` and `SystemTime` are not; they take
`bevy::platform::time::Instant` and `web_time::SystemTime`. Watch for the brace form
(`use std::time::{Duration, Instant}`), which a grep for the plain import misses.

Open the pull request against `Arnesen/wenilla` (`gh pr create -R Arnesen/wenilla …`) with three
sections: *what comes in*, *conflicts and how each was resolved*, *verification*. PR #12 is the
model, PR #21 the one that added the sweep above. `check.yml` runs the wasm check and the realm
tests on it — neither of which can see a wasm time panic, which is the whole point.

**Merge it with a merge commit, not squash.** A squash turns the merge back into a single-parent
content copy, and git forgets that upstream was merged (see below). The pin bot then moves prod's
pin to the merge; wenilla-realm/docs/RELEASE.md takes it from there.

## Where the conflicts will be

The list is generated, not remembered:

```bash
git diff --name-only $(git merge-base main upstream/main) main -- crates | grep -v '^crates/wenilla'
```

The recurring ones and the rule for each:

| file | our carry | resolution |
|---|---|---|
| `benilla-app/src/net/io.rs` | `dispatch()` extracted; native/wasm split around the spawn | keep the split, take upstream's arms |
| `benilla-app/src/net.rs` | `bevy::platform::time::Instant` (std's panics on wasm) | keep ours |
| `benilla-protocol/…/world/session.rs` | `recv_async().await` | keep ours, take upstream's new fields |
| `benilla-app/src/sound/mixer.rs`, `sound/mod.rs` | kira backend per target: upstream's `OutputBackend` natively, our `sound/web_backend.rs` (Web Audio over cpal 0.17) on wasm32 | keep the split; kira's own cpal backend is on cpal 0.18, which cannot share the lockfile (`alsa-sys`) |
| `benilla-app/src/sound/zone.rs`, `sound/web_load.rs` | soundscape loading off the frame on wasm: the music load, and the ambience bed's `OpeningBed` holding a `web_load::Pending` where native holds an IO-pool task | keep ours |
| `benilla-app/src/cvars/mod.rs`, `cvars/browser.rs` | the page's URL overrides (wasm-only), `local_state` reads | follow upstream's `REGISTERED` shape |
| `benilla-app/src/bindings.rs` | `BindingsState::synth_*`: the bridge presses a command by name through its `Bindings.xml` body, latched apart from the keys | keep ours |
| `benilla-app/src/lib.rs`, `benilla-app/Cargo.toml` | plugin registration, wasm-only deps | keep ours plus upstream's additions |
| `benilla-app/src/ui_script/lifecycle.rs`, `manifest.rs` | the entry load sliced across frames (`run_pending_entry_load`, `entry_prepare`/`entry_finish`/`entry_close`), stepping the core's `FrameXML.toc` rows through `manifest::load_core_missing`/`close_core` | keep the split; lift upstream's one-shot body into the three, and thread any new load input (`EntryInputs`) through the sliced path too |
| `benilla-app/src/local_state.rs`, `ui_saved.rs`, `ui_script/addons.rs`, `login/mod.rs` | state files through `local_state::read*`/`write_atomic*` (localStorage on the page) | keep ours; a new `std::fs` read of a state file takes the `local_state` twin |
| `benilla-protocol/src/{auth,lib}.rs`, `world/{mod,reader,session,writer/mod}.rs`, `tests/auth.rs` | the awaited readers over `transport::Conn` | keep ours; upstream's edits there have been comment-only so far |
| `AGENTS.md`, `README.md`, `.gitignore` | the fork's map and front page | keep ours; upstream's own map is `docs/METHOD.md` and `docs/MAP.md` |

## Android carries

The Android port (docs/ANDROID.md) lives in its own crate, `android/`, `scripts/android-*` and
`benilla-app/src/survivors/` (this repository's own code), and reaches upstream's files only at
the sites below. Each is a candidate conflict at the next sync; the rule for each is in the last
column. The logic sits in new files (`sound/mixer/suspend.rs`, `sound/output/suspend.rs`,
`gpu_blp/mip_drop.rs`, child modules so they reach private fields), so upstream files carry
only the lines listed. Two sites are not `#[cfg]`-gated, which the plan did not foresee (its
"Upstream touches" table allowed one `cfg(android)` arm each). The review accepted them
ungated: a gate would add more upstream lines than it removes, the browser takes the gate half
of the suspend, and both are inert off Android.

| file | change | gated? | resolution |
|---|---|---|---|
| `benilla-app/Cargo.toml` | the `arboard` target edge widened: `cfg(not(any(target_arch = "wasm32", target_os = "android")))` | cfg (one line) | keep ours |
| `benilla-app/src/textinput/clipboard.rs` | the three `cfg(not(target_arch = "wasm32"))` / `cfg(target_arch = "wasm32")` attributes on the `Pasteboard` impl and the `None =>` arms widened to `any(wasm32, android)`; Android takes the no-pasteboard arm | cfg (attributes only) | keep ours |
| `benilla-app/src/sound/mixer.rs` | one `mod suspend;` line; `Mixer::set_suspended(on)` itself is in `sound/mixer/suspend.rs` (shuts the output gate and, under `cfg(not(wasm32))`, calls `OutputBackend::set_suspended`), called from `survivors/lifecycle.rs` on `AppLifecycle::WillSuspend`/`WillResume` | no (a `mod` line) | keep ours; drop it if upstream grows a stop of its own |
| `benilla-app/src/sound/output/mod.rs` | one `mod suspend;` line, a `suspended: bool` on `OutputBackend` (declared and initialised `false`), one `!self.suspended &&` in `service`'s reopen condition; `OutputBackend::set_suspended` (closes a `Running` stream, resets the retry clock on resume) is in `sound/output/suspend.rs` | **no**: inert while `suspended` is false, which it always is off Android | keep ours; take upstream's `service` body around the one condition |
| `benilla-assets/src/gpu_blp.rs` | one `mod mip_drop;` line and the call in `for_upload`'s decode arm; `WOW_MIP_DROP` (`mip_drop()`, `levels_to_drop`, `drop_top_mips`, tests) is in `gpu_blp/mip_drop.rs`, where only `DEFAULT_MIP_DROP` is `cfg(target_os = "android")` (1 there, 0 elsewhere) | **partly**: the call is ungated, the default is the cfg | keep ours; a desktop run is unchanged at the default 0 |
| `benilla-app/src/glue/mod.rs` | `GlueClicks::tap(widget)`, four lines: `survivors/tap.rs` adds the touch-screen click (`Pressed → None` with the finger lifted inside the node) that `glue_clicks` never sees, since it reads a click as `Pressed → Hovered` | **no**: a method nothing upstream calls | keep ours |
| `.cargo/config.toml` | a `[target.aarch64-linux-android]` section with the 16 KB page-size link flag | new section, no cfg needed | keep ours |
| `Cargo.lock` | the `warcraft-survivors-android` package (`android_log-sys` for the logcat panic hook is already in the lock through `bevy_log`) | lockfile | regenerate |

The no-data panel's built-in font needs no manifest change: `default_font` comes with the
workspace's `default_platform` bevy feature. The generated list in "Where the conflicts will be"
picks all of these up; this section is the explanation.

## Why a real merge matters

Git resolves a merge against the last common ancestor. PR #7 was a real merge: `c3d8c1a0` has
upstream `6356bc8` as a parent. PR #12's sync commit `a9180db7` was a single-parent content copy,
so git's merge base stayed at `6356bc8`, and the next `git merge upstream/main` would have
replayed those 23 commits and re-raised every conflict #12 resolved by hand. PR #18 repaired it
with an empty merge, `git merge -s ours upstream/main`: the tree unchanged, upstream's `a8c9bc37`
recorded as a parent. That is also the tool if it ever happens again.

## When upstream rewrites its history

Upstream publishes squashed snapshots, and may one day force-push a new root. Then
`git merge upstream/main` reports unrelated histories. Do not pass `--allow-unrelated-histories`;
with an empty base every file conflicts. Graft the new history onto the commit we last merged,
merge, then drop the graft. The merge commit still records upstream's real sha as its parent.

```bash
old=$(git rev-parse <the upstream sha the last sync merged>)   # second parent of the last sync merge
root=$(git rev-list --max-parents=0 upstream/main | head -1)   # the new snapshot's root commit
git replace --graft "$root" "$old"      # locally: the new root now descends from what we merged
git merge upstream/main                 # a normal three-way merge against $old
git replace -d "$root"                  # the graft was only for the merge computation
```

## Is upstream's fix already in?

`git merge-base --is-ancestor` answers only for real merges. For a file, compare the trees:
`git diff upstream/main main -- path/to/file`. Empty means identical; a diff consisting only of
our `#[cfg]` blocks means current, plus our carry.
