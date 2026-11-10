//! The backgrounded app's stop, the backend half (`Mixer::set_suspended` calls it): a carry of
//! this repository's, kept out of `output/mod.rs` but for the `suspended` field and the one
//! condition in `service` that reads it (docs/UPSTREAM.md, "Android carries").

use std::time::Instant;

use super::{Event, OutputBackend, Stage, REOPEN_EVERY};

impl OutputBackend {
    /// Close the stream and keep it closed (a backgrounded app plays nothing and holds no device),
    /// or let the next `service` reopen it at once. Idempotent; a close that loses the ring
    /// consumer is a `Dead` event like any other.
    pub(in crate::sound) fn set_suspended(&mut self, on: bool) -> Option<Event> {
        if self.suspended == on {
            return None;
        }
        self.suspended = on;
        if on {
            if matches!(self.stage, Stage::Running { .. }) {
                if let Err(e) = self.close() {
                    self.stage = Stage::Dead;
                    return Some(Event::Dead(format!("{e:#}")));
                }
            }
        } else if let Stage::Idle { since, .. } = &mut self.stage {
            // Due now: the retry clock reads a full period elapsed.
            let now = Instant::now();
            *since = now.checked_sub(REOPEN_EVERY).unwrap_or(now);
        }
        None
    }
}
