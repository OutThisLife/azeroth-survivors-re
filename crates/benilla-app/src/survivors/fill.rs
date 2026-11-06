//! The overlays' big soft gradients as one dithered material (`shaders/survivors_fill.wgsl`).
//! Bevy's `BackgroundGradient` writes straight to the 8-bit target, so long fades across dark
//! values band; this mixes the same way (Oklab) and adds sub-byte noise before the write.

use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use bevy::ui_render::ui_material::UiMaterial;

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(super) struct FillMaterial {
    #[uniform(0)]
    fill: FillUniform,
}

#[derive(Clone, Copy, ShaderType)]
struct FillUniform {
    a: Vec4,
    b: Vec4,
    c: Vec4,
    glow: Vec4,
    glow_at: Vec4,
    shape: Vec4,
}

impl UiMaterial for FillMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://benilla_app/shaders/survivors_fill.wgsl".into()
    }
}

fn lin(c: Color) -> Vec4 {
    c.to_linear().to_vec4()
}

impl FillMaterial {
    /// Top to bottom through `mid` at `mid_at` (0..1), with corners rounded by `radius`, a
    /// fraction of the node's height.
    pub(super) fn vertical(top: Color, mid: Color, mid_at: f32, bottom: Color, radius: f32) -> Self {
        Self {
            fill: FillUniform {
                a: lin(top),
                b: lin(mid),
                c: lin(bottom),
                glow: Vec4::ZERO,
                glow_at: Vec4::ZERO,
                shape: Vec4::new(radius, 0.0, mid_at, 0.0),
            },
        }
    }

    /// From `centre` at the middle to `edge` at the farthest corner.
    pub(super) fn radial(centre: Color, edge: Color) -> Self {
        Self {
            fill: FillUniform {
                a: lin(centre),
                b: Vec4::ZERO,
                c: lin(edge),
                glow: Vec4::ZERO,
                glow_at: Vec4::ZERO,
                shape: Vec4::new(0.0, 1.0, 0.5, 0.0),
            },
        }
    }

    /// Lays an elliptical light over the fill: `color` (its alpha is the peak) at `at` in UV,
    /// fading to nothing at `radii`, fractions of the node's height.
    pub(super) fn with_glow(mut self, color: Color, at: Vec2, radii: Vec2) -> Self {
        self.fill.glow = lin(color);
        self.fill.glow_at = Vec4::new(at.x, at.y, radii.x, radii.y);
        self
    }
}
