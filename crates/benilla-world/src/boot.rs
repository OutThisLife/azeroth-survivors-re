//! The tuned Bevy boot: the `DefaultPlugins` set every benilla binary stands on. The engine tuning
//! is shared; the `Window` is the caller's.

use bevy::app::{PluginGroupBuilder, TaskPoolOptions, TaskPoolPlugin};
use bevy::prelude::*;

use crate::thread_qos;

/// `DefaultPlugins` with benilla's engine tuning applied, around the caller's primary window.
#[cfg_attr(not(target_arch = "wasm32"), allow(unused_mut))]
pub fn tuned_default_plugins(mut primary_window: Window) -> PluginGroupBuilder {
    // The web build's canvas binding: with no `canvas` selector winit creates its own `<canvas>`
    // and appends it to `<body>` at a fixed size, ignoring the full-viewport element
    // `web/index.html` already gives it — so bind to that one by id instead and let it own the
    // surface. `prevent_default_event_handling` stops the page from scrolling/zooming under our
    // own keyboard and touch/wheel input (the browser's defaults for those events).
    #[cfg(target_arch = "wasm32")]
    {
        primary_window.canvas = Some("#benilla".into());
        primary_window.fit_canvas_to_parent = true;
        primary_window.prevent_default_event_handling = true;
    }
    DefaultPlugins
        .set(WindowPlugin {
            primary_window: Some(primary_window),
            ..default()
        })
        // Deliberately no `AssetPlugin::file_path`: a baked source path resolves only on the build
        // machine. Every shader is embedded (`embedded://<crate>/shaders/…`), so no root is read.
        // Quiet wgpu/naga; the ring keeps the last stderr lines for the crash report (`log_ring`).
        .set(bevy::log::LogPlugin {
            filter: "wgpu=error,naga=warn".into(),
            custom_layer: |_| {
                #[cfg(target_arch = "wasm32")]
                {
                    use bevy::log::tracing_subscriber::Layer as _;
                    Some(Box::new(crate::log_ring::LogRing.and_then(NoSpans)))
                }
                #[cfg(not(target_arch = "wasm32"))]
                Some(Box::new(crate::log_ring::LogRing))
            },
            ..default()
        })
        // Asset loads parse synchronously on the IO pool, and Bevy's default 4 threads saturate on
        // a dense teleport. Workers spawn at default QoS, behind any background build:
        // compute is user-interactive, IO and async compute user-initiated, and
        // `ThreadQosPlugin` promotes the render thread from inside.
        .set(TaskPoolPlugin {
            task_pool_options: TaskPoolOptions {
                io: bevy::app::TaskPoolThreadAssignmentPolicy {
                    min_threads: 2,
                    max_threads: 8,
                    percent: 0.5,
                    on_thread_spawn: Some(std::sync::Arc::new(|| {
                        thread_qos::promote_current_thread(thread_qos::QosClass::UserInitiated)
                    })),
                    on_thread_destroy: None,
                },
                async_compute: bevy::app::TaskPoolThreadAssignmentPolicy {
                    on_thread_spawn: Some(std::sync::Arc::new(|| {
                        thread_qos::promote_current_thread(thread_qos::QosClass::UserInitiated)
                    })),
                    ..TaskPoolOptions::default().async_compute
                },
                compute: bevy::app::TaskPoolThreadAssignmentPolicy {
                    on_thread_spawn: Some(std::sync::Arc::new(|| {
                        thread_qos::promote_current_thread(thread_qos::QosClass::UserInteractive)
                    })),
                    // `WOW_THREADS=1` serialises the frame's systems, a diagnostic: a defect that
                    // survives it is not a race between two systems.
                    max_threads: match std::env::var("WOW_THREADS").ok().as_deref() {
                        Some("1") => 1,
                        _ => TaskPoolOptions::default().compute.max_threads,
                    },
                    ..TaskPoolOptions::default().compute
                },
                ..default()
            },
        })
        // Sound is kira behind our own mixer; `bevy_audio` is off by feature (workspace
        // `Cargo.toml`). Kept though they look idle: gizmos (bowstring, fishing line), sprites (the
        // FrameXML quad pass), picking, TextPlugin (glue text), PostProcessPlugin (glow bloom) and
        // ScenePlugin (avian's collider backend reads `SceneSpawner`); the ForwardDecal family is
        // registered inside `PbrPlugin::build`, so it cannot be disabled on its own.
        //
        // M2/WMO/ADT load through our own `mpq://` loaders; there is no glTF.
        .disable::<bevy::gltf::GltfPlugin>()
        // No bevy AA: no Fxaa/TAA/SMAA/CAS component anywhere (MSAA is core render, unaffected).
        .disable::<bevy::anti_alias::AntiAliasPlugin>()
        // No gamepad input; 1.12's bindings are keyboard/mouse.
        .disable::<bevy::gilrs::GilrsPlugin>()
}

/// The web build's tracing without spans. Bevy's wasm log layer turns every entered span into a
/// `performance.mark` + `measure` pair with formatted names, unconditionally: about 5 % of the
/// browser's main thread in a busy scene (profiled 2026-10-13), and User Timing entries that
/// nothing ever clears. Disabling spans at the callsite keeps them from being built at all;
/// events (the console log) are untouched.
#[cfg(target_arch = "wasm32")]
struct NoSpans;

#[cfg(target_arch = "wasm32")]
impl<S: bevy::log::tracing::Subscriber> bevy::log::tracing_subscriber::Layer<S> for NoSpans {
    fn register_callsite(
        &self,
        meta: &'static bevy::log::tracing::Metadata<'static>,
    ) -> bevy::log::tracing::subscriber::Interest {
        if meta.is_span() {
            bevy::log::tracing::subscriber::Interest::never()
        } else {
            bevy::log::tracing::subscriber::Interest::always()
        }
    }

    fn enabled(
        &self,
        meta: &bevy::log::tracing::Metadata<'_>,
        _: bevy::log::tracing_subscriber::layer::Context<'_, S>,
    ) -> bool {
        !meta.is_span()
    }
}

