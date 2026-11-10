//! A frame-rate cap for thermals: `WOW_FPS_CAP=<hz>` (the android shim sets 30 through
//! `env.toml`) sleeps the main thread at the end of every frame until the next period. The
//! present mode stays `Fifo` (the default, `video::present_mode`), so with the cap off the
//! display's refresh is the limit as before; with it on, the sleep lands the next present on the
//! second vblank of a 60 Hz panel.
//!
//! The browser has no thread to sleep: the page's `requestAnimationFrame` paces the frame there,
//! so on wasm32 this registers nothing.

use bevy::prelude::*;

pub(super) fn plugin(app: &mut App) {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(cap) = native::from_env() {
        info!(
            "framecap: WOW_FPS_CAP={} — {:?} per frame",
            cap.hz, cap.period
        );
        app.insert_resource(cap).add_systems(Last, native::limit);
    }
    #[cfg(target_arch = "wasm32")]
    let _ = app;
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::time::Duration;

    use bevy::platform::time::Instant;
    use bevy::prelude::*;

    #[derive(Resource)]
    pub(super) struct FrameCap {
        pub hz: u32,
        pub period: Duration,
        /// When the frame in flight may end; `None` before the first.
        deadline: Option<Instant>,
    }

    pub(super) fn from_env() -> Option<FrameCap> {
        let hz: u32 = std::env::var("WOW_FPS_CAP").ok()?.trim().parse().ok()?;
        period_of(hz).map(|period| FrameCap {
            hz,
            period,
            deadline: None,
        })
    }

    /// The frame period `hz` means; `None` for 0 (off) and for anything past 1000 Hz, which a
    /// sleep cannot pace.
    pub(super) fn period_of(hz: u32) -> Option<Duration> {
        (1..=1000)
            .contains(&hz)
            .then(|| Duration::from_nanos(1_000_000_000 / u64::from(hz)))
    }

    /// Sleep out the rest of this frame's period. The deadline advances by whole periods from
    /// the last deadline, not from now, so a short frame does not drift the cadence, and a long
    /// frame (a load) resets it rather than storing up catch-up frames.
    pub(super) fn limit(mut cap: ResMut<FrameCap>) {
        let now = Instant::now();
        if let Some(deadline) = cap.deadline {
            if now < deadline {
                std::thread::sleep(deadline - now);
            }
        }
        let next = next_deadline(cap.deadline, Instant::now(), cap.period);
        cap.deadline = Some(next);
    }

    /// The deadline after `last`, as seen at `now`: one period on from `last` when the frame
    /// kept up, one period on from `now` when it fell behind.
    pub(super) fn next_deadline(last: Option<Instant>, now: Instant, period: Duration) -> Instant {
        match last {
            Some(last) if now < last + period => last + period,
            _ => now + period,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn periods() {
            assert_eq!(period_of(30), Some(Duration::from_nanos(33_333_333)));
            assert_eq!(period_of(60), Some(Duration::from_nanos(16_666_666)));
            assert_eq!(period_of(0), None);
            assert_eq!(period_of(5000), None);
        }

        #[test]
        fn the_cadence_holds_when_frames_keep_up_and_resets_when_they_fall_behind() {
            let period = Duration::from_millis(10);
            let t0 = Instant::now();
            // First frame: a period from now.
            assert_eq!(next_deadline(None, t0, period), t0 + period);
            // Kept up: the deadline advances from the last deadline, not from now.
            let d1 = t0 + period;
            assert_eq!(
                next_deadline(Some(d1), d1 + Duration::from_millis(1), period),
                d1 + period
            );
            // Fell behind: no catch-up, the next deadline is a period from now.
            let late = d1 + Duration::from_millis(25);
            assert_eq!(next_deadline(Some(d1), late, period), late + period);
        }
    }
}
