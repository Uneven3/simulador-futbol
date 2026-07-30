//! The scoreboard, as one of two sinks over the same snapshot.
//!
//! Nothing is formatted here. The numbers on screen and the numbers in the log
//! have to agree, and the only way to guarantee that is for both to render the
//! same values rather than each formatting its own.

use bevy::prelude::*;
use football_domain::diagnostics::MatchSnapshot;

pub struct MatchHudPlugin;

impl Plugin for MatchHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_hud).add_systems(
            Update,
            draw_snapshot.run_if(resource_exists::<MatchSnapshot>),
        );
    }
}

#[derive(Component)]
struct HudText;

fn spawn_hud(mut commands: Commands) {
    commands.spawn((
        Name::new("Match HUD"),
        HudText,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(20.0),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        },
    ));
}

fn draw_snapshot(snapshot: Res<MatchSnapshot>, mut hud: Query<&mut Text, With<HudText>>) {
    let Ok(mut text) = hud.single_mut() else {
        return;
    };
    let drawn = snapshot.lines().join("\n");
    if text.0 != drawn {
        text.0 = drawn;
    }
}
