//! The hero: its body, WASD movement over the terrain, the top-down camera, and its health.

use benilla_world::view::WorldCamera;
use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;

use super::data::{self, ClassDef};
use super::input::SurvivorsInput;
use super::units::{self, flat, wow_to_bevy, yaw_toward, Fx, Marks, UnitSpec};
use super::{Phase, Run, Selection};
use crate::creature_anim::{move_flags, MovementState};
use crate::net::{FieldChanged, Guid, ObjectStore};

#[derive(Component)]
pub(super) struct Hero {
    /// Vertical velocity in yards/second while airborne; 0 on the ground.
    pub(super) vy: f32,
    /// Height above the ground while airborne. Kept here, not in the transform: the creature
    /// ground clamp (`net::motion::ground_clamp_creatures`) re-seats every unit on the floor at
    /// the top of the frame, so the arc is re-applied over the ground each frame.
    pub(super) air: f32,
}

/// The body radius enemies stop at.
pub(super) const HERO_RADIUS: f32 = 0.7;

/// The hop's initial upward speed and the gravity that brings it back down — the reference
/// `CMovement` jump seed and gravity (`player/state.rs`'s `JUMP_SPEED` / `GRAVITY`).
const JUMP_SPEED: f32 = 7.955_547;
const GRAVITY: f32 = 19.291_105;

/// Radians of orbit per raw mouse unit while right-dragging.
const ORBIT_RATE: f32 = 0.008;
/// Elevation clamp (radians): ~10°..~83° above the horizon, so the camera never dives underground.
const PITCH_MIN: f32 = 0.18;
const PITCH_MAX: f32 = 1.45;
/// The locked view's tilt.
const FIXED_PITCH: f32 = 61.0 * std::f32::consts::PI / 180.0;
/// Wheel zoom range, yards from the hero's shoulders.
const ZOOM_MIN: f32 = 1.5;
const ZOOM_MAX: f32 = 70.0;

/// Whether the camera is locked; `C` toggles it.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum CamMode {
    /// Locked top-down (58°), laptop-friendly — the original feel.
    #[default]
    Fixed,
    /// Unlocked: a mouse drag swings and tilts it around the hero.
    Orbit,
}

#[derive(Resource)]
pub(super) struct CamRig {
    pub dist: f32,
    pub orbit: f32,
    /// Orbit azimuth (radians) around the hero; right-drag swings it during play.
    pub yaw: f32,
    /// Elevation above the horizon (radians), clamped so the camera never dives underground.
    pub pitch: f32,
    pub mode: CamMode,
}

impl Default for CamRig {
    fn default() -> Self {
        Self {
            dist: 30.0,
            orbit: 0.6,
            yaw: 0.0,
            pitch: FIXED_PITCH,
            mode: CamMode::Fixed,
        }
    }
}

/// Where a battleground's runs start.
pub(super) fn arena(map: &data::MapDef) -> Vec3 {
    wow_to_bevy(map.center)
}

/// The world is the battleground being stood in ([`Selection::stage`]): entering one on another
/// continent worldports, which raises the loading screen until the new ground is in.
pub(super) fn follow_map(
    selection: Res<Selection>,
    current: Option<ResMut<benilla_world::world_map::CurrentMap>>,
) {
    let Some(mut current) = current else {
        return;
    };
    let want = selection.stage_def().map;
    if current.0 != want {
        current.0 = want;
    }
}

/// The hero in the class trainer's look, weapons drawn.
pub(super) fn spawn_hero(
    commands: &mut Commands,
    class: &'static ClassDef,
    display: u32,
    race: u8,
    sex: u8,
    serial: u64,
    pos: Vec3,
) -> Entity {
    let bytes0 = u32::from(race)
        | u32::from(class.id) << 8
        | u32::from(sex) << 16
        | u32::from(class.power) << 24;
    let e = units::spawn_unit(
        commands,
        &UnitSpec {
            serial,
            display,
            scale: 1.0,
            level: 60,
            faction: units::FACTION_FRIENDLY,
            bytes0,
            weapons: class.weapons,
            sheath: class.sheath,
        },
        pos,
        0.0,
    );
    commands
        .entity(e)
        .insert((Hero { vy: 0.0, air: 0.0 }, Marks::buffs()));
    e
}

/// Midday over the battleground, whatever the clock says.
pub(super) const MINUTE_OF_DAY: u32 = 780;

pub(super) fn daylight(debug: Option<ResMut<benilla_world::dev_state::DebugState>>) {
    let Some(mut debug) = debug else { return };
    if debug.lighting.follow_server_time || debug.lighting.manual_minute != MINUTE_OF_DAY {
        debug.lighting.follow_server_time = false;
        debug.lighting.manual_minute = MINUTE_OF_DAY;
    }
}

/// The ground under the aim point ([`SurvivorsInput::aim`]: the mouse cursor, or the last touch
/// that aimed), `None` with the cursor off the window or no touch yet. The locked camera's hero
/// faces it, and the aimed mobility spells (Blink, Charge) go toward it.
#[derive(Resource, Default)]
pub(super) struct AimPoint(pub(super) Option<Vec3>);

/// Cast the aim point onto the terrain: the camera ray meets the plane at the hero's height, then
/// the terrain height there, a few times over, which settles on hills within a frame.
pub(super) fn aim_cursor(
    input: Res<SurvivorsInput>,
    cams: Query<(&Camera, &Transform), With<WorldCamera>>,
    hero: Query<&Transform, (With<Hero>, Without<WorldCamera>)>,
    world: WorldPoint,
    mut aim: ResMut<AimPoint>,
) {
    let point = (|| {
        let cursor = input.aim?;
        let (cam, tf) = cams.single().ok()?;
        let ray = cam
            .viewport_to_world(&GlobalTransform::from(*tf), cursor)
            .ok()?;
        let mut y = hero.single().ok()?.translation.y;
        let mut at = None;
        for _ in 0..4 {
            if ray.direction.y.abs() < 1e-4 {
                break;
            }
            let t = (y - ray.origin.y) / ray.direction.y;
            if t <= 0.0 {
                break;
            }
            let p = ray.origin + *ray.direction * t;
            at = Some(p);
            match world.terrain_height_under(p) {
                Some(h) if (h - y).abs() > 0.05 => y = h,
                _ => break,
            }
        }
        at
    })();
    if aim.0 != point {
        aim.0 = point;
    }
}

/// WASD (or the joystick) over the terrain within the battleground's leash, and the streaming
/// focus on the hero.
#[allow(clippy::too_many_arguments)]
pub(super) fn drive_hero(
    time: Res<Time>,
    input: Res<SurvivorsInput>,
    phase: Res<State<Phase>>,
    run: Res<Run>,
    selection: Res<Selection>,
    rig: Res<CamRig>,
    aim: Res<AimPoint>,
    world: WorldPoint,
    auto: Option<Res<super::auto::Auto>>,
    mut player: ResMut<crate::player::Player>,
    mut hero: Query<(&mut Transform, &mut MovementState, &mut Hero)>,
) {
    let focus = match hero.single_mut() {
        Ok((mut tf, mut mv, mut hm)) => {
            let dt = time.delta_secs();
            let mut dir = Vec3::ZERO;
            if let Some(auto) = auto
                .as_deref()
                .filter(|_| *phase.get() == Phase::Playing && run.dead_t.is_none())
            {
                dir = super::auto::steer(auto);
            } else if *phase.get() == Phase::Playing && run.dead_t.is_none() {
                // Screen up is world -z; the joystick's length (under 1) is a walking pace.
                dir = Vec3::new(input.mov.x, 0.0, input.mov.y);
                // In orbit mode WASD is camera-relative; in fixed mode it stays world-relative.
                if rig.mode == CamMode::Orbit {
                    let (sy, cy) = rig.yaw.sin_cos();
                    dir = Vec3::new(dir.x * cy + dir.z * sy, 0.0, dir.z * cy - dir.x * sy);
                }
            }
            let mut pos = tf.translation;
            // Locked, the hero faces the cursor and WASD moves him under it, so he backpedals and
            // strafes as a player does; unlocked (or driven), he faces where he runs.
            let face = (rig.mode == CamMode::Fixed
                && auto.is_none()
                && *phase.get() == Phase::Playing
                && run.dead_t.is_none())
            .then(|| aim.0.map(|a| flat(a - pos)))
            .flatten()
            .filter(|d| d.length() > 0.5)
            .map(|d| d.normalize());
            if let Some(look) = face {
                let target = Quat::from_rotation_y(yaw_toward(look));
                tf.rotation = tf.rotation.slerp(target, (dt * 20.0).min(1.0));
            }
            if dir != Vec3::ZERO && dt > 0.0 {
                let pace = dir.length().min(1.0);
                let dir = dir.normalize();
                let speed = run.stats().speed * run.haste_mult() * pace;
                pos += dir * speed * dt;
                mv.speed = speed;
                mv.flags = match face {
                    Some(look) => gait(look, dir),
                    None => {
                        let target = Quat::from_rotation_y(yaw_toward(dir));
                        tf.rotation = tf.rotation.slerp(target, (dt * 20.0).min(1.0));
                        move_flags::FORWARD
                    }
                };
            } else if mv.speed != 0.0 || mv.flags != 0 {
                mv.speed = 0.0;
                mv.flags = 0;
            }
            let map = selection.stage_def();
            let center = arena(map);
            let off = flat(pos - center);
            if off.length() > map.leash * 2.0 {
                // Just entered another battleground: start at its centre.
                pos = center;
            } else if off.length() > map.leash {
                let held = center + off.normalize() * map.leash;
                pos.x = held.x;
                pos.z = held.z;
            }
            let ground = world.terrain_height_under(pos).unwrap_or(pos.y);
            let playing = *phase.get() == Phase::Playing && run.dead_t.is_none();
            // Space (or the jump button) hops: lift off the terrain, then gravity brings the hero
            // back down.
            if playing && input.jump && hm.vy == 0.0 && hm.air == 0.0 {
                hm.vy = JUMP_SPEED;
            }
            if playing && (hm.vy != 0.0 || hm.air > 0.0) {
                hm.vy -= GRAVITY * dt;
                hm.air += hm.vy * dt;
                if hm.air <= 0.0 {
                    hm.air = 0.0;
                    hm.vy = 0.0;
                }
            } else {
                hm.air = 0.0;
                hm.vy = 0.0;
            }
            pos.y = ground + hm.air;
            // Feed the animation driver: `vertical_speed`'s sign and the `FALLING` flag are what
            // select the Jump (37/38) / Fall (40) animation (`creature_anim::select`).
            mv.vertical_speed = hm.vy;
            if hm.vy != 0.0 {
                mv.flags |= move_flags::FALLING;
            }
            if tf.translation != pos {
                tf.translation = pos;
            }
            pos
        }
        Err(_) => {
            let mut p = arena(selection.stage_def());
            if let Some(h) = world.terrain_height_under(p) {
                p.y = h;
            }
            p
        }
    };
    // The terrain streams and the loading screen waits on the ground under this.
    player.active = true;
    player.detached = false;
    player.settling = false;
    player.world_stale = false;
    player.pos = focus;
}

/// Let go of the mouse buttons when the page loses focus or the pointer leaves it. A release that
/// lands elsewhere (alt-tab mid-drag, a browser or OS popup) never reaches the game, so the button
/// would read as held and the unlocked camera would swing with every move until both buttons were
/// clicked again. Survivors only: the engine's mouse-look keeps its buttons across a deactivate,
/// as 1.12.1 does.
pub(super) fn drop_stray_buttons(
    mut focus: MessageReader<bevy::window::WindowFocused>,
    mut left: MessageReader<bevy::window::CursorLeft>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
) {
    let lost = focus.read().any(|f| !f.focused);
    if lost | (left.read().count() > 0) {
        mouse.release_all();
    }
}

/// A slow orbit at the menu; in play, top-down until `C` unlocks it to swing and tilt.
pub(super) fn place_camera(
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    mut rig: ResMut<CamRig>,
    input: Res<SurvivorsInput>,
    player: Res<crate::player::Player>,
    world: WorldPoint,
    hero: Query<&Transform, (With<Hero>, Without<WorldCamera>)>,
    mut cams: Query<&mut Transform, With<WorldCamera>>,
) {
    // `C` (or the pause panel's toggle) unlocks the camera to swing and tilt; locking it again
    // snaps back to the top-down view, so world-relative WASD lines up with the screen again.
    if input.camera_toggle {
        rig.mode = match rig.mode {
            CamMode::Fixed => CamMode::Orbit,
            CamMode::Orbit => {
                rig.yaw = 0.0;
                rig.pitch = FIXED_PITCH;
                CamMode::Fixed
            }
        };
    }
    // With no hero yet, the focus `drive_hero` grounded.
    let focus = hero.single().map_or(player.pos, |t| t.translation);
    // Multiplicative (the wheel's notches, or a pinch), so a step moves as far by eye close in as
    // far out; all the way down to an over-the-shoulder view.
    if input.zoom != 1.0 {
        rig.dist = (rig.dist * input.zoom).clamp(ZOOM_MIN, ZOOM_MAX);
    }
    let (eye, look) = if *phase.get() == Phase::Menu {
        rig.orbit += time.delta_secs() * 0.2;
        let d = 7.0;
        (
            focus + Vec3::new(rig.orbit.sin() * d, 2.4, rig.orbit.cos() * d),
            focus + Vec3::Y * 1.1,
        )
    } else {
        // Unlocked, either mouse button (or two fingers) drags the camera around the hero, as in
        // WoW.
        if rig.mode == CamMode::Orbit && input.orbit != Vec2::ZERO {
            rig.yaw -= input.orbit.x * ORBIT_RATE;
            rig.pitch = (rig.pitch + input.orbit.y * ORBIT_RATE).clamp(PITCH_MIN, PITCH_MAX);
        }
        let (sy, cy) = rig.yaw.sin_cos();
        let (sp, cp) = rig.pitch.sin_cos();
        let look = focus + Vec3::Y * 1.4;
        let mut eye = look + Vec3::new(cp * sy, sp, cp * cy) * rig.dist;
        // Never under the ground: a low tilt on a slope would bury the lens.
        if let Some(h) = world.terrain_height_under(eye) {
            eye.y = eye.y.max(h + 0.6);
        }
        (eye, look)
    };
    let placed = Transform::from_translation(eye).looking_at(look, Vec3::Y);
    for mut cam in &mut cams {
        *cam = placed;
    }
}

/// Damage the hero takes from one blow, after immunity, armour and the shield; returns what got
/// through.
pub(super) fn hurt_hero(run: &mut Run, raw: f32) -> f32 {
    if run.immune_t > 0.0 || run.dead_t.is_some() {
        return 0.0;
    }
    let mut dmg = (raw - run.stats().armor).max(1.0);
    if run.shield_t > 0.0 && run.shield > 0.0 {
        let absorbed = dmg.min(run.shield);
        run.shield -= absorbed;
        dmg -= absorbed;
    }
    run.hp -= dmg;
    dmg
}

/// The clock, regeneration, the timed buffs and their visuals, death and victory.
pub(super) fn hero_vitals(
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut next: ResMut<NextState<Phase>>,
    mut hero: Query<(Entity, &Guid, &mut ObjectStore, &mut Marks), With<Hero>>,
    mut fields: MessageWriter<FieldChanged>,
    mut fx: Fx,
) {
    let dt = time.delta_secs();
    let stats = run.stats();
    let Ok((entity, guid, mut store, mut marks)) = hero.single_mut() else {
        return;
    };
    if let Some(t) = run.dead_t.as_mut() {
        *t += dt;
        if *t > 3.0 {
            next.set(Phase::GameOver);
        }
        return;
    }
    run.elapsed += dt;
    let r = &mut *run;
    for t in [&mut r.shield_t, &mut r.immune_t, &mut r.thorns_t] {
        *t = (*t - dt).max(0.0);
    }
    let mut heal = stats.regen * dt;
    if run.heal_t > 0.0 {
        heal += run.heal_rate * dt.min(run.heal_t);
        run.heal_t = (run.heal_t - dt).max(0.0);
    }
    run.hp = (run.hp + heal).min(stats.max_hp);

    // The buffs show as their own state kits: the shield bubble, the immunity glow, the orbs.
    let mut shown: Vec<u32> = Vec::new();
    if run.shield_t > 0.0 && run.shield > 0.0 {
        shown.push(17);
    }
    if run.immune_t > 0.0 {
        shown.push(642);
    }
    if run.thorns_t > 0.0 {
        shown.push(run.thorns_spell);
    }
    if run.heal_t > 0.0 {
        shown.push(774);
    }
    for owned in &run.abilities {
        if owned.def.key == "lightning_shield" {
            shown.push(owned.def.spell);
        }
    }
    if run.passives.iter().any(|(p, _)| p.key == "renew") {
        shown.push(139);
    }
    marks.set(&shown);
    marks.flush(entity, guid.0, &mut store, &mut fields);

    if run.hp <= 0.0 {
        run.hp = 0.0;
        run.dead_t = Some(0.0);
        units::kill_unit(&mut store);
        marks.clear();
        marks.flush(entity, guid.0, &mut store, &mut fields);
    } else if run.elapsed >= data::VICTORY_SECS {
        run.won = true;
        // A last flourish before the results.
        fx.cast(entity, 642, &[entity], None);
        next.set(Phase::GameOver);
    }
}

/// The ground-plane distance from the hero, the measure every range in the mode uses.
/// The movement flags for running `dir` while facing `look`: forward, backpedal or strafe, by
/// which quarter the run falls in, so the walk, backpedal and strafe animations play.
fn gait(look: Vec3, dir: Vec3) -> u32 {
    let ahead = look.dot(dir);
    // Positive when `dir` is to the hero's left: WoW faces −Z with +X on the right.
    let left = look.cross(dir).y;
    let quarter = std::f32::consts::FRAC_1_SQRT_2;
    if ahead >= quarter {
        move_flags::FORWARD
    } else if ahead <= -quarter {
        move_flags::BACKWARD
    } else if left > 0.0 {
        move_flags::STRAFE_LEFT
    } else {
        move_flags::STRAFE_RIGHT
    }
}

pub(super) fn reach(hero: Vec3, other: Vec3) -> f32 {
    flat(other - hero).length()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::window::{CursorLeft, WindowFocused};

    fn held_left() -> World {
        let mut world = World::new();
        world.init_resource::<Messages<WindowFocused>>();
        world.init_resource::<Messages<CursorLeft>>();
        let mut mouse = ButtonInput::<MouseButton>::default();
        mouse.press(MouseButton::Left);
        world.insert_resource(mouse);
        world
    }

    fn left_held(world: &World) -> bool {
        world.resource::<ButtonInput<MouseButton>>().pressed(MouseButton::Left)
    }

    #[test]
    fn a_held_button_is_dropped_when_the_page_loses_focus() {
        let mut world = held_left();
        world.run_system_once(drop_stray_buttons).unwrap();
        assert!(left_held(&world), "nothing happened: the drag goes on");
        world.write_message(WindowFocused { window: Entity::PLACEHOLDER, focused: true });
        world.run_system_once(drop_stray_buttons).unwrap();
        assert!(left_held(&world), "gaining focus is no release");
        world.write_message(WindowFocused { window: Entity::PLACEHOLDER, focused: false });
        world.run_system_once(drop_stray_buttons).unwrap();
        assert!(!left_held(&world));
    }

    #[test]
    fn a_held_button_is_dropped_when_the_pointer_leaves_the_page() {
        let mut world = held_left();
        world.write_message(CursorLeft { window: Entity::PLACEHOLDER });
        world.run_system_once(drop_stray_buttons).unwrap();
        assert!(!left_held(&world));
    }
}
