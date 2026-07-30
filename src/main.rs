use bevy::prelude::*;

use football_domain::{BallTouched, MatchState, PitchConfig};
use football_presentation::{
    GameCameraPlugin, PitchMeshPlugin, PrimitiveVisualsPlugin, StadiumLightingPlugin,
};
use football_simulation::{
    BallCollisionPlugin, BallPhysicsPlugin, MatchSetupPlugin, PlayerMovementPlugin, RefereePlugin,
    SimulationOrderPlugin,
};

fn main() {
    App::new()
        // Default Bevy plugins
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Gameplay Football (Bevy)".to_string(),
                ..default()
            }),
            ..default()
        }))
        // The original engine simulates at 100 Hz (10 ms steps); the analytical
        // ball integrator depends on this rate.
        .insert_resource(Time::<Fixed>::from_hz(100.0))
        // Global match configurations and states
        .insert_resource(MatchState::default())
        .insert_resource(PitchConfig::default())
        .add_message::<BallTouched>()
        // Authoritative simulation: runs identically without any of the below
        .add_plugins((
            MatchSetupPlugin,
            SimulationOrderPlugin,
            BallPhysicsPlugin,
            BallCollisionPlugin,
            RefereePlugin,
            PlayerMovementPlugin,
        ))
        // Presentation: reads the simulation, never writes it
        .add_plugins((
            PrimitiveVisualsPlugin,
            PitchMeshPlugin,
            StadiumLightingPlugin,
            GameCameraPlugin,
        ))
        .run();
}
