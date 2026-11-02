//! The "No game data" panel: what the pick screen becomes when there is no `Data` folder to
//! build it from. `open_world_assets` (benilla-world's `Startup`) inserts `WorldAssets` only once
//! the patch chain opened, and `menu::build_menu` waits on it forever otherwise, so an absent
//! `WorldAssets` after `Startup` is the one condition. The panel says what is needed, whose data
//! it is and the two ways to put it where the app looks, and its **Validate** button runs
//! `Chain::open` on the folder and shows the first error verbatim.
//!
//! Plain `bevy_ui`, not the glue widgets: those are drawn from the very archives that are
//! missing. The text takes Bevy's built-in font (`TextFont::default()`; `default_font` comes with
//! the workspace's `default_platform` bevy feature), since `Fonts/FRIZQT__.ttf` is in `fonts.MPQ`.
//!
//! The `mpq://` source registers before `AssetPlugin` builds (`crate::run`), so a folder that
//! validates needs a restart to be read: the panel says so.

use bevy::prelude::*;

use super::hud::MenuRoot;
use super::menu::MenuBuilt;

/// On [`MenuRoot`] once the panel is in; [`MenuBuilt`] is not set, so a chain that appears
/// later (it cannot today) would still build the menu.
#[derive(Component)]
struct NoDataBuilt;

/// The Validate button.
#[derive(Component)]
struct ValidateButton;

/// The status line Validate writes.
#[derive(Component)]
struct StatusText;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, (build_panel, validate));
}

const PANEL_BG: Color = Color::srgba(0.05, 0.05, 0.07, 0.96);
const PANEL_EDGE: Color = Color::srgb(0.55, 0.45, 0.2);
const BODY: Color = Color::srgb(0.85, 0.82, 0.75);
const HEAD: Color = Color::srgb(1.0, 0.82, 0.3);
const BUTTON: Color = Color::srgba(0.22, 0.1, 0.05, 0.98);
const BUTTON_HOT: Color = Color::srgba(0.36, 0.12, 0.06, 0.98);

/// What the panel says: the first paragraph is the situation, then the three routes.
const ABOUT: &str = "Warcraft Survivors runs on the art, maps and sounds of a World of Warcraft \
1.12.1 client. It ships none of them: the game needs the Data folder (about 5.5 GB of .MPQ \
archives) from a 1.12.1 client you own, read in place. Nothing is copied or uploaded.";

// Pushed into `files/`, not `files/Data`: `adb push` nests a folder inside an existing target,
// so a second push to `files/Data` would make `files/Data/Data` (scripts/android-data.sh).
const ROUTE_ADB: &str = "With a PC: connect the phone with USB debugging on and, from the game's \
source folder, run\n\
    make android-data\n\
or push a Data folder you already have:\n\
    adb push Data /sdcard/Android/data/io.github.songoku119.azerothsurvivors/files/\n\
then press Validate.";

const ROUTE_PICK: &str = "On the phone (coming with the Android app): allow \"All files \
access\" in the system settings, then pick the Data folder; it stays where it is.";

const ROUTE_DESKTOP: &str = "On a desktop: set WOW_DATA to the Data folder, or put Data (or \
WoW/Data) next to the executable.";

fn text(s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(s),
        TextFont {
            font_size: size,
            ..default()
        },
        TextColor(color),
        TextLayout::new(Justify::Left, LineBreak::WordBoundary),
    )
}

/// Build the panel into [`MenuRoot`] when `Startup` left no `WorldAssets` behind.
#[allow(clippy::type_complexity)]
fn build_panel(
    mut commands: Commands,
    roots: Query<Entity, (With<MenuRoot>, Without<MenuBuilt>, Without<NoDataBuilt>)>,
    assets: Option<Res<benilla_assets::WorldAssets>>,
) {
    if assets.is_some() {
        return;
    }
    let Ok(root) = roots.single() else { return };
    let status = probe();
    warn!("survivors: no game data — {status}");
    commands
        .entity(root)
        .insert(NoDataBuilt)
        .with_children(|ui| {
            ui.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
            ))
            .with_children(|center| {
                center
                    .spawn((
                        Node {
                            width: Val::Px(640.0),
                            max_width: Val::Percent(94.0),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(12.0),
                            padding: UiRect::all(Val::Px(22.0)),
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(PANEL_BG),
                        BorderColor::all(PANEL_EDGE),
                    ))
                    .with_children(|panel| {
                        panel.spawn(text("No game data", 26.0, HEAD));
                        panel.spawn(text(ABOUT, 15.0, BODY));
                        panel.spawn(text(ROUTE_ADB, 14.0, BODY));
                        panel.spawn(text(ROUTE_PICK, 14.0, BODY));
                        panel.spawn(text(ROUTE_DESKTOP, 14.0, BODY));
                        panel.spawn((StatusText, text(status, 14.0, HEAD)));
                        panel
                            .spawn((
                                Button,
                                ValidateButton,
                                Node {
                                    align_self: AlignSelf::FlexStart,
                                    padding: UiRect::axes(Val::Px(26.0), Val::Px(10.0)),
                                    border: UiRect::all(Val::Px(1.0)),
                                    ..default()
                                },
                                BackgroundColor(BUTTON),
                                BorderColor::all(PANEL_EDGE),
                            ))
                            .with_children(|b| {
                                b.spawn(text("Validate", 17.0, HEAD));
                            });
                    });
            });
        });
}

/// Validate: re-run the probe and show its line.
#[allow(clippy::type_complexity)]
fn validate(
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<ValidateButton>),
    >,
    mut status: Query<&mut Text, With<StatusText>>,
) {
    for (interaction, mut bg) in &mut buttons {
        bg.0 = match interaction {
            Interaction::None => BUTTON,
            _ => BUTTON_HOT,
        };
        if *interaction == Interaction::Pressed {
            let line = probe();
            info!("survivors: validate — {line}");
            for mut t in &mut status {
                t.0 = line.clone();
            }
        }
    }
}

/// The status line: where the data was looked for and not found, the first error opening the
/// chain, or that it opened (and a restart is needed, see the module doc).
fn probe() -> String {
    match benilla_formats::wow_data() {
        None => {
            let looked = benilla_formats::candidates();
            if looked.is_empty() {
                "No Data folder: WOW_DATA is set and empty.".to_string()
            } else {
                format!(
                    "No Data folder found. Looked in: {}",
                    looked
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        }
        Some(dir) => match benilla_formats::Chain::open(&dir) {
            Ok(_) => format!(
                "Data found and valid at {}. Restart Warcraft Survivors to play.",
                dir.display()
            ),
            Err(e) => format!("Data at {} does not open: {e:#}", dir.display()),
        },
    }
}
