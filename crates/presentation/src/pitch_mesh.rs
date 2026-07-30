use bevy::prelude::*;
use football_domain::PitchConfig;

pub struct PitchMeshPlugin;

impl Plugin for PitchMeshPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_pitch_visuals);
    }
}

fn spawn_pitch_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    pitch_config: Res<PitchConfig>,
) {
    // 1. Spawning Grass Mesh (Sleek grass green HSL-tailored)
    let grass_color = Color::srgb(0.08, 0.45, 0.12);
    // This game is Z-up; Plane3d::default() faces +Y, so give the plane an
    // explicit +Z normal or the grass renders as a vertical wall.
    let grass_mesh = meshes.add(Plane3d::new(Vec3::Z, Vec2::splat(75.0)).mesh());
    let grass_material = materials.add(StandardMaterial {
        base_color: grass_color,
        perceptual_roughness: 0.8,
        ..default()
    });

    commands.spawn((
        Name::new("Grass Visual"),
        Mesh3d(grass_mesh),
        MeshMaterial3d(grass_material),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    // 2. Spawn White lines
    let line_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.5,
        ..default()
    });

    let hw = pitch_config.half_width;
    let hh = pitch_config.half_height;
    let lw = pitch_config.line_half_width;

    // Line mesh templates (thin cuboids elevated by 0.002 to avoid z-fighting)
    // Horizontal lines (along Y)
    let horiz_line_mesh = meshes.add(Cuboid::new(lw * 2.0, hh * 2.0, 0.001));
    // Vertical lines (along X)
    let vert_line_mesh = meshes.add(Cuboid::new(hw * 2.0, lw * 2.0, 0.001));

    // Left backline
    commands.spawn((
        Mesh3d(horiz_line_mesh.clone()),
        MeshMaterial3d(line_material.clone()),
        Transform::from_xyz(-hw, 0.0, 0.002),
    ));
    // Right backline
    commands.spawn((
        Mesh3d(horiz_line_mesh.clone()),
        MeshMaterial3d(line_material.clone()),
        Transform::from_xyz(hw, 0.0, 0.002),
    ));
    // Bottom touchline
    commands.spawn((
        Mesh3d(vert_line_mesh.clone()),
        MeshMaterial3d(line_material.clone()),
        Transform::from_xyz(0.0, -hh, 0.002),
    ));
    // Top touchline
    commands.spawn((
        Mesh3d(vert_line_mesh.clone()),
        MeshMaterial3d(line_material.clone()),
        Transform::from_xyz(0.0, hh, 0.002),
    ));
    // Center line
    commands.spawn((
        Mesh3d(horiz_line_mesh.clone()),
        MeshMaterial3d(line_material.clone()),
        Transform::from_xyz(0.0, 0.0, 0.002),
    ));

    // Center circle: a thin torus ring. Bevy tori (like cylinders) are Y-up,
    // so rotate 90° about X to lay it flat on the Z-up pitch.
    let circle_mesh = meshes.add(Torus::new(9.15 - lw, 9.15 + lw));
    commands.spawn((
        Mesh3d(circle_mesh),
        MeshMaterial3d(line_material.clone()),
        Transform::from_xyz(0.0, 0.0, 0.0)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
    ));

    // Goal frames (posts + crossbar). Cylinders are Y-up: posts need the 90° X
    // rotation to stand upright; crossbars lie along Y, which IS the goal width
    // axis, so they stay unrotated.
    let goal_h = pitch_config.goal_height;
    let goal_hw = pitch_config.goal_half_width;
    let pr = pitch_config.post_radius;
    let post_mesh = meshes.add(Cylinder::new(pr, goal_h));
    let crossbar_mesh = meshes.add(Cylinder::new(pr, goal_hw * 2.0 + pr * 2.0));
    let white_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.3,
        ..default()
    });

    for side in [-1.0f32, 1.0] {
        for y_side in [-1.0f32, 1.0] {
            commands.spawn((
                Name::new("Goal Post"),
                Mesh3d(post_mesh.clone()),
                MeshMaterial3d(white_material.clone()),
                Transform::from_xyz(hw * side, goal_hw * y_side, goal_h / 2.0)
                    .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            ));
        }
        commands.spawn((
            Name::new("Goal Crossbar"),
            Mesh3d(crossbar_mesh.clone()),
            MeshMaterial3d(white_material.clone()),
            Transform::from_xyz(hw * side, 0.0, goal_h),
        ));
    }
}
