//! The backgrounded app's stop (`survivors/lifecycle.rs` on `AppLifecycle::WillSuspend` and
//! `WillResume`): a carry of this repository's, kept out of `mixer.rs` so an upstream sync meets
//! one `mod` line there (docs/UPSTREAM.md, "Android carries").

use super::Mixer;

impl Mixer {
    /// Suspend or resume the output: the gate shuts (a de-click, the stream closes right after)
    /// and the device stream closes until the resume, so nothing plays and no device is held.
    /// The browser has no stream of its own to close, so there the gate is the whole of it.
    pub(crate) fn set_suspended(&mut self, on: bool) {
        self.set_output_gate(!on);
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(event) = self.manager.backend_mut().set_suspended(on) {
            bevy::log::error!("audio: output is gone for good — {event:?}");
        }
    }
}
