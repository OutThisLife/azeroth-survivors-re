//! The app going to and coming back from the background ([`AppLifecycle`]): a phone's home
//! button, a call, the task switcher. Three things happen on the way out and undo on the way back:
//!
//! - A run in progress pauses, the same `Phase::Paused` the Esc key sets (`hud::keys`), so the
//!   hero is not killed while the screen is off.
//! - Every active camera is switched off until the first frame after the resume. Android
//!   destroys the window on pause and winit rebuilds the surface on `Resumed`; nothing may render
//!   to the old one in between, and a camera that is not active renders nowhere.
//! - The audio output is suspended ([`Mixer::set_suspended`]): the stream closes, so a
//!   backgrounded app plays nothing and holds no device.
//!
//! Desktop and the browser never send `WillSuspend`, so this is inert there.

use bevy::prelude::*;
use bevy::window::AppLifecycle;

use super::Phase;

/// What the suspend switched off, to put back on the resume.
#[derive(Resource, Default)]
struct Suspended {
    /// The cameras that were active at the suspend.
    cameras: Vec<Entity>,
    /// Set while suspended.
    down: bool,
    /// The resume arrived; the cameras come back on the frame after it.
    resume_in: Option<u8>,
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Suspended>()
        .add_systems(Update, lifecycle);
}

fn lifecycle(
    mut events: MessageReader<AppLifecycle>,
    mut state: ResMut<Suspended>,
    phase: Res<State<Phase>>,
    mut next: ResMut<NextState<Phase>>,
    mut cameras: Query<(Entity, &mut Camera)>,
    mut out: Option<NonSendMut<crate::sound::SoundOutput>>,
) {
    for event in events.read() {
        match event {
            AppLifecycle::WillSuspend | AppLifecycle::Suspended => {
                if state.down {
                    continue;
                }
                state.down = true;
                state.resume_in = None;
                if *phase.get() == Phase::Playing {
                    next.set(Phase::Paused);
                }
                state.cameras = cameras
                    .iter_mut()
                    .filter_map(|(entity, mut camera)| {
                        camera.is_active.then(|| {
                            camera.is_active = false;
                            entity
                        })
                    })
                    .collect();
                if let Some(mixer) = out.as_mut().and_then(|o| o.mixer.as_mut()) {
                    mixer.set_suspended(true);
                }
                info!("lifecycle: suspended ({} cameras off)", state.cameras.len());
            }
            AppLifecycle::WillResume | AppLifecycle::Running => {
                if state.down && state.resume_in.is_none() {
                    // Audio first: the stream reopens on the next service, the cameras one frame
                    // later, once the surface is back.
                    if let Some(mixer) = out.as_mut().and_then(|o| o.mixer.as_mut()) {
                        mixer.set_suspended(false);
                    }
                    state.resume_in = Some(1);
                }
            }
            AppLifecycle::Idle => {}
        }
    }
    if let Some(frames) = state.resume_in {
        if frames > 0 {
            state.resume_in = Some(frames - 1);
            return;
        }
        let Suspended {
            cameras: off,
            down,
            resume_in,
        } = &mut *state;
        for entity in off.drain(..) {
            if let Ok((_, mut camera)) = cameras.get_mut(entity) {
                camera.is_active = true;
            }
        }
        *down = false;
        *resume_in = None;
        info!("lifecycle: resumed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<Phase>()
            .add_message::<AppLifecycle>()
            .add_plugins(plugin);
        app
    }

    fn active(app: &App, e: Entity) -> bool {
        app.world().get::<Camera>(e).unwrap().is_active
    }

    #[test]
    fn suspend_pauses_a_run_and_parks_the_cameras_until_a_frame_after_resume() {
        let mut app = app();
        let on = app.world_mut().spawn(Camera::default()).id();
        let off = app
            .world_mut()
            .spawn(Camera {
                is_active: false,
                ..default()
            })
            .id();
        app.world_mut()
            .resource_mut::<NextState<Phase>>()
            .set(Phase::Playing);
        app.update();
        assert_eq!(
            *app.world().resource::<State<Phase>>().get(),
            Phase::Playing
        );

        app.world_mut().write_message(AppLifecycle::WillSuspend);
        app.update();
        assert!(!active(&app, on), "the active camera is off");
        assert!(!active(&app, off));
        // The pause lands on the next state transition.
        app.update();
        assert_eq!(*app.world().resource::<State<Phase>>().get(), Phase::Paused);

        app.world_mut().write_message(AppLifecycle::Suspended);
        app.update();
        app.world_mut().write_message(AppLifecycle::WillResume);
        app.update();
        assert!(
            !active(&app, on),
            "not yet: the surface is rebuilt this frame"
        );
        app.update();
        assert!(active(&app, on), "the frame after the resume");
        assert!(!active(&app, off), "a camera that was off stays off");
        // Still paused: the player resumes.
        assert_eq!(*app.world().resource::<State<Phase>>().get(), Phase::Paused);
    }

    #[test]
    fn a_menu_stays_a_menu() {
        let mut app = app();
        app.update();
        app.world_mut().write_message(AppLifecycle::WillSuspend);
        app.update();
        app.update();
        assert_eq!(*app.world().resource::<State<Phase>>().get(), Phase::Menu);
    }
}
