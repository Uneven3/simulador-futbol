use bevy::prelude::*;
use football_domain::{Attributes, BALL_RADIUS, Ball, Facing, Player, Position, TeamId};

/// Primitive representations of the authoritative bodies.
///
/// This plugin is a pure consumer: it creates one disposable visual entity per
/// simulation entity, linked with [`VisualOf`], and derives its `Transform` from
/// the authoritative [`Position`] and [`Facing`]. It never writes simulation
/// state, and removing it changes nothing about the match.
pub struct PrimitiveVisualsPlugin;

impl Plugin for PrimitiveVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_primitive_assets)
            .add_systems(FixedLast, sample_authoritative_positions)
            .add_systems(
                Update,
                (
                    (spawn_player_visuals, spawn_ball_visuals),
                    despawn_orphan_visuals,
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
