//! Boot progress for a host with no console: when `WOW_STATUS_FILE=<path>` is set, the stage the
//! survivors boot has reached is written there, and whoever launched the process reads it. On
//! Android that is `MainActivity`'s overlay (`android/`), the only thing a player sees between
//! tapping the icon and the pick screen: the engine takes seconds to come up on a phone, and a
//! black window says nothing. Unset (desktop, web), this plugin adds nothing.
//!
//! The file is two lines, `<key>\n<text>\n`: a stable key for code and a sentence for people.
//! The launcher writes the stages before and after this plugin's reach in the same format
//! (`crates/warcraft-survivors-android/src/android.rs`: `starting` before Bevy, `crash` from the
//! panic hook), so one reader covers the whole run.
//!
//! One stage is written from [`plugin`] itself, while the `App` is still being built: on Android
//! the activity's UI thread waits for the native side to take the window, which happens only
//! once the event loop polls, after every plugin's build. Nothing on screen can change until
//! then, so a boot that stalls or dies in a plugin's build is told apart from one that never
//! started by what the file says the next time.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use bevy::prelude::*;

use super::menu::MenuBuilt;
use super::nodata::NoDataBuilt;

pub(super) fn plugin(app: &mut App) {
    let Some(path) = std::env::var_os("WOW_STATUS_FILE") else {
        return;
    };
    let path = PathBuf::from(path);
    if let Err(e) = write(&path, "plugins", "Building the engine (the window is not up yet)…") {
        warn!("survivors: status file {}: {e}", path.display());
    }
    app.insert_resource(StatusFile { path, last: None })
    // After the menu and no-data builders of the same frame, so a stage is reported the frame
    // it is reached rather than the next.
    .add_systems(Update, report.after(super::menu::build_menu));
}

#[derive(Resource)]
struct StatusFile {
    path: PathBuf,
    last: Option<Stage>,
}

/// Where the boot is, once the event loop runs. `Ready` and `NoData` are terminal for the
/// reader: the pick screen or the no-data panel is on screen, nothing more is coming. (The
/// archives open in `Startup`, before the first `Update`, so there is no "opening" stage to
/// see from here.)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Stage {
    /// The event loop runs; the pick screen's art is loading.
    Building,
    /// No `Data` folder: the no-data panel is up.
    NoData,
    /// The pick screen is built.
    Ready,
}

impl Stage {
    pub(super) fn key(self) -> &'static str {
        match self {
            Stage::Building => "building",
            Stage::NoData => "nodata",
            Stage::Ready => "ready",
        }
    }

    pub(super) fn text(self) -> &'static str {
        match self {
            Stage::Building => "Building the pick screen…",
            Stage::NoData => "No game data found.",
            Stage::Ready => "Ready.",
        }
    }
}

/// The stage from what exists this frame: a built menu wins, then the no-data panel.
pub(super) fn stage(menu_built: bool, no_data_panel: bool) -> Stage {
    if menu_built {
        Stage::Ready
    } else if no_data_panel {
        Stage::NoData
    } else {
        Stage::Building
    }
}

/// Write `<key>\n<text>\n` through a sibling temp file and a rename, so a reader polling the
/// path never sees a half-written one.
pub(super) fn write(path: &Path, key: &str, text: &str) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, format!("{key}\n{text}\n"))?;
    fs::rename(&tmp, path)
}

fn report(
    mut file: ResMut<StatusFile>,
    built: Query<(), With<MenuBuilt>>,
    panel: Query<(), With<NoDataBuilt>>,
) {
    let now = stage(!built.is_empty(), !panel.is_empty());
    if file.last == Some(now) {
        return;
    }
    file.last = Some(now);
    if let Err(e) = write(&file.path, now.key(), now.text()) {
        warn!("survivors: status file {}: {e}", file.path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_built_menu_wins_and_nothing_means_building() {
        assert_eq!(stage(true, true), Stage::Ready);
        assert_eq!(stage(false, true), Stage::NoData);
        assert_eq!(stage(false, false), Stage::Building);
    }

    #[test]
    fn the_file_is_key_then_text_and_lands_whole() {
        let dir = std::env::temp_dir().join(format!("survivors-status-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("status.txt");
        write(&path, Stage::Building.key(), Stage::Building.text()).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "building\nBuilding the pick screen…\n"
        );
        assert!(!path.with_extension("tmp").exists(), "the temp file is renamed away");
        write(&path, "ready", "Ready.").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "ready\nReady.\n");
        fs::remove_dir_all(&dir).unwrap();
    }
}
