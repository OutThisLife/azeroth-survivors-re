//! The env shim's decisions, with no Android in them so they unit-test on the host: which
//! variables to set from the app's two data directories and its `env.toml`.
//!
//! `env.toml` is the player's override file at `<BENILLA_HOME>/env.toml`, one `KEY = "value"`
//! per line (`WOW_DATA`, `WOW_NO_BC`, any lever the client reads through `std::env::var`). It
//! is parsed here by hand: the format is a flat key/value list, and the `toml` crate would be
//! a dependency for three rules.

use std::path::{Path, PathBuf};

/// The preset [`crate`]'s android half writes to `env.toml` when the file does not exist yet:
/// the P2 starting point (docs/ANDROID.md), edited in the repository, not here.
pub const DEFAULT_ENV_TOML: &str = include_str!("../../../scripts/android-env.default.toml");

/// Parse `KEY = "value"` lines. `#` comments and blank lines are skipped, quotes around the
/// value are optional, a line without `=` or with an empty key is ignored. Later lines win over
/// earlier ones for the same key.
pub fn parse_env_toml(text: &str) -> Vec<(String, String)> {
    let mut vars: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            continue;
        }
        let value = unquote(value.trim());
        match vars.iter_mut().find(|(k, _)| k == key) {
            Some(slot) => slot.1 = value,
            None => vars.push((key.to_string(), value)),
        }
    }
    vars
}

/// Strip one pair of matching `"` or `'` quotes; inside double quotes `\"`, `\\`, `\n` and `\t`
/// unescape.
fn unquote(value: &str) -> String {
    let bytes = value.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"' {
        let inner = &value[1..value.len() - 1];
        let mut out = String::with_capacity(inner.len());
        let mut chars = inner.chars();
        while let Some(c) = chars.next() {
            match c {
                '\\' => match chars.next() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some(other) => out.push(other),
                    None => out.push('\\'),
                },
                c => out.push(c),
            }
        }
        out
    } else if bytes.len() >= 2 && bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\'' {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

/// Where the `Data` folder is looked for: `<external>/Data` when it exists (that is where
/// `adb push` and the on-phone folder picker put it), else `<internal>/Data`.
pub fn data_dir(
    internal: &Path,
    external: Option<&Path>,
    exists: impl Fn(&Path) -> bool,
) -> PathBuf {
    if let Some(external) = external {
        let candidate = external.join("Data");
        if exists(&candidate) {
            return candidate;
        }
    }
    internal.join("Data")
}

/// The boot-stage file `MainActivity` polls (`WOW_STATUS_FILE`; `survivors/status.rs` has the
/// format). Under `home`, next to `env.toml`, so the Java side finds it from `getFilesDir()`.
pub fn status_file(home: &Path) -> PathBuf {
    home.join("status.txt")
}

/// Write one stage, `<key>\n<text>\n`, through a temp file and a rename so the polling reader
/// never sees half a line. The same shape `survivors/status.rs` writes from inside Bevy.
pub fn write_status(path: &Path, key: &str, text: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, format!("{key}\n{text}\n"))?;
    std::fs::rename(&tmp, path)
}

/// The variables to set before the app boots, in order: `env.toml`'s own first, then the
/// defaults it did not name. `BENILLA_HOME` and `WOW_STATUS_FILE` are always under `home`
/// (the file lives in it, so it cannot move itself, and the Java side reads the status from
/// there); `WOW_DATA` and `WGPU_BACKEND` keep an `env.toml` value, and `WGPU_BACKEND` also
/// keeps one the process already carries (`already_set`).
pub fn variables(
    home: &Path,
    data: &Path,
    env_toml: &str,
    already_set: impl Fn(&str) -> bool,
) -> Vec<(String, String)> {
    let mut vars = parse_env_toml(env_toml);
    vars.retain(|(k, _)| k != "BENILLA_HOME" && k != "WOW_STATUS_FILE");
    let has = |vars: &[(String, String)], key: &str| vars.iter().any(|(k, _)| k == key);
    vars.push((
        "BENILLA_HOME".to_string(),
        home.to_string_lossy().into_owned(),
    ));
    vars.push((
        "WOW_STATUS_FILE".to_string(),
        status_file(home).to_string_lossy().into_owned(),
    ));
    if !has(&vars, "WOW_DATA") {
        vars.push(("WOW_DATA".to_string(), data.to_string_lossy().into_owned()));
    }
    if !has(&vars, "WGPU_BACKEND") && !already_set("WGPU_BACKEND") {
        vars.push(("WGPU_BACKEND".to_string(), "vulkan".to_string()));
    }
    vars
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get<'a>(vars: &'a [(String, String)], key: &str) -> Option<&'a str> {
        vars.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    #[test]
    fn parses_quoted_bare_and_commented_lines() {
        let text = "# the data folder\nWOW_DATA = \"/sdcard/wow/Data\"\nWOW_NO_BC=1\n\n  WOW_X = 'a b'  \nnot a line\n= \"no key\"\nBAD KEY = 1\n";
        let vars = parse_env_toml(text);
        assert_eq!(get(&vars, "WOW_DATA"), Some("/sdcard/wow/Data"));
        assert_eq!(get(&vars, "WOW_NO_BC"), Some("1"));
        assert_eq!(get(&vars, "WOW_X"), Some("a b"));
        assert_eq!(vars.len(), 3);
    }

    #[test]
    fn last_line_wins_and_escapes_unquote() {
        let vars = parse_env_toml("K = \"one\"\nK = \"two \\\"quoted\\\" \\\\ back\"\n");
        assert_eq!(get(&vars, "K"), Some("two \"quoted\" \\ back"));
        assert_eq!(vars.len(), 1);
    }

    #[test]
    fn data_prefers_external_when_present() {
        let internal = Path::new("/data/user/0/pkg/files");
        let external = Path::new("/storage/emulated/0/Android/data/pkg/files");
        let ext = data_dir(internal, Some(external), |p| p == external.join("Data"));
        assert_eq!(ext, external.join("Data"));
        let int = data_dir(internal, Some(external), |_| false);
        assert_eq!(int, internal.join("Data"));
        let none = data_dir(internal, None, |_| true);
        assert_eq!(none, internal.join("Data"));
    }

    #[test]
    fn defaults_fill_what_env_toml_left_out() {
        let home = Path::new("/files/benilla-config");
        let data = Path::new("/files/Data");
        let vars = variables(home, data, "", |_| false);
        assert_eq!(get(&vars, "BENILLA_HOME"), Some("/files/benilla-config"));
        assert_eq!(get(&vars, "WOW_DATA"), Some("/files/Data"));
        assert_eq!(get(&vars, "WGPU_BACKEND"), Some("vulkan"));
        assert_eq!(
            get(&vars, "WOW_STATUS_FILE"),
            Some("/files/benilla-config/status.txt")
        );
        assert_eq!(vars.len(), 4);
    }

    #[test]
    fn env_toml_overrides_data_and_backend_but_not_home() {
        let home = Path::new("/files/benilla-config");
        let data = Path::new("/files/Data");
        let text = "WOW_DATA = \"/sdcard/wow/Data\"\nWGPU_BACKEND = \"gl\"\nBENILLA_HOME = \"/elsewhere\"\nWOW_STATUS_FILE = \"/elsewhere/s\"\nWOW_NO_BC = \"1\"\n";
        let vars = variables(home, data, text, |_| false);
        assert_eq!(get(&vars, "WOW_DATA"), Some("/sdcard/wow/Data"));
        assert_eq!(get(&vars, "WGPU_BACKEND"), Some("gl"));
        assert_eq!(get(&vars, "BENILLA_HOME"), Some("/files/benilla-config"));
        assert_eq!(
            get(&vars, "WOW_STATUS_FILE"),
            Some("/files/benilla-config/status.txt")
        );
        assert_eq!(get(&vars, "WOW_NO_BC"), Some("1"));
        assert_eq!(vars.len(), 5);
    }

    #[test]
    fn a_status_write_is_whole_and_leaves_no_temp() {
        let dir = std::env::temp_dir().join(format!("wsurvivors-status-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = status_file(&dir);
        write_status(&path, "starting", "Starting the engine…").unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "starting\nStarting the engine…\n"
        );
        assert!(!path.with_extension("tmp").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_seeded_preset_parses() {
        let vars = parse_env_toml(DEFAULT_ENV_TOML);
        assert_eq!(get(&vars, "WOW_FPS_CAP"), Some("30"));
        assert_eq!(get(&vars, "WOW_MIP_DROP"), Some("1"));
        assert!(
            vars.iter().all(|(k, _)| k.starts_with("WOW_")),
            "only client levers: {vars:?}"
        );
        // The preset never pins the data folder or the backend; the shim's defaults fill them.
        assert_eq!(get(&vars, "WOW_DATA"), None);
    }

    #[test]
    fn a_backend_already_in_the_process_is_kept() {
        let vars = variables(Path::new("/h"), Path::new("/d"), "", |k| {
            k == "WGPU_BACKEND"
        });
        assert_eq!(get(&vars, "WGPU_BACKEND"), None);
    }
}
