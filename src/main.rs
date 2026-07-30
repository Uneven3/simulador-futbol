use bevy::prelude::*;

mod data;
mod math;
mod presentation;
mod simulation;

use data::{BallTouched, MatchState, PitchConfig};
use presentation::{GameCameraPlugin, PitchMeshPlugin, RenderSetupPlugin};
use simulation::{
    BallCollisionPlugin, BallPhysicsPlugin, PlayerMovementPlugin, RefereePlugin,
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
        // Game Layers
        .add_plugins((
            SimulationOrderPlugin,
            BallPhysicsPlugin,
            BallCollisionPlugin,
            RefereePlugin,
            PlayerMovementPlugin,
            PitchMeshPlugin,
            RenderSetupPlugin,
            GameCameraPlugin,
        ))
        .run();
}
