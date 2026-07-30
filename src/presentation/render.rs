use crate::data::Ball;
use bevy::prelude::*;

pub struct RenderSetupPlugin;

impl Plugin for RenderSetupPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (setup_lighting, spawn_ball));
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

fn spawn_ball(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let ball_mesh = meshes.add(Sphere::new(0.11));
    let ball_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.25,
        reflectance: 0.6,
        ..default()
    });

    // The ball is simulated analytically (see simulation::ball_physics); its
    // Transform (position + orientation) is written by the simulation each tick.
    commands.spawn((
        Name::new("Ball"),
        Ball::default(),
        Mesh3d(ball_mesh),
        MeshMaterial3d(ball_material),
        Transform::from_xyz(0.0, 0.0, 0.11), // Start at center spot
    ));
}
