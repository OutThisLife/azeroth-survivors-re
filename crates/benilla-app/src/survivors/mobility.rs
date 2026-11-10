//! The class's mobility spell: learned at [`LEARN_LEVEL`], cast only on `F` (or its on-screen
//! button), never by itself. Each is the class's own signature escape: a mage blinks, a rogue
//! sprints, a warrior charges.

use bevy::prelude::*;

use super::enemies::Enemy;
use super::hero::{self, Hero};
use super::units::{flat, Fx};
use super::{Phase, Run};
use benilla_world::world_point::WorldPoint;

pub(super) const LEARN_LEVEL: u32 = 5;
pub(super) const KEY: KeyCode = KeyCode::KeyF;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Move {
    /// Jump this many yards along the way the hero faces.
    Blink(f32),
    /// Rush at the nearest enemy within this many yards, stunning it; else lunge that far.
    Charge(f32),
    /// Run faster by this factor for `secs`.
    Haste(f32),
    /// Haste, and enemies lose the hero (they stop chasing) for `secs`.
    Vanish(f32),
    /// Haste, and every enemy within this many yards flees in fear.
    Scatter(f32),
}

pub(super) struct Mobility {
    pub spell: u32,
    pub name: &'static str,
    pub kind: Move,
    pub cooldown: f32,
    pub secs: f32,
    pub desc: &'static str,
}

/// One per class, indexed by class id.
pub(super) fn for_class(class: u8) -> Option<&'static Mobility> {
    MOBILITY.iter().find(|(c, _)| *c == class).map(|(_, m)| m)
}

static MOBILITY: &[(u8, Mobility)] = &[
    (
        1,
        Mobility {
            spell: 100,
            name: "Charge",
            kind: Move::Charge(25.0),
            cooldown: 8.0,
            secs: 1.5,
            desc: "Charge the nearest enemy within 25 yards, stunning it.",
        },
    ),
    (
        2,
        Mobility {
            spell: 1044,
            name: "Blessing of Freedom",
            kind: Move::Haste(1.6),
            cooldown: 15.0,
            secs: 5.0,
            desc: "Nothing can hold you: run 60% faster for 5 sec.",
        },
    ),
    (
        3,
        Mobility {
            spell: 5118,
            name: "Aspect of the Cheetah",
            kind: Move::Haste(1.7),
            cooldown: 14.0,
            secs: 5.0,
            desc: "Run 70% faster for 5 sec.",
        },
    ),
    (
        4,
        Mobility {
            spell: 2983,
            name: "Sprint",
            kind: Move::Haste(1.8),
            cooldown: 15.0,
            secs: 4.0,
            desc: "Run 80% faster for 4 sec.",
        },
    ),
    (
        5,
        Mobility {
            spell: 586,
            name: "Fade",
            kind: Move::Vanish(1.4),
            cooldown: 18.0,
            secs: 4.0,
            desc: "Fade from the horde's sight: enemies lose you and you run 40% faster for 4 sec.",
        },
    ),
    (
        7,
        Mobility {
            spell: 2645,
            name: "Ghost Wolf",
            kind: Move::Haste(1.6),
            cooldown: 15.0,
            secs: 6.0,
            desc: "Take the form of a ghost wolf, running 60% faster for 6 sec.",
        },
    ),
    (
        8,
        Mobility {
            spell: 1953,
            name: "Blink",
            kind: Move::Blink(18.0),
            cooldown: 10.0,
            secs: 0.0,
            desc: "Teleport 18 yards forward.",
        },
    ),
    (
        9,
        Mobility {
            spell: 5484,
            name: "Howl of Terror",
            kind: Move::Scatter(12.0),
            cooldown: 20.0,
            secs: 4.0,
            desc: "Howl, sending every enemy within 12 yards fleeing in terror for 4 sec.",
        },
    ),
    (
        11,
        Mobility {
            spell: 1850,
            name: "Dash",
            kind: Move::Haste(1.8),
            cooldown: 15.0,
            secs: 4.0,
            desc: "Run 80% faster for 4 sec.",
        },
    ),
];

/// The mobility's run state, on [`Run`]: cooldown left, and the haste or vanish still running.
#[derive(Default, Clone, Copy)]
pub(super) struct MobilityState {
    pub cd: f32,
    pub haste: f32,
    pub haste_t: f32,
    pub vanish_t: f32,
}

impl Run {
    /// The class's mobility once the hero is level enough.
    pub(super) fn mobility(&self) -> Option<&'static Mobility> {
        (self.level >= LEARN_LEVEL)
            .then(|| for_class(self.class.id))
            .flatten()
    }

    /// The run speed's multiplier from a haste still running.
    pub(super) fn haste_mult(&self) -> f32 {
        if self.mobile.haste_t > 0.0 {
            self.mobile.haste
        } else {
            1.0
        }
    }
}

/// `F` (or the touch button): cast the mobility if it is learned and ready; ticks the cooldown and
/// the running effect.
#[allow(clippy::too_many_arguments)]
pub(super) fn cast_mobility(
    time: Res<Time>,
    input: Res<super::input::SurvivorsInput>,
    phase: Res<State<Phase>>,
    mut run: ResMut<Run>,
    world: WorldPoint,
    mut hero: Query<(Entity, &mut Transform), With<Hero>>,
    mut enemies: Query<(Entity, &Transform, &mut Enemy), Without<Hero>>,
    selection: Res<super::Selection>,
    aim: Res<hero::AimPoint>,
    mut fx: Fx,
) {
    let dt = time.delta_secs();
    let m = &mut run.mobile;
    m.cd = (m.cd - dt).max(0.0);
    m.haste_t = (m.haste_t - dt).max(0.0);
    m.vanish_t = (m.vanish_t - dt).max(0.0);
    if *phase.get() != Phase::Playing || run.dead_t.is_some() || !input.mobility {
        return;
    }
    let Some(def) = run.mobility() else { return };
    if run.mobile.cd > 0.0 {
        return;
    }
    let Ok((me, mut tf)) = hero.single_mut() else {
        return;
    };
    let pos = tf.translation;
    let facing = tf.rotation * Vec3::NEG_Z;
    let facing = flat(facing).normalize_or(Vec3::NEG_Z);
    // Aimed at the cursor where there is one, else straight ahead.
    let cursor = aim.0.filter(|a| flat(*a - pos).length() > 0.5);
    let toward = cursor.map_or(facing, |a| flat(a - pos).normalize());
    // Keep any jump inside the battleground's leash.
    let map = selection.map_def();
    let center = hero::arena(map);
    let leash = |p: Vec3| {
        let off = flat(p - center);
        if off.length() > map.leash {
            center + off.normalize() * map.leash
        } else {
            p
        }
    };
    let ground = |p: Vec3| world.terrain_height_under(p).unwrap_or(p.y);
    match def.kind {
        Move::Blink(yd) => {
            fx.cast(me, def.spell, &[me], None);
            // To the cursor, or as far as the spell reaches toward it.
            let far = cursor.map_or(yd, |a| flat(a - pos).length().min(yd));
            tf.rotation = Quat::from_rotation_y(super::units::yaw_toward(toward));
            let mut to = leash(pos + toward * far);
            to.y = ground(to);
            tf.translation = to;
        }
        Move::Charge(yd) => {
            // The enemy in reach nearest the cursor (or the hero, with none).
            let mark = cursor.unwrap_or(pos);
            let target = enemies
                .iter()
                .filter(|(_, t, e)| !e.dead && hero::reach(pos, t.translation) <= yd)
                .map(|(e, t, _)| (e, t.translation, hero::reach(mark, t.translation)))
                .min_by(|a, b| a.2.total_cmp(&b.2));
            let to = match target {
                Some((e, at, _)) => {
                    if let Ok((_, _, mut en)) = enemies.get_mut(e) {
                        en.stun_t = en.stun_t.max(def.secs);
                    }
                    let dir = flat(at - pos).normalize_or(facing);
                    tf.rotation = Quat::from_rotation_y(super::units::yaw_toward(dir));
                    fx.cast(me, def.spell, &[e], None);
                    at - dir * 1.5
                }
                None => {
                    fx.cast(me, def.spell, &[me], None);
                    tf.rotation = Quat::from_rotation_y(super::units::yaw_toward(toward));
                    pos + toward * yd * 0.5
                }
            };
            let mut to = leash(to);
            to.y = ground(to);
            tf.translation = to;
        }
        Move::Haste(x) => {
            fx.cast(me, def.spell, &[me], None);
            run.mobile.haste = x;
            run.mobile.haste_t = def.secs;
        }
        Move::Vanish(x) => {
            fx.cast(me, def.spell, &[me], None);
            run.mobile.haste = x;
            run.mobile.haste_t = def.secs;
            run.mobile.vanish_t = def.secs;
        }
        Move::Scatter(r) => {
            fx.cast(me, def.spell, &[me], None);
            for (_, t, mut en) in &mut enemies {
                if !en.dead && !en.boss && hero::reach(pos, t.translation) <= r {
                    en.fear_t = en.fear_t.max(def.secs);
                }
            }
        }
    }
    run.mobile.cd = def.cooldown;
}
