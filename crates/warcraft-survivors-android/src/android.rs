//! The Android-only half of the shim: the app's directories, the environment, the panic hook.

use std::ffi::CString;
use std::fs;
use std::path::PathBuf;

use android_log_sys::LogPriority;

use crate::env_shim;

/// The logcat tag, for `adb logcat -s wsurvivors:*`.
const TAG: &str = "wsurvivors";

/// Everything that has to be true before `run_survivors`: the app's directories, the status
/// file and the panic hook, then the environment from `env.toml`. The hook comes after the
/// first two lines because it writes to the status file; a panic before it (the `expect`, the
/// `create_dir_all`) still reaches logcat through android-activity's stderr redirect
/// (`RustStdoutStderr`), and the file keeps the Java side's `launching`.
pub fn prepare() {
    let app = bevy::android::ANDROID_APP
        .get()
        .expect("android_main sets ANDROID_APP before main");
    // `internal_data_path` is `getFilesDir()`; it is `None` only when the activity is gone.
    let internal = app
        .internal_data_path()
        .unwrap_or_else(|| PathBuf::from("/data/local/tmp"));
    let external = app.external_data_path();

    let home = internal.join("benilla-config");
    if let Err(e) = fs::create_dir_all(&home) {
        log(
            LogPriority::WARN,
            &format!("cannot create {}: {e}", home.display()),
        );
    }
    // The stage file `MainActivity` shows while the engine comes up (`survivors/status.rs`
    // takes over once Bevy runs). First thing after `home` exists, so the hook below can
    // write `crash` into it too.
    let status = env_shim::status_file(&home);
    status_write(&status, "starting", "Starting the engine…");
    install_panic_hook(status);
    // First run: seed the preset, which the player edits from then on (`adb shell run-as`).
    let env_path = home.join("env.toml");
    if !env_path.exists() {
        if let Err(e) = fs::write(&env_path, env_shim::DEFAULT_ENV_TOML) {
            log(
                LogPriority::WARN,
                &format!("cannot seed {}: {e}", env_path.display()),
            );
        }
    }
    let env_toml = fs::read_to_string(&env_path).unwrap_or_default();
    let data = env_shim::data_dir(&internal, external.as_deref(), |p| p.is_dir());
    let vars = env_shim::variables(&home, &data, &env_toml, |k| std::env::var_os(k).is_some());
    for (key, value) in vars {
        log(LogPriority::INFO, &format!("env {key}={value}"));
        std::env::set_var(key, value);
    }
}

/// Chain a hook that writes the panic to logcat and to the status file (so the next launch
/// can say what happened), then runs the previous one (the default stderr print;
/// `crash::install` chains its file writer on top of this one later).
fn install_panic_hook(status: PathBuf) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log(LogPriority::FATAL, &format!("panic: {info}"));
        status_write(&status, "crash", &format!("The game crashed: {info}"));
        previous(info);
    }));
}

/// One stage into the status file; a failure is logged, never fatal (it is a progress line).
fn status_write(path: &std::path::Path, key: &str, text: &str) {
    if let Err(e) = env_shim::write_status(path, key, text) {
        log(
            LogPriority::WARN,
            &format!("cannot write {}: {e}", path.display()),
        );
    }
}

/// One line to logcat under [`TAG`]. An interior NUL would make `CString` fail; it is replaced
/// rather than dropping the line.
fn log(priority: LogPriority, text: &str) {
    let tag = CString::new(TAG).expect("static tag has no NUL");
    let text = CString::new(text.replace('\0', "?")).expect("NULs replaced");
    // SAFETY: both pointers are live NUL-terminated C strings for the duration of the call.
    unsafe {
        android_log_sys::__android_log_write(priority as i32, tag.as_ptr(), text.as_ptr());
    }
}
