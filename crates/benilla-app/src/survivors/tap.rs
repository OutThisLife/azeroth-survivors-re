//! Taps on glue widgets. `glue::glue_clicks` takes a click from a press released back on the
//! widget, which it reads as `Interaction::Pressed → Hovered`: the mouse is still over the
//! button when the finger comes off it. A finger coming off a screen leaves nothing over the
//! button, so bevy_ui goes `Pressed → None` and the pick screen never heard the tap, though the
//! button showed the press. This runs after `glue_clicks` and adds that case: a widget pressed
//! last frame whose touch lifted this frame inside its node is a click.
//!
//! Only `Interaction::None` after a press is taken, so a mouse release (`Hovered`, which
//! `glue_clicks` has) is never counted twice; a touch that lifted elsewhere, or over a node
//! hidden mid-press, drops the press as the reference does.

use bevy::ecs::entity::EntityHashSet;
use bevy::prelude::*;
use bevy::ui::UiGlobalTransform;
use bevy::window::PrimaryWindow;

use super::input::node_hit;
use crate::glue::GlueClicks;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(PreUpdate, taps.after(crate::glue::glue_clicks));
}

/// Whether a widget's press ended as a tap: it was pressed, the press is gone without a hover
/// (`Interaction::None`), and a touch lifted inside it this frame.
pub(super) fn tapped(now: &Interaction, lifted_inside: bool) -> bool {
    *now == Interaction::None && lifted_inside
}

#[allow(clippy::type_complexity)]
fn taps(
    nodes: Query<(
        Entity,
        &Interaction,
        &ComputedNode,
        &UiGlobalTransform,
        &InheritedVisibility,
    )>,
    touches: Res<Touches>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut pressed: Local<EntityHashSet>,
    mut clicks: ResMut<GlueClicks>,
) {
    let scale = windows.single().map(|w| w.scale_factor()).unwrap_or(1.0);
    // Where fingers came off this frame, in the physical pixels `node_hit` reads.
    let lifted: Vec<Vec2> = touches
        .iter_just_released()
        .map(|t| t.position() * scale)
        .collect();
    pressed.retain(|&e| match nodes.get(e) {
        Ok((_, Interaction::Pressed, ..)) => true, // still held
        Ok((_, now, node, tf, vis)) => {
            if tapped(now, lifted.iter().any(|&p| node_hit(node, tf, vis, p))) {
                clicks.tap(e);
            }
            false
        }
        Err(_) => false, // despawned mid-press
    });
    for (e, interaction, ..) in &nodes {
        if *interaction == Interaction::Pressed {
            pressed.insert(e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_press_that_lifts_inside_without_a_hover_is_a_tap() {
        assert!(tapped(&Interaction::None, true));
        // Lifted elsewhere: the press is dropped.
        assert!(!tapped(&Interaction::None, false));
        // A mouse release: `glue_clicks` already has it.
        assert!(!tapped(&Interaction::Hovered, true));
        // Still held: not over yet.
        assert!(!tapped(&Interaction::Pressed, true));
    }
}
