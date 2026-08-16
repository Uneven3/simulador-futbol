use bevy::prelude::*;

use football_presentation::{
    DebugHubPlugin, DiagnosticOverlaysPlugin, GameCameraPlugin, LowPolyVisualsPlugin,
    MatchHudPlugin, MatchPlaybackPlugin, PitchMeshPlugin, StadiumLightingPlugin,
};
use football_simulation::MatchKernelPlugin;
use gameplayfootball::scenarios;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Gameplay Football (Bevy)".to_string(),
                ..default()
            }),
            ..default()
        }))
        // The authoritative match: one scenario, which also fixes the tick rate,
        // the pitch and the seed. It runs identically without anything below.
        .add_plugins(MatchKernelPlugin::new(scenarios::lab_match()))
        // Presentation: reads the simulation, never writes it
        .add_plugins((
            LowPolyVisualsPlugin,
            DiagnosticOverlaysPlugin,
            DebugHubPlugin,
            MatchHudPlugin,
            MatchPlaybackPlugin,
            PitchMeshPlugin,
            StadiumLightingPlugin,
            GameCameraPlugin,
        ))
        .run();
}
