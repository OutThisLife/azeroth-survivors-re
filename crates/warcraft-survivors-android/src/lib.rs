//! The Android launcher: `warcraft-survivors/src/main.rs` as a `cdylib` that `GameActivity`
//! loads (`android/`), plus the env shim that makes the client's desktop assumptions hold on a
//! phone. `#[bevy_main]` emits the `android_main` entry on `target_os = "android"` and nothing
//! elsewhere, so this crate is a plain workspace member on the host too, where only the shim's
//! cfg-free half ([`env_shim`]) and its tests are live.
//!
//! The shim sets `BENILLA_HOME`, `WOW_DATA` and `WGPU_BACKEND` (and whatever
//! `<BENILLA_HOME>/env.toml` holds) before `run_survivors`, which is why nothing in `benilla-app`
//! needs an android arm for paths: `local_state::home()` and `install::wow_data()` read those
//! variables already. Edition 2021 and no app thread exists yet, so `set_var` is sound here.

use benilla_app::BuildId;

pub mod env_shim;

#[cfg(target_os = "android")]
mod android;

#[bevy::prelude::bevy_main]
fn main() {
    #[cfg(target_os = "android")]
    android::prepare();
    let _ = benilla_app::run_survivors(BuildId {
        version: env!("CARGO_PKG_VERSION"),
        describe: env!("BENILLA_GIT_DESCRIBE"),
        sha: env!("BENILLA_GIT_SHA"),
        short: env!("BENILLA_GIT_SHORT"),
        date: env!("BENILLA_GIT_DATE"),
        profile: env!("BENILLA_PROFILE"),
        project_dir: env!("BENILLA_PROJECT_DIR"),
        ..Default::default()
    });
}
