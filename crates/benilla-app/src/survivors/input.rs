//! One frame's intents, from the keyboard and mouse or from the touch screen, filled first in the
//! input stage so the gameplay systems read a single source whatever the device. The desktop
//! bindings (`CONTROLS.md`) are the reference; touch is a second way to produce the same intents:
//! a floating joystick under the first touch in the left part of the screen, a tap or drag on the
//! right that aims, a two-finger pinch that zooms, and on-screen buttons for the rest.
//!
//! On Android the touch layout is the control scheme from launch ([`touch_first`]): the HUD
//! starts in it and a key or button never switches it off, though every binding still works for
//! a Bluetooth keyboard. Elsewhere the first touch switches the layout on and a key switches it
//! back; `WOW_TOUCH=1` (`?touch=1` on the web) asks for touch-first there too.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::{AccumulatedMouseMotion, MouseScrollUnit, MouseWheel};
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::ui::UiGlobalTransform;
use bevy::window::PrimaryWindow;

/// The screen's left part, as a fraction of its width, where a touch starts the joystick.
pub(super) const STICK_ZONE: f32 = 0.45;
/// The joystick's full-deflection radius in logical pixels (dp on a phone).
pub(super) const STICK_RADIUS: f32 = 80.0;
/// The joystick's dead zone, as a fraction of its radius.
pub(super) const STICK_DEAD: f32 = 0.15;
/// A second press of Android's Back within this many seconds at the menu exits.
pub(super) const BACK_EXIT_SECS: f32 = 2.0;
/// How long a touch rests on an icon before its tooltip shows.
pub(super) const LONG_PRESS_SECS: f32 = 0.4;
/// How long the aim holds after the aiming finger lifts; then the hero faces where it runs.
pub(super) const AIM_HOLD_SECS: f32 = 1.5;

/// Whether the touch layout is the control scheme from the start and for good: on Android, or
/// wherever `WOW_TOUCH=1` says so (`?touch=1` on the web); `WOW_TOUCH=0` turns it off on Android.
pub(super) fn touch_first() -> bool {
    touch_first_from(
        cfg!(target_os = "android"),
        crate::webenv::var("WOW_TOUCH").as_deref(),
    )
}

/// The pure half of [`touch_first`]: the lever decides when set, the target otherwise. "0",
/// "off", "false" and an empty value turn it off; anything else turns it on.
pub(super) fn touch_first_from(android: bool, lever: Option<&str>) -> bool {
    match lever.map(|v| v.trim().to_ascii_lowercase()) {
        Some(v) => !matches!(v.as_str(), "" | "0" | "off" | "false" | "no"),
        None => android,
    }
}

/// The wording of a prompt for the layout: `key` names a key on the desktop layout, `touch`
/// says what to tap on the touch layout, so no key is ever asked of a player who has none.
pub(super) fn prompt<'a>(touch_active: bool, key: &'a str, touch: &'a str) -> &'a str {
    if touch_active {
        touch
    } else {
        key
    }
}

/// A text node whose wording follows the layout; [`prompts`] keeps it at [`prompt`]'s pick.
#[derive(Component)]
pub(super) struct Prompt {
    pub key: &'static str,
    pub touch: &'static str,
}

/// Rewrites every [`Prompt`] text for the current layout; a write only on a real change.
pub(super) fn prompts(input: Res<SurvivorsInput>, mut texts: Query<(&Prompt, &mut Text)>) {
    for (p, mut text) in &mut texts {
        let want = prompt(input.touch_active, p.key, p.touch);
        if text.0 != want {
            text.0 = want.to_string();
        }
    }
}

/// What the player asked for this frame.
#[derive(Resource)]
pub(super) struct SurvivorsInput {
    /// Screen-relative, -1..1 per axis: up is `-y` as `W` is; the joystick gives a magnitude.
    pub mov: Vec2,
    /// A window point (logical pixels) to raycast at: the cursor, or the last touch that aimed.
    pub aim: Option<Vec2>,
    pub jump: bool,
    pub mobility: bool,
    /// `Esc`, Android's Back, or the on-screen pause button.
    pub pause: bool,
    /// Android's Back alone: at the menu a second press exits.
    pub back: bool,
    /// `Enter`.
    pub confirm: bool,
    pub camera_toggle: bool,
    pub grade_toggle: bool,
    /// Multiplied into the camera distance: 1 is no change, under 1 zooms in.
    pub zoom: f32,
    /// A camera drag's delta in raw mouse units (or logical pixels for two fingers), while dragging.
    pub orbit: Vec2,
    /// Latched by the first touch; a key or mouse button press clears it, except under
    /// [`touch_first`], where it starts set and stays so. The HUD shows the on-screen controls
    /// while set.
    pub touch_active: bool,
    /// [`touch_first`], read once when the resource is made: the lever never changes mid-run.
    touch_first: bool,
    /// The joystick while a thumb holds it: its origin and the knob's point, logical pixels.
    pub joystick: Option<(Vec2, Vec2)>,
    /// Intents raised by UI systems later in the frame, taken by [`collect`] the next.
    pub queued: Queued,
}

impl Default for SurvivorsInput {
    fn default() -> Self {
        let first = touch_first();
        Self {
            mov: Vec2::ZERO,
            aim: None,
            jump: false,
            mobility: false,
            pause: false,
            back: false,
            confirm: false,
            camera_toggle: false,
            grade_toggle: false,
            zoom: 1.0,
            orbit: Vec2::ZERO,
            touch_active: first,
            touch_first: first,
            joystick: None,
            queued: Queued::default(),
        }
    }
}

#[derive(Default, Clone, Copy)]
pub(super) struct Queued {
    pub camera_toggle: bool,
    pub grade_toggle: bool,
}

/// An on-screen touch button: a touch starting on it fires the intent, and claims the touch so it
/// never moves or aims.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum TouchWidget {
    Jump,
    Mobility,
    Pause,
}

/// A node laid out only while [`SurvivorsInput::touch_active`].
#[derive(Component)]
pub(super) struct TouchOnly;

/// A node hidden while [`SurvivorsInput::touch_active`]: the key hints under the on-screen buttons.
#[derive(Component)]
pub(super) struct KeyHint;

/// What a touch does, decided when it lands and kept until it lifts.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Role {
    /// Landed on a UI node: bevy_ui's, never the world's.
    Claimed,
    /// The joystick, floating around where it landed.
    Stick {
        origin: Vec2,
    },
    Aim,
    /// One of the two fingers of a pinch.
    Pinch,
    /// A third finger, or a pinch's survivor: nothing.
    Idle,
}

#[derive(Default)]
pub(super) struct TouchState {
    roles: Vec<(u64, Role)>,
    /// The pinch's last distance and centroid.
    pinch_prev: Option<(f32, Vec2)>,
    /// Where the last aiming touch was; it holds [`AIM_HOLD_SECS`] after the finger lifts.
    last_aim: Option<Vec2>,
    /// Seconds since a finger last aimed.
    aim_age: f32,
}

/// Where the joystick's thumb puts the move vector: `offset` from the origin in logical pixels,
/// nothing inside the dead zone, full speed at the radius, and the slack between rescaled so the
/// speed ramps from zero at the dead zone's edge.
pub(super) fn joystick_vector(offset: Vec2, radius: f32, dead: f32) -> Vec2 {
    let len = offset.length();
    if len <= 0.0 || radius <= 0.0 {
        return Vec2::ZERO;
    }
    let t = ((len / radius - dead) / (1.0 - dead)).clamp(0.0, 1.0);
    if t == 0.0 {
        Vec2::ZERO
    } else {
        offset / len * t
    }
}

/// The knob's point for the visual: the offset clamped to the radius.
pub(super) fn joystick_knob(origin: Vec2, offset: Vec2, radius: f32) -> Vec2 {
    origin + offset.clamp_length_max(radius)
}

/// The camera-distance factor a pinch gives: the fingers spreading from `d0` to `d1` zooms in
/// by as much, and nothing moves when either distance is degenerate.
pub(super) fn pinch_zoom(d0: f32, d1: f32) -> f32 {
    if d0 <= 1e-3 || d1 <= 1e-3 {
        1.0
    } else {
        (d0 / d1).clamp(0.5, 2.0)
    }
}

/// The camera-distance factor one wheel notch gives, as the wheel zoom always has.
pub(super) fn wheel_zoom(notches: f32) -> f32 {
    (1.0 - 0.12 * notches).clamp(0.5, 1.5)
}

/// Whether a touch landing at `x` on a window `width` wide starts the joystick.
pub(super) fn in_stick_zone(x: f32, width: f32) -> bool {
    x < width * STICK_ZONE
}

/// Whether a touch at `physical` (physical pixels) lands on a UI node. A node under a hidden
/// panel (`Visibility::Hidden`) fails `InheritedVisibility`; a `Display::None` node (a touch
/// button on the desktop layout, a key hint on touch) keeps `InheritedVisibility`, so it is
/// told by its laid-out size, zero as bevy_ui's own picking backend reads it.
pub(super) fn node_hit(
    node: &ComputedNode,
    tf: &UiGlobalTransform,
    vis: &InheritedVisibility,
    physical: Vec2,
) -> bool {
    vis.get() && node.size() != Vec2::ZERO && node.contains_point(*tf, physical)
}

/// Fill [`SurvivorsInput`] for the frame.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn collect(
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut keyboard: MessageReader<KeyboardInput>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    mut wheel: MessageReader<MouseWheel>,
    touches: Res<Touches>,
    windows: Query<&Window, With<PrimaryWindow>>,
    nodes: Query<
        (
            &ComputedNode,
            &UiGlobalTransform,
            &InheritedVisibility,
            Option<&TouchWidget>,
        ),
        With<Interaction>,
    >,
    mut input: ResMut<SurvivorsInput>,
    mut touch: Local<TouchState>,
) {
    let queued = std::mem::take(&mut input.queued);
    input.mov = Vec2::ZERO;
    input.jump = false;
    input.mobility = false;
    input.pause = false;
    input.back = false;
    input.confirm = false;
    input.camera_toggle = queued.camera_toggle;
    input.grade_toggle = queued.grade_toggle;
    input.zoom = 1.0;
    input.orbit = Vec2::ZERO;
    input.joystick = None;

    // --- Keyboard and mouse, the bindings as they always were. ---
    if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        input.mov.y -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        input.mov.y += 1.0;
    }
    if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        input.mov.x -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        input.mov.x += 1.0;
    }
    input.jump |= keys.just_pressed(KeyCode::Space);
    input.mobility |= keys.just_pressed(super::mobility::KEY);
    input.pause |= keys.just_pressed(KeyCode::Escape);
    input.confirm |= keys.just_pressed(KeyCode::Enter);
    input.camera_toggle |= keys.just_pressed(KeyCode::KeyC);
    input.grade_toggle |= keys.just_pressed(KeyCode::KeyG);
    // Android's Back comes as a `BrowserBack` logical key, its physical code unidentified on some
    // devices and `BrowserBack` on others. The reader drains every frame, whichever matched: an
    // event left unread would be read again next frame, a second Back that exits at the menu.
    let mut logical_back = false;
    for k in keyboard.read() {
        logical_back |=
            k.state == ButtonState::Pressed && !k.repeat && k.logical_key == Key::BrowserBack;
    }
    if keys.just_pressed(KeyCode::BrowserBack) || logical_back {
        input.back = true;
        input.pause = true;
    }
    for w in wheel.read() {
        let notches = match w.unit {
            MouseScrollUnit::Line => w.y,
            MouseScrollUnit::Pixel => w.y * 0.02,
        };
        input.zoom *= wheel_zoom(notches);
    }
    if mouse.any_pressed([MouseButton::Left, MouseButton::Right]) {
        input.orbit = motion.delta;
    }
    // A real key or button switches the HUD back to the desktop layout; Back is not one, and
    // under touch-first nothing does: a Bluetooth keyboard works, silently.
    let keyed = keys
        .get_just_pressed()
        .any(|k| !matches!(k, KeyCode::Unidentified(_) | KeyCode::BrowserBack))
        || mouse.get_just_pressed().next().is_some();
    if keyed && input.touch_active && !input.touch_first {
        input.touch_active = false;
        touch.last_aim = None;
    }

    // --- Touch. ---
    let Ok(window) = windows.single() else {
        input.aim = None;
        return;
    };
    let width = window.width();
    let scale = window.scale_factor();
    // Touches that lifted (or were cancelled) leave; a touch that landed and lifted within the
    // frame still counts as landed below.
    touch
        .roles
        .retain(|(id, _)| touches.get_pressed(*id).is_some() || touches.just_pressed(*id));
    for t in touches.iter_just_pressed() {
        if touch.roles.iter().any(|(id, _)| *id == t.id()) {
            continue;
        }
        input.touch_active = true;
        let at = t.position();
        let physical = at * scale;
        // A touch on a UI node is the node's. The on-screen buttons fire here rather than through
        // `Interaction`, which follows the first touch only: a jump with a thumb on the stick.
        let hit = nodes
            .iter()
            .filter(|(node, tf, vis, _)| node_hit(node, tf, vis, physical))
            .max_by_key(|(node, ..)| node.stack_index());
        let role = if let Some((_, _, _, widget)) = hit {
            match widget {
                Some(TouchWidget::Jump) => input.jump = true,
                Some(TouchWidget::Mobility) => input.mobility = true,
                Some(TouchWidget::Pause) => input.pause = true,
                None => {}
            }
            Role::Claimed
        } else if !touch
            .roles
            .iter()
            .any(|(_, r)| matches!(r, Role::Stick { .. }))
            && in_stick_zone(at.x, width)
        {
            Role::Stick { origin: at }
        } else {
            let aiming = touch.roles.iter().filter(|(_, r)| *r == Role::Aim).count();
            let pinching = touch.roles.iter().any(|(_, r)| *r == Role::Pinch);
            if aiming == 0 && !pinching {
                touch.last_aim = Some(at);
                touch.aim_age = 0.0;
                Role::Aim
            } else if aiming == 1 && !pinching {
                // The second finger beside the aiming one: the pair pinches, from here.
                for (_, r) in touch.roles.iter_mut() {
                    if *r == Role::Aim {
                        *r = Role::Pinch;
                    }
                }
                touch.pinch_prev = None;
                Role::Pinch
            } else {
                Role::Idle
            }
        };
        touch.roles.push((t.id(), role));
    }
    // A pinch ends when either finger lifts; the other does nothing until it lifts too.
    if touch
        .roles
        .iter()
        .filter(|(_, r)| *r == Role::Pinch)
        .count()
        < 2
    {
        for (_, r) in touch.roles.iter_mut() {
            if *r == Role::Pinch {
                *r = Role::Idle;
            }
        }
        touch.pinch_prev = None;
    }
    let mut pinch = Vec::with_capacity(2);
    let mut aimed = None;
    for (id, role) in touch.roles.iter() {
        let Some(t) = touches.get_pressed(*id) else {
            continue;
        };
        match *role {
            Role::Stick { origin } => {
                let offset = t.position() - origin;
                input.mov = joystick_vector(offset, STICK_RADIUS, STICK_DEAD);
                input.joystick = Some((origin, joystick_knob(origin, offset, STICK_RADIUS)));
            }
            Role::Aim => aimed = Some(t.position()),
            Role::Pinch => pinch.push(t.position()),
            Role::Claimed | Role::Idle => {}
        }
    }
    if aimed.is_some() {
        touch.last_aim = aimed;
        touch.aim_age = 0.0;
    } else if touch.last_aim.is_some() {
        touch.aim_age += time.delta_secs();
        if touch.aim_age > AIM_HOLD_SECS {
            touch.last_aim = None;
        }
    }
    if let [a, b] = pinch[..] {
        let d = a.distance(b);
        let c = (a + b) / 2.0;
        if let Some((d0, c0)) = touch.pinch_prev {
            input.zoom *= pinch_zoom(d0, d);
            input.orbit += c - c0;
        }
        touch.pinch_prev = Some((d, c));
    }
    input.aim = if input.touch_active {
        touch.last_aim
    } else {
        window.cursor_position()
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joystick_dead_zone_and_clamp() {
        assert_eq!(joystick_vector(Vec2::ZERO, 80.0, 0.15), Vec2::ZERO);
        // Inside the dead zone: nothing.
        assert_eq!(
            joystick_vector(Vec2::new(10.0, 0.0), 80.0, 0.15),
            Vec2::ZERO
        );
        // At the radius and beyond: full speed along the offset.
        let full = joystick_vector(Vec2::new(0.0, 80.0), 80.0, 0.15);
        assert!((full - Vec2::new(0.0, 1.0)).length() < 1e-5);
        let far = joystick_vector(Vec2::new(-300.0, 0.0), 80.0, 0.15);
        assert!((far - Vec2::new(-1.0, 0.0)).length() < 1e-5);
        // Halfway between the dead zone's edge and the radius: half speed.
        let dead_px = 0.15 * 80.0;
        let mid = joystick_vector(Vec2::new((dead_px + 80.0) / 2.0, 0.0), 80.0, 0.15);
        assert!((mid.x - 0.5).abs() < 1e-5 && mid.y == 0.0);
        // Degenerate radius: nothing, no NaN.
        assert_eq!(joystick_vector(Vec2::new(5.0, 5.0), 0.0, 0.15), Vec2::ZERO);
    }

    #[test]
    fn joystick_ramp_is_monotonic_and_bounded() {
        let mut last = 0.0;
        for i in 0..200 {
            let v = joystick_vector(Vec2::new(i as f32, 0.0), 80.0, 0.15).length();
            assert!(v >= last && v <= 1.0, "i={i} v={v} last={last}");
            last = v;
        }
    }

    #[test]
    fn knob_stays_on_the_rim() {
        let o = Vec2::new(100.0, 200.0);
        assert_eq!(
            joystick_knob(o, Vec2::new(30.0, 0.0), 80.0),
            Vec2::new(130.0, 200.0)
        );
        let k = joystick_knob(o, Vec2::new(0.0, 500.0), 80.0);
        assert!(((k - o).length() - 80.0).abs() < 1e-4);
    }

    #[test]
    fn pinch_spreading_zooms_in() {
        assert!(pinch_zoom(100.0, 200.0) < 1.0);
        assert!(pinch_zoom(200.0, 100.0) > 1.0);
        assert_eq!(pinch_zoom(100.0, 100.0), 1.0);
        // A finger on top of the other, or no previous distance: no jump.
        assert_eq!(pinch_zoom(0.0, 100.0), 1.0);
        assert_eq!(pinch_zoom(100.0, 0.0), 1.0);
        // One frame's change is bounded.
        assert_eq!(pinch_zoom(1000.0, 1.0), 2.0);
        assert_eq!(pinch_zoom(1.0, 1000.0), 0.5);
    }

    #[test]
    fn wheel_matches_the_old_notch() {
        assert!((wheel_zoom(1.0) - 0.88).abs() < 1e-6);
        assert!((wheel_zoom(-1.0) - 1.12).abs() < 1e-6);
        assert_eq!(wheel_zoom(100.0), 0.5);
        assert_eq!(wheel_zoom(-100.0), 1.5);
    }

    #[test]
    fn touch_first_lever_overrides_the_target() {
        // No lever: the target decides.
        assert!(touch_first_from(true, None));
        assert!(!touch_first_from(false, None));
        // The lever on, anywhere.
        assert!(touch_first_from(false, Some("1")));
        assert!(touch_first_from(false, Some("true")));
        assert!(touch_first_from(false, Some(" yes ")));
        // The lever off, even on Android.
        for off in ["0", "off", "OFF", "false", "no", "", "  "] {
            assert!(!touch_first_from(true, Some(off)), "{off:?}");
        }
    }

    #[test]
    fn prompt_picks_the_layout_wording() {
        assert_eq!(
            prompt(false, "press Enter", "tap Play again"),
            "press Enter"
        );
        assert_eq!(
            prompt(true, "press Enter", "tap Play again"),
            "tap Play again"
        );
    }

    #[test]
    fn hidden_nodes_take_no_touch() {
        // A `Display::None` node lays out at zero size: never a hit, whatever its visibility,
        // even at its own centre, where `contains_point`'s rounded-rectangle test is nearest.
        let none = ComputedNode::default();
        let tf = UiGlobalTransform::from(bevy::math::Affine2::from_translation(Vec2::new(
            100.0, 100.0,
        )));
        assert!(!node_hit(
            &none,
            &tf,
            &InheritedVisibility::VISIBLE,
            Vec2::new(100.0, 100.0)
        ));
        assert!(!node_hit(
            &none,
            &tf,
            &InheritedVisibility::VISIBLE,
            Vec2::new(101.0, 100.0)
        ));
        // A laid-out node is hit inside and not outside, and never while its panel is hidden.
        let laid = ComputedNode {
            size: Vec2::new(48.0, 48.0),
            ..default()
        };
        assert!(node_hit(
            &laid,
            &tf,
            &InheritedVisibility::VISIBLE,
            Vec2::new(110.0, 90.0)
        ));
        assert!(!node_hit(
            &laid,
            &tf,
            &InheritedVisibility::VISIBLE,
            Vec2::new(130.0, 90.0)
        ));
        assert!(!node_hit(
            &laid,
            &tf,
            &InheritedVisibility::HIDDEN,
            Vec2::new(110.0, 90.0)
        ));
    }

    #[test]
    fn stick_zone_is_the_left_part() {
        assert!(in_stick_zone(0.0, 1000.0));
        assert!(in_stick_zone(449.0, 1000.0));
        assert!(!in_stick_zone(450.0, 1000.0));
        assert!(!in_stick_zone(999.0, 1000.0));
    }
}
