//! The survivors grade: a light ReShade-style polish (CAS sharpening, vibrance, a soft S-curve, a
//! touch of warmth) on the world camera, after the FFXGlow filter passes and before the HUD, which
//! composites over it untouched. On by default in survivors; `G` toggles it in play, and
//! `?grade=0` (`WOW_GRADE=0` native) starts it off.
//!
//! It is Bevy's [`FullscreenMaterial`]: one uniform, one fragment shader
//! (`shaders/survivors_grade.wgsl`), and a node the plugin places between FFXGlow and the end of
//! the world's post-processing.

use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::core_pipeline::fullscreen_material::{FullscreenMaterial, FullscreenMaterialPlugin};
use bevy::prelude::*;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::render_graph::{
    InternedRenderLabel, InternedRenderSubGraph, RenderLabel, RenderSubGraph,
};
use bevy::render::render_resource::ShaderType;
use bevy::shader::ShaderRef;

use benilla_world::view::WorldCamera;

/// The grade's uniform, on the world camera. `amount` 0 is the identity, and the toggle eases it.
#[derive(Component, Clone, Copy, ExtractComponent, ShaderType)]
pub(super) struct Grade {
    pub amount: f32,
    /// CAS sharpness, 0..1.
    pub sharpness: f32,
    /// Extra saturation for the least saturated pixels.
    pub vibrance: f32,
    /// How much of the S-curve to blend in.
    pub contrast: f32,
}

impl Default for Grade {
    fn default() -> Self {
        Self {
            amount: 0.0,
            sharpness: 0.4,
            vibrance: 0.18,
            contrast: 0.12,
        }
    }
}

impl FullscreenMaterial for Grade {
    fn fragment_shader() -> ShaderRef {
        "embedded://benilla_app/shaders/survivors_grade.wgsl".into()
    }

    fn sub_graph() -> Option<InternedRenderSubGraph> {
        Some(Core3d.intern())
    }

    /// After the glow's filter passes, before the world's post-processing ends.
    fn node_edges() -> Vec<InternedRenderLabel> {
        vec![
            Node3d::Tonemapping.intern(),
            Self::node_label().intern(),
            Node3d::EndMainPassPostProcessing.intern(),
        ]
    }
}

/// Whether the grade is wanted; the camera's `amount` eases toward it.
#[derive(Resource)]
pub(super) struct GradeOn(pub(super) bool);

pub(super) fn plugin(app: &mut App) {
    let on = crate::webenv::var("WOW_GRADE").is_none_or(|v| v != "0");
    app.add_plugins(FullscreenMaterialPlugin::<Grade>::default())
        .insert_resource(GradeOn(on))
        .add_systems(
            Update,
            (attach, toggle, drive)
                .chain()
                .after(benilla_world::schedule::WorldStage::Input),
        );
}

fn attach(
    mut commands: Commands,
    cams: Query<Entity, (With<WorldCamera>, With<Camera3d>, Without<Grade>)>,
) {
    for e in &cams {
        commands.entity(e).insert(Grade::default());
    }
}

/// `G`, or the pause panel's toggle on touch.
fn toggle(input: Res<super::input::SurvivorsInput>, mut on: ResMut<GradeOn>) {
    if input.grade_toggle {
        on.0 = !on.0;
    }
}

/// Eases `amount` over a quarter second.
fn drive(time: Res<Time<Real>>, on: Res<GradeOn>, mut grades: Query<&mut Grade>) {
    let target = if on.0 { 1.0 } else { 0.0 };
    let k = 1.0 - (-time.delta_secs() * 12.0).exp();
    for mut g in &mut grades {
        g.amount += (target - g.amount) * k;
        if (g.amount - target).abs() < 1e-3 {
            g.amount = target;
        }
    }
}
