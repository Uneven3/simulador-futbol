use bevy::{
    mesh::{
        MeshBuilder, VertexAttributeValues,
        skinning::{SkinnedMesh, SkinnedMeshInverseBindposes},
    },
    prelude::*,
};
use football_domain::{Attributes, BALL_RADIUS, Ball, Facing, Player, Position, TeamId};

/// Primitive representations of the authoritative bodies: one disposable visual
/// entity per simulation entity, linked with [`VisualOf`], its `Transform`
/// derived from [`Position`] and [`Facing`]. Removing it changes no match (§1).
pub struct PrimitiveVisualsPlugin;

impl Plugin for PrimitiveVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_primitive_assets).add_systems(
            Update,
            (
                (spawn_player_visuals, spawn_ball_visuals),
                despawn_orphan_visuals,
                sample_authoritative_positions,
                interpolate_visual_transforms,
            )
                .chain(),
        );
    }
}

/// Low-poly, skinned player representations.  The rig is entirely a
/// presentation concern: its bones are descendants of [`VisualOf`], while the
/// authoritative player remains a mesh-free entity (§1, §2).
pub struct LowPolyVisualsPlugin;

impl Plugin for LowPolyVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_low_poly_assets).add_systems(
            Update,
            (
                (spawn_low_poly_player_visuals, spawn_low_poly_ball_visuals),
                despawn_orphan_visuals,
                sample_authoritative_positions,
                interpolate_visual_transforms,
            )
                .chain(),
        );
    }
}

/// Links a disposable presentation entity to the authoritative simulation
/// entity it represents.
#[derive(Component, Debug, Clone, Copy)]
pub struct VisualOf(pub Entity);

/// Marks the ball's representation, which the camera follows.
#[derive(Component)]
pub struct BallVisual;

/// The two most recent authoritative positions, sampled at the fixed-tick
/// boundary. Rendering runs at its own rate, so a frame lands between two
/// simulation ticks and interpolates instead of snapping to the last one.
#[derive(Component)]
struct PositionSamples {
    previous: Vec3,
    current: Vec3,
}

impl PositionSamples {
    fn resting_at(position: Vec3) -> Self {
        Self {
            previous: position,
            current: position,
        }
    }

    fn interpolated(&self, fraction: f32) -> Vec3 {
        self.previous.lerp(self.current, fraction)
    }
}

/// Meshes and materials shared by every primitive body.
#[derive(Resource)]
struct PrimitiveAssets {
    body: Handle<Mesh>,
    facing_marker: Handle<Mesh>,
    ball: Handle<Mesh>,
    home_kit: Handle<StandardMaterial>,
    away_kit: Handle<StandardMaterial>,
    ball_surface: Handle<StandardMaterial>,
}

/// Shared geometry for the deliberately small footballer rig.  Every player
/// part carrying a mesh also carries [`SkinnedMesh`]; this makes the model
/// replaceable by an imported rig without promoting its pose into match state.
#[derive(Resource)]
struct LowPolyAssets {
    torso: Handle<Mesh>,
    head: Handle<Mesh>,
    limb: Handle<Mesh>,
    ball: Handle<Mesh>,
    torso_bindposes: Handle<SkinnedMeshInverseBindposes>,
    chest_bindpose: Handle<SkinnedMeshInverseBindposes>,
    hip_bindpose: Handle<SkinnedMeshInverseBindposes>,
    home_kit: Handle<StandardMaterial>,
    away_kit: Handle<StandardMaterial>,
    skin: Handle<StandardMaterial>,
    ball_surface: Handle<StandardMaterial>,
}

const BODY_RADIUS: f32 = 0.35;

fn load_primitive_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(PrimitiveAssets {
        body: meshes.add(Capsule3d::new(BODY_RADIUS, 1.8 - 2.0 * BODY_RADIUS)),
        facing_marker: meshes.add(Cuboid::new(0.3, 0.12, 0.12)),
        ball: meshes.add(Sphere::new(BALL_RADIUS)),
        home_kit: materials.add(StandardMaterial {
            base_color: Color::srgb(0.85, 0.1, 0.15),
            perceptual_roughness: 0.5,
            ..default()
        }),
        away_kit: materials.add(StandardMaterial {
            base_color: Color::srgb(0.1, 0.25, 0.85),
            perceptual_roughness: 0.5,
            ..default()
        }),
        ball_surface: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.25,
            reflectance: 0.6,
            ..default()
        }),
    });
}

const RIG_HEIGHT: f32 = 1.8;
const HIP_HEIGHT: f32 = -0.36;
const CHEST_HEIGHT: f32 = 0.27;

fn low_poly_cylinder(radius: f32, height: f32, segments: u32) -> Mesh {
    Cylinder::new(radius, height)
        .mesh()
        .resolution(6)
        .segments(segments)
        .build()
}

/// Adds the pair of attributes which make a mesh eligible for Bevy skinning.
/// `joint` is an index into the matching [`SkinnedMesh::joints`] list.
fn fully_weighted(mut mesh: Mesh, joint: u16) -> Mesh {
    let vertex_count = mesh.count_vertices();
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_JOINT_INDEX,
        VertexAttributeValues::Uint16x4(vec![[joint, 0, 0, 0]; vertex_count]),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_JOINT_WEIGHT,
        vec![[1.0, 0.0, 0.0, 0.0]; vertex_count],
    );
    mesh
}

/// The torso bends at the chest: lower vertices use hip joint 0 and upper
/// vertices use chest joint 1.  The six-sided cylinder keeps it recognisably
/// low-poly while still exercising actual weighted skinning.
fn two_bone_torso() -> Mesh {
    let mut mesh = low_poly_cylinder(0.25, 1.02, 2);
    let positions = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(positions)) => positions,
        _ => unreachable!("a cylinder builder always creates Float32x3 positions"),
    };
    let mut indices = Vec::with_capacity(positions.len());
    let mut weights = Vec::with_capacity(positions.len());
    for position in positions {
        if position[1] < 0.0 {
            indices.push([0, 0, 0, 0]);
        } else {
            indices.push([1, 0, 0, 0]);
        }
        weights.push([1.0, 0.0, 0.0, 0.0]);
    }
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_JOINT_INDEX,
        VertexAttributeValues::Uint16x4(indices),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, weights);
    mesh
}

fn load_low_poly_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut bindposes: ResMut<Assets<SkinnedMeshInverseBindposes>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(LowPolyAssets {
        torso: meshes.add(two_bone_torso()),
        head: meshes.add(fully_weighted(Sphere::new(0.19).mesh().uv(8, 6), 0)),
        limb: meshes.add(fully_weighted(low_poly_cylinder(0.095, 0.72, 1), 0)),
        ball: meshes.add(Sphere::new(BALL_RADIUS).mesh().uv(12, 8)),
        torso_bindposes: bindposes.add(SkinnedMeshInverseBindposes::from(vec![
            Mat4::from_translation(Vec3::Y * -HIP_HEIGHT),
            Mat4::from_translation(Vec3::Y * -CHEST_HEIGHT),
        ])),
        chest_bindpose: bindposes.add(SkinnedMeshInverseBindposes::from(vec![
            Mat4::from_translation(Vec3::Y * -CHEST_HEIGHT),
        ])),
        hip_bindpose: bindposes.add(SkinnedMeshInverseBindposes::from(vec![
            Mat4::from_translation(Vec3::Y * -HIP_HEIGHT),
        ])),
        home_kit: materials.add(StandardMaterial {
            base_color: Color::srgb(0.85, 0.1, 0.15),
            perceptual_roughness: 0.7,
            ..default()
        }),
        away_kit: materials.add(StandardMaterial {
            base_color: Color::srgb(0.1, 0.25, 0.85),
            perceptual_roughness: 0.7,
            ..default()
        }),
        skin: materials.add(StandardMaterial {
            base_color: Color::srgb(0.48, 0.28, 0.16),
            perceptual_roughness: 0.8,
            ..default()
        }),
        ball_surface: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.25,
            reflectance: 0.6,
            ..default()
        }),
    });
}

fn attach_child(commands: &mut Commands, parent: Entity, child: Entity) {
    commands.entity(parent).add_child(child);
}

/// Spawns a minimal, all-skinned footballer.  Its model coordinate system is
/// Y-up; the model parent rotates it once into the pitch's Z-up coordinates.
fn spawn_low_poly_player_visuals(
    mut commands: Commands,
    assets: Res<LowPolyAssets>,
    new_players: Query<(Entity, &Player, &Attributes, &Position), Added<Player>>,
) {
    for (simulation_entity, player, attributes, position) in new_players.iter() {
        let kit = match player.id.team {
            TeamId::Home => assets.home_kit.clone(),
            TeamId::Away => assets.away_kit.clone(),
        };
        let root = commands
            .spawn((
                Name::new(format!("Low-poly visual of {}", player.id)),
                VisualOf(simulation_entity),
                PositionSamples::resting_at(position.0),
                Transform::from_translation(position.0),
                Visibility::default(),
            ))
            .id();
        let model = commands
            .spawn((
                Name::new("Skinned low-poly footballer"),
                Transform::from_xyz(0.0, 0.0, attributes.height * 0.5)
                    .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
                    .with_scale(Vec3::splat(attributes.height / RIG_HEIGHT)),
            ))
            .id();
        attach_child(&mut commands, root, model);

        let hip = commands
            .spawn((
                Name::new("Hip bone"),
                Transform::from_translation(Vec3::Y * HIP_HEIGHT),
            ))
            .id();
        let chest = commands
            .spawn((
                Name::new("Chest bone"),
                Transform::from_translation(Vec3::Y * CHEST_HEIGHT),
            ))
            .id();
        attach_child(&mut commands, model, hip);
        attach_child(&mut commands, model, chest);

        let torso = commands
            .spawn((
                Name::new("Skinned torso"),
                Mesh3d(assets.torso.clone()),
                MeshMaterial3d(kit.clone()),
                SkinnedMesh {
                    inverse_bindposes: assets.torso_bindposes.clone(),
                    joints: vec![hip, chest],
                },
            ))
            .id();
        let head = commands
            .spawn((
                Name::new("Skinned head"),
                Mesh3d(assets.head.clone()),
                MeshMaterial3d(assets.skin.clone()),
                Transform::from_translation(Vec3::Y * 0.69),
                SkinnedMesh {
                    inverse_bindposes: assets.chest_bindpose.clone(),
                    joints: vec![chest],
                },
            ))
            .id();
        attach_child(&mut commands, model, torso);
        attach_child(&mut commands, model, head);

        for (side, label) in [(-1.0_f32, "left"), (1.0_f32, "right")] {
            let leg = commands
                .spawn((
                    Name::new(format!("Skinned {label} leg")),
                    Mesh3d(assets.limb.clone()),
                    MeshMaterial3d(kit.clone()),
                    Transform::from_xyz(side * 0.13, -0.69, 0.0),
                    SkinnedMesh {
                        inverse_bindposes: assets.hip_bindpose.clone(),
                        joints: vec![hip],
                    },
                ))
                .id();
            let arm = commands
                .spawn((
                    Name::new(format!("Skinned {label} arm")),
                    Mesh3d(assets.limb.clone()),
                    MeshMaterial3d(kit.clone()),
                    Transform::from_xyz(side * 0.34, 0.17, 0.0)
                        .with_rotation(Quat::from_rotation_z(side * 0.38)),
                    SkinnedMesh {
                        inverse_bindposes: assets.chest_bindpose.clone(),
                        joints: vec![chest],
                    },
                ))
                .id();
            attach_child(&mut commands, model, leg);
            attach_child(&mut commands, model, arm);
        }
    }
}

fn spawn_low_poly_ball_visuals(
    mut commands: Commands,
    assets: Res<LowPolyAssets>,
    new_balls: Query<(Entity, &Position), Added<Ball>>,
) {
    for (simulation_entity, position) in new_balls.iter() {
        commands.spawn((
            Name::new("Low-poly visual of ball"),
            VisualOf(simulation_entity),
            BallVisual,
            PositionSamples::resting_at(position.0),
            Mesh3d(assets.ball.clone()),
            MeshMaterial3d(assets.ball_surface.clone()),
            Transform::from_translation(position.0),
        ));
    }
}

/// A capsule for the body plus a small nose that makes [`Facing`] readable —
/// a bare capsule is rotationally symmetric and hides where the player looks.
fn spawn_player_visuals(
    mut commands: Commands,
    assets: Res<PrimitiveAssets>,
    new_players: Query<(Entity, &Player, &Attributes, &Position), Added<Player>>,
) {
    for (simulation_entity, player, attributes, position) in new_players.iter() {
        let kit = match player.id.team {
            TeamId::Home => assets.home_kit.clone(),
            TeamId::Away => assets.away_kit.clone(),
        };
        commands
            .spawn((
                Name::new(format!("Visual of {}", player.id)),
                VisualOf(simulation_entity),
                PositionSamples::resting_at(position.0),
                Transform::from_translation(position.0),
                Visibility::default(),
            ))
            .with_children(|body| {
                // Bevy capsules are Y-up and this pitch is Z-up, so the body
                // mesh needs a 90° X rotation or the players lie flat.
                body.spawn((
                    Mesh3d(assets.body.clone()),
                    MeshMaterial3d(kit.clone()),
                    Transform::from_xyz(0.0, 0.0, attributes.height * 0.5)
                        .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                ));
                body.spawn((
                    Mesh3d(assets.facing_marker.clone()),
                    MeshMaterial3d(kit),
                    Transform::from_xyz(BODY_RADIUS, 0.0, attributes.height * 0.8),
                ));
            });
    }
}

fn spawn_ball_visuals(
    mut commands: Commands,
    assets: Res<PrimitiveAssets>,
    new_balls: Query<(Entity, &Position), Added<Ball>>,
) {
    for (simulation_entity, position) in new_balls.iter() {
        commands.spawn((
            Name::new("Visual of ball"),
            VisualOf(simulation_entity),
            BallVisual,
            PositionSamples::resting_at(position.0),
            Mesh3d(assets.ball.clone()),
            MeshMaterial3d(assets.ball_surface.clone()),
            Transform::from_translation(position.0),
        ));
    }
}

/// Runs once per fixed tick, after the simulation has settled, so every visual
/// holds the segment (previous tick → current tick) it has to interpolate over.
fn sample_authoritative_positions(
    mut visuals: Query<(&VisualOf, &mut PositionSamples)>,
    positions: Query<&Position>,
) {
    for (visual_of, mut samples) in visuals.iter_mut() {
        if let Ok(position) = positions.get(visual_of.0) {
            samples.previous = samples.current;
            samples.current = position.0;
        }
    }
}

fn interpolate_visual_transforms(
    fixed_time: Res<Time<Fixed>>,
    mut visuals: Query<(&VisualOf, &PositionSamples, &mut Transform)>,
    bodies: Query<(Option<&Facing>, Option<&Ball>)>,
) {
    let fraction = fixed_time.overstep_fraction();
    for (visual_of, samples, mut transform) in visuals.iter_mut() {
        transform.translation = samples.interpolated(fraction);
        let Ok((facing, ball)) = bodies.get(visual_of.0) else {
            continue;
        };
        // Orientation is taken from the current tick rather than interpolated:
        // both facing and ball spin already change smoothly per tick.
        if let Some(facing) = facing {
            transform.rotation = Quat::from_rotation_z(facing.0.to_angle());
        } else if let Some(ball) = ball {
            transform.rotation = ball.orientation;
        }
    }
}

/// Visuals are disposable: when the body they represent leaves the simulation,
/// its representation goes with it.
fn despawn_orphan_visuals(
    mut commands: Commands,
    visuals: Query<(Entity, &VisualOf)>,
    bodies: Query<&Position>,
) {
    for (visual, visual_of) in visuals.iter() {
        if bodies.get(visual_of.0).is_err() {
            commands.entity(visual).despawn();
        }
    }
}
