//! The level-up picker, as a survivors game deals it: three tall cards that flip up out of the
//! deck one after another, tilt toward the cursor, lift under it and burst on the pick. Each card
//! wears its rarity in WoW's item-quality colours, so what is better reads at a glance: a fresh
//! spell is Rare, a rank up Uncommon, a rank that adds a target or reaches the top Epic.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use super::data::{self, Kind};
use super::fill::FillMaterial;
use super::hud::{self, Choice, HudFont, Offer, SpellIcon};
use super::{Phase, Rng, Run};

/// WoW's item-quality colours (`ITEM_QUALITY_COLORS`).
pub(super) const COMMON: Color = Color::srgb(1.0, 1.0, 1.0);
pub(super) const UNCOMMON: Color = Color::srgb(0.118, 1.0, 0.0);
pub(super) const RARE: Color = Color::srgb(0.0, 0.439, 0.867);
pub(super) const EPIC: Color = Color::srgb(0.639, 0.208, 0.933);
pub(super) const LEGENDARY: Color = Color::srgb(1.0, 0.502, 0.0);

const CARD_W: f32 = 236.0;
const CARD_H: f32 = 330.0;
/// Seconds between one card landing and the next.
const DEAL_STAGGER: f32 = 0.09;
const DEAL_SECS: f32 = 0.38;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

impl Rarity {
    pub(super) fn color(self) -> Color {
        match self {
            Self::Common => COMMON,
            Self::Uncommon => UNCOMMON,
            Self::Rare => RARE,
            Self::Epic => EPIC,
            Self::Legendary => LEGENDARY,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Common => "Common",
            Self::Uncommon => "Uncommon",
            Self::Rare => "Rare",
            Self::Epic => "Epic",
            Self::Legendary => "Legendary",
        }
    }
}

/// A spell's rough damage per second at rank 1 against a crowd: per-cast damage times the
/// enemies it can reach, over its cooldown. Only for ranking a class's own spells against each
/// other; the defensive kinds have none and sit in the middle.
fn power(def: &data::AbilityDef) -> Option<f32> {
    let area = |r: f32| 1.0 + r * 0.6;
    let reach = match def.kind {
        Kind::Bolt | Kind::Strike => def.count as f32 + def.radius * 0.4,
        Kind::Melee | Kind::Orbit => def.count as f32,
        Kind::Nova | Kind::Cone | Kind::Ground | Kind::GroundSelf | Kind::Aura => area(def.radius),
        Kind::Chain => def.count as f32 + 1.0,
        Kind::Dot => def.count as f32,
        _ => return None,
    };
    let per = if def.damage > 0.0 {
        def.damage
    } else {
        def.dot * def.duration.max(1.0)
    };
    let ticks = if matches!(def.kind, Kind::Ground | Kind::GroundSelf) && def.duration > 0.0 {
        def.duration
    } else {
        1.0
    };
    Some(per * ticks * reach / def.cooldown.max(0.2))
}

/// Where a new spell stands among its class's: the strongest third Epic, the weakest third
/// Uncommon, the rest (and the defensive spells) Rare.
fn spell_tier(def: &data::AbilityDef, run: &Run) -> Rarity {
    let Some(mine) = power(def) else {
        return Rarity::Rare;
    };
    let mut all: Vec<f32> = run
        .class
        .pool
        .iter()
        .filter_map(|k| power(data::ability(k)))
        .collect();
    all.sort_by(f32::total_cmp);
    let rank = all.iter().filter(|&&p| p < mine).count() as f32 / all.len().max(1) as f32;
    if rank >= 0.66 {
        Rarity::Epic
    } else if rank < 0.33 {
        Rarity::Uncommon
    } else {
        Rarity::Rare
    }
}

/// What a choice is worth, by what it brings: a top rank is Legendary, a rank that adds a target
/// Epic, a new spell by its power among the class's ([`spell_tier`]), a plain rank Uncommon.
pub(super) fn rarity(choice: Choice, run: &Run) -> Rarity {
    match choice {
        Choice::NewAbility(def) => spell_tier(def, run),
        Choice::Upgrade(k) => {
            let o = &run.abilities[k];
            let next = o.level + 1;
            let targets = [3, 5, 7].contains(&next)
                && matches!(
                    o.def.kind,
                    Kind::Bolt | Kind::Strike | Kind::Melee | Kind::Dot | Kind::Chain | Kind::Orbit
                );
            if next == data::MAX_ABILITY_LEVEL {
                Rarity::Legendary
            } else if targets {
                Rarity::Epic
            } else {
                Rarity::Uncommon
            }
        }
        Choice::NewPassive(_) => Rarity::Uncommon,
        Choice::UpPassive(k) => match run.passives[k].1 + 1 {
            n if n == data::MAX_PASSIVE_LEVEL => Rarity::Epic,
            n if n + 1 == data::MAX_PASSIVE_LEVEL => Rarity::Rare,
            2 => Rarity::Common,
            _ => Rarity::Uncommon,
        },
        Choice::Heal => Rarity::Common,
    }
}

/// One card's motion: dealt in, tilted to the cursor, lifted under it, burst on the pick.
#[derive(Component)]
pub(super) struct Card {
    index: usize,
    /// Seconds since this card's deal began; negative while it waits its turn.
    t: f32,
    hover: f32,
    tilt: Vec2,
    picked: Option<f32>,
    rarity: Rarity,
}

/// The shine that sweeps across a card as it lands.
#[derive(Component)]
pub(super) struct CardShine(usize);

/// The radiant burst behind the row, turning slowly.
#[derive(Component)]
pub(super) struct Rays;

/// The pending pick: the card bursts, then the choice applies.
#[derive(Resource, Default)]
pub(super) struct PendingPick(Option<(usize, f32)>);

const PICK_SECS: f32 = 0.28;

/// The cards, rebuilt for each offer.
pub(super) fn deal_cards(
    mut commands: Commands,
    offer: Res<Offer>,
    run: Res<Run>,
    spells: Option<Res<crate::ui_action::Spells>>,
    font: Res<HudFont>,
    rows: Query<Entity, With<hud::OfferRow>>,
    mut fills: ResMut<Assets<FillMaterial>>,
) {
    if !offer.is_changed() {
        return;
    }
    let Ok(row) = rows.single() else { return };
    commands.entity(row).despawn_related::<Children>();
    let f = &font.0;
    commands.entity(row).with_children(|row| {
        for (i, &choice) in offer.choices().iter().enumerate() {
            let card = hud::describe_card(choice, &run, spells.as_deref());
            let rarity = rarity(choice, &run);
            let rc = rarity.color();
            row.spawn((
                Card {
                    index: i,
                    t: -(i as f32) * DEAL_STAGGER,
                    hover: 0.0,
                    tilt: Vec2::ZERO,
                    picked: None,
                    rarity,
                },
                Button,
                Interaction::default(),
                RelativeCursorPosition::default(),
                Node {
                    width: Val::Px(CARD_W),
                    height: Val::Px(CARD_H),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::new(
                        Val::Px(16.0),
                        Val::Px(16.0),
                        Val::Px(18.0),
                        Val::Px(14.0),
                    ),
                    row_gap: Val::Px(6.0),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(10.0)),
                    ..default()
                },
                BorderColor::all(rc),
                BoxShadow::new(
                    rc.with_alpha(0.0),
                    Val::Px(0.0),
                    Val::Px(0.0),
                    Val::Px(0.0),
                    Val::Px(26.0),
                ),
                UiTransform {
                    translation: Val2::px(0.0, 260.0),
                    scale: Vec2::splat(0.6),
                    rotation: Rot2::degrees(-14.0),
                },
                Visibility::Hidden,
            ))
            .with_children(|c| {
                // A dark face lit from the top in the rarity's colour, inside the border.
                let inner_h = CARD_H - 4.0;
                c.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        right: Val::Px(0.0),
                        top: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        ..default()
                    },
                    MaterialNode(
                        fills.add(
                            FillMaterial::vertical(
                                rc.mix(&Color::BLACK, 0.55).with_alpha(0.98),
                                Color::srgba(0.05, 0.04, 0.035, 0.97),
                                0.42,
                                Color::srgba(0.02, 0.015, 0.01, 0.98),
                                8.0 / inner_h,
                            )
                            .with_glow(
                                rc.with_alpha(0.45),
                                Vec2::new(0.5, 60.0 / inner_h),
                                Vec2::new((CARD_W - 4.0) / 2.0 / inner_h, 90.0 / inner_h),
                            ),
                        ),
                    ),
                    bevy::ui::FocusPolicy::Pass,
                ));
                // The key and the rarity on one line along the top.
                c.spawn(Node {
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::SpaceBetween,
                    ..default()
                })
                .with_children(|top| {
                    top.spawn(hud::text_bundle(
                        f,
                        format!("{}", i + 1),
                        15.0,
                        hud::DIM_GREY,
                    ));
                    top.spawn(hud::text_bundle(f, rarity.label(), 13.0, rc));
                });
                // The icon in a gold-rimmed socket.
                c.spawn((
                    Node {
                        width: Val::Px(74.0),
                        height: Val::Px(74.0),
                        margin: UiRect::vertical(Val::Px(6.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(6.0)),
                        ..default()
                    },
                    BorderColor::all(rc.mix(&hud::GOLD_TONE, 0.4)),
                    BoxShadow::new(
                        rc.with_alpha(0.55),
                        Val::Px(0.0),
                        Val::Px(0.0),
                        Val::Px(2.0),
                        Val::Px(14.0),
                    ),
                ))
                .with_children(|s| {
                    s.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                            ..default()
                        },
                        ImageNode {
                            color: Color::srgba(0.0, 0.0, 0.0, 0.6),
                            ..default()
                        },
                        SpellIcon(card.spell),
                    ));
                });
                c.spawn(hud::centered_bundle(
                    f,
                    card.name,
                    21.0,
                    rc.mix(&Color::WHITE, 0.25),
                ));
                c.spawn(hud::centered_bundle(f, card.tag, 14.0, card.tag_color));
                // The pips: rank now, and the one this pick fills.
                if let Some((have, max)) = card.ranks {
                    c.spawn(Node {
                        column_gap: Val::Px(4.0),
                        margin: UiRect::vertical(Val::Px(2.0)),
                        ..default()
                    })
                    .with_children(|pips| {
                        for r in 0..max {
                            let color = if r < have {
                                hud::GOLD_TONE
                            } else if r == have {
                                rc
                            } else {
                                Color::srgba(1.0, 1.0, 1.0, 0.12)
                            };
                            pips.spawn((
                                Node {
                                    width: Val::Px(14.0),
                                    height: Val::Px(6.0),
                                    border_radius: BorderRadius::all(Val::Px(2.0)),
                                    ..default()
                                },
                                BackgroundColor(color),
                            ));
                        }
                    });
                }
                c.spawn((
                    Node {
                        width: Val::Percent(80.0),
                        height: Val::Px(1.0),
                        margin: UiRect::vertical(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundGradient::from(LinearGradient::to_right(vec![
                        ColorStop::percent(rc.with_alpha(0.0), 0.0),
                        ColorStop::percent(rc.with_alpha(0.8), 50.0),
                        ColorStop::percent(rc.with_alpha(0.0), 100.0),
                    ])),
                ));
                c.spawn(hud::centered_bundle(
                    f,
                    card.desc,
                    14.0,
                    Color::srgb(0.92, 0.9, 0.84),
                ));
                // The landing shine, a bright band that crosses once.
                c.spawn((
                    CardShine(i),
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(-40.0),
                        bottom: Val::Px(-40.0),
                        width: Val::Px(70.0),
                        left: Val::Px(-120.0),
                        ..default()
                    },
                    BackgroundGradient::from(LinearGradient::to_right(vec![
                        ColorStop::percent(Color::srgba(1.0, 1.0, 1.0, 0.0), 0.0),
                        ColorStop::percent(Color::srgba(1.0, 0.97, 0.85, 0.28), 50.0),
                        ColorStop::percent(Color::srgba(1.0, 1.0, 1.0, 0.0), 100.0),
                    ])),
                    UiTransform::from_rotation(Rot2::degrees(18.0)),
                    bevy::ui::FocusPolicy::Pass,
                ));
            });
        }
    });
}

fn ease_out_back(t: f32) -> f32 {
    let c1 = 1.70158;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

/// Deal, tilt, lift and burst: real time, since the game clock is paused under the picker.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn animate_cards(
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    mut cards: Query<(
        &mut Card,
        &Interaction,
        &RelativeCursorPosition,
        &mut UiTransform,
        &mut BoxShadow,
        &mut Visibility,
    )>,
    mut shines: Query<(&CardShine, &mut Node)>,
    mut rays: Query<&mut UiTransform, (With<Rays>, Without<Card>)>,
    mut pending: ResMut<PendingPick>,
    mut run: ResMut<Run>,
    mut offer: ResMut<Offer>,
    mut rng: ResMut<Rng>,
    mut next: ResMut<NextState<Phase>>,
) {
    if *phase.get() != Phase::LevelUp {
        pending.0 = None;
        return;
    }
    let dt = time.delta_secs().min(0.05);
    let now = time.elapsed_secs();
    for mut r in &mut rays {
        r.rotation = Rot2::radians(now * 0.12);
        r.scale = Vec2::splat(1.0 + 0.03 * (now * 1.7).sin());
    }
    let chosen = pending.0.map(|(i, _)| i);
    let mut shine_at = [None::<f32>; 4];
    for (mut card, interaction, cursor, mut tf, mut shadow, mut vis) in &mut cards {
        let card = &mut *card;
        card.t += dt;
        if card.t < 0.0 {
            *vis = Visibility::Hidden;
            continue;
        }
        *vis = Visibility::Inherited;
        let deal = (card.t / DEAL_SECS).clamp(0.0, 1.0);
        let e = ease_out_back(deal);
        let hot = *interaction != Interaction::None && chosen.is_none() && deal >= 1.0;
        let k = 1.0 - (-dt * 14.0).exp();
        let hv = card.hover;
        card.hover = hv + ((if hot { 1.0 } else { 0.0 }) - hv) * k;
        let want_tilt = if hot {
            cursor
                .normalized
                .unwrap_or_default()
                .clamp(Vec2::splat(-0.5), Vec2::splat(0.5))
        } else {
            Vec2::ZERO
        };
        let tl = card.tilt;
        card.tilt = tl + (want_tilt - tl) * k;
        let h = card.hover;
        // The pick: the chosen card swells and rises, the others drop away.
        let (pick_scale, pick_y, pick_fade) = match (chosen, card.picked) {
            (Some(c), _) if c == card.index => {
                let p = card.picked.get_or_insert(0.0);
                *p += dt;
                let u = (*p / PICK_SECS).clamp(0.0, 1.0);
                (1.0 + 0.18 * u, -30.0 * u, 0.0)
            }
            (Some(_), _) => {
                let p = card.picked.get_or_insert(0.0);
                *p += dt;
                let u = (*p / PICK_SECS).clamp(0.0, 1.0);
                (1.0 - 0.15 * u, 140.0 * u * u, u)
            }
            _ => (1.0, 0.0, 0.0),
        };
        // A perspective tilt, faked: the card turns about the cursor's side and leans with it.
        let lean = card.tilt.x * 7.0;
        let squash = 1.0 - card.tilt.x.abs() * 0.08;
        let bob = (now * 2.2 + card.index as f32 * 1.3).sin() * 1.5 * (1.0 - h);
        tf.translation = Val2::px(
            0.0,
            (1.0 - e) * 260.0 - 18.0 * h + card.tilt.y * 8.0 + bob + pick_y,
        );
        let s = (0.6 + 0.4 * e) * (1.0 + 0.06 * h) * pick_scale;
        tf.scale = Vec2::new(s * squash, s * (1.0 + card.tilt.y.abs() * 0.02));
        tf.rotation = Rot2::degrees((1.0 - e) * -14.0 + lean);
        let rc = card.rarity.color();
        if let Some(sh) = shadow.0.first_mut() {
            sh.color = rc.with_alpha((0.25 + 0.6 * h) * (1.0 - pick_fade));
            sh.blur_radius = Val::Px(18.0 + 22.0 * h);
            sh.spread_radius = Val::Px(2.0 + 4.0 * h);
            sh.y_offset = Val::Px(6.0 + 10.0 * h);
        }
        if pick_fade >= 1.0 {
            *vis = Visibility::Hidden;
        }
        if card.index < shine_at.len() {
            shine_at[card.index] = Some(card.t);
        }
    }
    for (shine, mut node) in &mut shines {
        let Some(t) = shine_at.get(shine.0).copied().flatten() else {
            continue;
        };
        let u = ((t - DEAL_SECS * 0.6) / 0.5).clamp(0.0, 1.0);
        node.left = Val::Px(-120.0 + u * (CARD_W + 200.0));
    }
    if let Some((i, t)) = pending.0.as_mut() {
        *t += dt;
        if *t >= PICK_SECS {
            let i = *i;
            pending.0 = None;
            hud::pick(i, &mut run, &mut offer, &mut rng, &mut next);
        }
    }
}

/// A click on a card, or 1/2/3: start its burst; the choice applies when it finishes.
pub(super) fn choose_card(
    phase: Res<State<Phase>>,
    keys: Res<ButtonInput<KeyCode>>,
    cards: Query<(&Card, &Interaction), Changed<Interaction>>,
    offer: Res<Offer>,
    mut pending: ResMut<PendingPick>,
) {
    if *phase.get() != Phase::LevelUp || pending.0.is_some() {
        return;
    }
    let mut pick = cards
        .iter()
        .find(|(c, i)| **i == Interaction::Pressed && c.t >= DEAL_SECS)
        .map(|(c, _)| c.index);
    for (i, key) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3]
        .into_iter()
        .enumerate()
    {
        if keys.just_pressed(key) {
            pick = Some(i);
        }
    }
    if let Some(i) = pick.filter(|&i| i < offer.len()) {
        pending.0 = Some((i, 0.0));
    }
}
