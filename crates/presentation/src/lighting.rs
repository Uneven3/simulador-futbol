use bevy::prelude::*;

/// Stadium lighting. Purely presentational: light never decides contact,
/// visibility or any match outcome.
pub struct StadiumLightingPlugin;

impl Plugin for StadiumLightingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_lighting);
    }
}

fn setup_lighting(mut commands: Commands) {
    // Stadium floodlight/sun light
    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: true,
            illuminance: 32000.0,
            ..default()
        },
        Transform::from_xyz(15.0, -15.0, 35.0).looking_at(Vec3::ZERO, Vec3::Z),
    ));
}
