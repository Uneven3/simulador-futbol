use crate::visuals::BallVisual;
use bevy::prelude::*;

pub struct GameCameraPlugin;

impl Plugin for GameCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_camera)
            .add_systems(Update, camera_follow_system);
    }
}

#[derive(Component)]
pub struct GameCamera;

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, -35.0, 20.0).looking_at(Vec3::ZERO, Vec3::Z),
        GameCamera,
        AmbientLight {
            color: Color::WHITE,
            brightness: 750.0,
            ..default()
        },
    ));
}

/// Follows the ball's *representation* rather than its authoritative position,
/// so the camera inherits the same interpolation as everything else on screen.
fn camera_follow_system(
    ball: Single<&Transform, (With<BallVisual>, Without<GameCamera>)>,
    mut camera: Single<&mut Transform, (With<GameCamera>, Without<BallVisual>)>,
    time: Res<Time>,
) {
    let ball_pos = ball.translation;

    // Position camera at a smooth follow offset: X aligns with ball, Y lags behind, Z is elevated
    let target_pos = Vec3::new(
        ball_pos.x * 0.9, // Slightly compress X pan for a cinematic angle
        ball_pos.y - 28.0,
        16.0,
    );

    // Smooth transition
    let lerp_factor = (5.0 * time.delta_secs()).min(1.0);
    camera.translation = camera.translation.lerp(target_pos, lerp_factor);

    // Look at ball's current position
    camera.look_at(ball_pos, Vec3::Z);
}
