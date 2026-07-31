//! Diagnostic overlays: the primitives as a scientific instrument.
//!
//! Each overlay draws a value the simulation already published — the ball's own
//! prediction buffer, the referee's judged offside line, the team AI's
//! designated player. None of them recomputes a rule: a second implementation
//! of a rule in this layer would diverge silently and show a decision that never
//! happened.
//!
//! Overlays read authoritative [`Position`] rather than the interpolated
//! transforms of the visuals, because an instrument should show the truth at the
//! current tick, not a frame-smoothed version of it.

use bevy::prelude::*;
use football_domain::{
    Attributes, Ball, Facing, MatchState, ObservationMemory, OffsideRecords, PitchConfig, Player,
    Position, PossessionDesignation, SetPiece, TeamId, Velocity, Vision,
};

pub struct DiagnosticOverlaysPlugin;

impl Plugin for DiagnosticOverlaysPlugin {
    fn build(&self, app: &mut App) {
        // The switches are the debug hub's; this plugin only draws. The guard
        // matters for a headless run, which has no renderer: without it the
        // overlays would take the app down instead of standing down.
        app.init_resource::<OverlaySettings>().add_systems(
            Update,
            (
                draw_velocities,
                draw_ball_future,
                draw_possession,
                draw_offside_judgement,
                draw_restart_spot,
                draw_vision,
            )
                .run_if(resource_exists::<GizmoConfigStore>),
        );
    }
}

/// Which overlays are on. Every one can be toggled from the debug hub, so the
/// same run can be read as truth, as intent, or as a refereeing decision
/// without restarting it.
#[derive(Resource, Debug, Clone)]
pub struct OverlaySettings {
    pub velocities: bool,
    pub ball_future: bool,
    pub possession: bool,
    pub offside: bool,
    pub restart_spot: bool,
    pub vision: bool,
}

impl Default for OverlaySettings {
    fn default() -> Self {
        Self {
            velocities: true,
            ball_future: true,
            possession: true,
            offside: true,
            restart_spot: true,
            // apagado: veintidós conos y veintidós telarañas tapan el fútbol
            vision: false,
        }
    }
}

const HOME_COLOUR: Srgba = Srgba::new(1.0, 0.35, 0.4, 1.0);
const AWAY_COLOUR: Srgba = Srgba::new(0.4, 0.6, 1.0, 1.0);
const BALL_FUTURE_COLOUR: Srgba = Srgba::new(1.0, 0.95, 0.3, 1.0);
const REFEREE_COLOUR: Srgba = Srgba::new(1.0, 1.0, 1.0, 1.0);
const OFFSIDE_COLOUR: Srgba = Srgba::new(1.0, 0.5, 0.0, 1.0);

fn team_colour(team: TeamId) -> Srgba {
    match team {
        TeamId::Home => HOME_COLOUR,
        TeamId::Away => AWAY_COLOUR,
    }
}

/// How far ahead a velocity arrow reaches. Half a second is long enough to read
/// direction and pace at a glance, short enough not to cross the pitch.
pub const VELOCITY_LEAD: f32 = 0.5;

/// Where a body will be in `VELOCITY_LEAD` seconds if nothing changes. The arrow
/// is drawn at chest height so it reads over the body, not through the grass.
pub fn velocity_arrow(position: Position, velocity: Velocity, body_height: f32) -> (Vec3, Vec3) {
    let shoulder = Vec3::new(0.0, 0.0, body_height * 0.55);
    let start = position.0 + shoulder;
    (start, start + velocity.0 * VELOCITY_LEAD)
}

fn draw_velocities(
    mut gizmos: Gizmos,
    settings: Res<OverlaySettings>,
    players: Query<(&Position, &Velocity, &Player, &Attributes)>,
) {
    if !settings.velocities {
        return;
    }
    for (position, velocity, player, attributes) in players.iter() {
        if velocity.0.length_squared() < 0.01 {
            continue;
        }
        let (start, end) = velocity_arrow(*position, *velocity, attributes.height);
        gizmos.arrow(start, end, team_colour(player.id.team));
    }
}

/// The ball's own prediction buffer, thinned for legibility. Not an estimate
/// drawn for the viewer: it is the buffer the AI reads, and in this model the
/// prediction IS the physics.
pub fn ball_future_polyline(ball: &Ball, every_n_steps: usize) -> Vec<Vec3> {
    ball.predictions
        .iter()
        .step_by(every_n_steps.max(1))
        .copied()
        .collect()
}

fn draw_ball_future(mut gizmos: Gizmos, settings: Res<OverlaySettings>, balls: Query<&Ball>) {
    if !settings.ball_future {
        return;
    }
    for ball in balls.iter() {
        // one point per 50 ms of the 3 s horizon
        gizmos.linestrip(ball_future_polyline(ball, 5), BALL_FUTURE_COLOUR);
        // and a ring where the ball will be in one second
        if let Some(in_one_second) = ball.predictions.get(100) {
            gizmos
                .circle(
                    Isometry3d::from_translation(*in_one_second),
                    0.3,
                    BALL_FUTURE_COLOUR,
                )
                .resolution(16);
        }
    }
}

/// Radius of the ring marking a player the team AI has designated to go for the
/// ball; the player actually holding it gets the wider one.
pub const DESIGNATED_RING: f32 = 0.8;
pub const POSSESSOR_RING: f32 = 1.1;

fn draw_possession(
    mut gizmos: Gizmos,
    settings: Res<OverlaySettings>,
    designation: Res<PossessionDesignation>,
    match_state: Res<MatchState>,
    players: Query<(&Position, &Player)>,
) {
    if !settings.possession {
        return;
    }

    for (position, player) in players.iter() {
        let ring_at_feet = Isometry3d::from_translation(position.0 + Vec3::Z * 0.02);
        if designation.designated[player.id.team] == Some(player.id) {
            gizmos
                .circle(ring_at_feet, DESIGNATED_RING, team_colour(player.id.team))
                .resolution(24);
        }
        if match_state.possession_player == Some(player.id) {
            gizmos
                .circle(ring_at_feet, POSSESSOR_RING, REFEREE_COLOUR)
                .resolution(24);
        }
    }

    // A pass in flight is the one committed intention this model already has:
    // draw who it was meant for.
    if let Some(target) = match_state.pass_target
        && let Some((target_position, _)) = players.iter().find(|(_, p)| p.id == target)
    {
        let aim = match_state.pass_aim;
        gizmos.line(
            Vec3::new(aim.x, aim.y, 0.05),
            target_position.0 + Vec3::Z * 0.05,
            team_colour(target.team),
        );
    }
}

/// The offside line as a segment across the pitch, at the x the referee judged.
pub fn offside_line_segment(line_x: f32, pitch: &PitchConfig) -> (Vec3, Vec3) {
    (
        Vec3::new(line_x, -pitch.half_height, 0.05),
        Vec3::new(line_x, pitch.half_height, 0.05),
    )
}

fn draw_offside_judgement(
    mut gizmos: Gizmos,
    settings: Res<OverlaySettings>,
    records: Res<OffsideRecords>,
    pitch: Res<PitchConfig>,
) {
    if !settings.offside {
        return;
    }

    if let Some(line_x) = records.judged_line_x {
        let (start, end) = offside_line_segment(line_x, &pitch);
        gizmos.line(start, end, OFFSIDE_COLOUR);
    }

    // Where each recorded player stood at the moment of the touch — the position
    // the referee will rule on, not where that player is now.
    for (_, recorded_position) in &records.players {
        let at_the_touch = Vec3::new(recorded_position.x, recorded_position.y, 0.05);
        gizmos
            .circle(
                Isometry3d::from_translation(at_the_touch),
                0.5,
                OFFSIDE_COLOUR,
            )
            .resolution(12);
    }
}

fn draw_restart_spot(
    mut gizmos: Gizmos,
    settings: Res<OverlaySettings>,
    match_state: Res<MatchState>,
) {
    if !settings.restart_spot || match_state.set_piece == SetPiece::None {
        return;
    }
    let spot = match_state.restart_pos + Vec3::Z * 0.05;
    gizmos
        .circle(Isometry3d::from_translation(spot), 0.6, REFEREE_COLOUR)
        .resolution(20);
    gizmos.line(spot, spot + Vec3::Z * 2.0, REFEREE_COLOUR);
}

/// Los bordes del cono de visión de un jugador, a la altura de los ojos.
pub fn vision_cone(
    position: Position,
    facing: Dir2,
    vision: &Vision,
    eye_height: f32,
) -> [Vec3; 3] {
    let eyes = position.0 + Vec3::Z * eye_height;
    let edge = |angle: f32| {
        let direction = Vec2::from_angle(angle).rotate(*facing) * vision.range;
        eyes + Vec3::new(direction.x, direction.y, 0.0)
    };
    [eyes, edge(vision.half_angle), edge(-vision.half_angle)]
}

/// Qué alcanza a ver cada jugador y a quién tiene en la cabeza: el cono es el
/// sensor y las líneas finas la memoria. Las que apuntan fuera del cono son
/// dónde cree que sigue alguien a quien ya no ve.
fn draw_vision(
    mut gizmos: Gizmos,
    settings: Res<OverlaySettings>,
    time: Res<Time>,
    watchers: Query<(
        &Position,
        &Facing,
        &Player,
        &Attributes,
        &Vision,
        &ObservationMemory,
    )>,
) {
    if !settings.vision {
        return;
    }
    let now = time.elapsed();
    for (position, facing, player, attributes, vision, memory) in watchers.iter() {
        let colour = team_colour(player.id.team);
        let [eyes, left, right] = vision_cone(*position, facing.0, vision, attributes.height * 0.9);
        let faded = colour.with_alpha(0.25);
        gizmos.line(eyes, left, faded);
        gizmos.line(eyes, right, faded);

        for (_, seen) in memory.everyone() {
            let believed = seen.projected_to(now);
            // lo recién visto es lo que menos interesa: lo que se mira aquí es
            // cuánto se ha quedado atrás lo que cree
            let staleness = seen.age(now).as_secs_f32().min(3.0) / 3.0;
            gizmos.line(
                eyes,
                Vec3::new(believed.x, believed.y, 0.2),
                colour.with_alpha(0.05 + staleness * 0.35),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use football_domain::BALL_RADIUS;

    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "geometría exacta: la mitad del campo y una recta a velocidad constante"
    )]
    fn a_velocity_arrow_leads_the_body_by_half_a_second() {
        let position = Position(Vec3::new(10.0, -5.0, 0.0));
        let velocity = Velocity(Vec3::new(8.0, 0.0, 0.0));
        let (start, end) = velocity_arrow(position, velocity, 1.8);

        assert_eq!(start.truncate(), position.on_pitch());
        assert!(start.z > 0.9, "the arrow must sit at chest height");
        assert_eq!(end.x - start.x, 4.0, "8 m/s for half a second is 4 m");
    }

    #[test]
    fn a_still_body_gets_an_arrow_of_no_length() {
        let position = Position(Vec3::ZERO);
        let (start, end) = velocity_arrow(position, Velocity(Vec3::ZERO), 1.8);
        assert_eq!(start, end);
    }

    #[test]
    fn the_ball_future_is_the_ball_own_prediction() {
        let ball = Ball::placed_at(Vec3::new(0.0, 0.0, BALL_RADIUS), Vec3::new(10.0, 0.0, 0.0));
        let polyline = ball_future_polyline(&ball, 5);

        assert_eq!(polyline.len(), ball.predictions.len() / 5);
        assert_eq!(
            polyline[0], ball.predictions[0],
            "the line must start where the ball is"
        );
        assert_eq!(polyline[1], ball.predictions[5], "one point per 5 steps");
    }

    #[test]
    fn thinning_the_ball_future_by_zero_does_not_divide_by_zero() {
        let ball = Ball::placed_at(Vec3::ZERO, Vec3::ZERO);
        assert_eq!(
            ball_future_polyline(&ball, 0).len(),
            ball.predictions.len(),
            "a step of zero must fall back to every point"
        );
    }

    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "geometría exacta: la mitad del campo y una recta a velocidad constante"
    )]
    fn the_offside_line_spans_the_pitch_at_the_judged_x() {
        let pitch = PitchConfig::default();
        let (start, end) = offside_line_segment(12.5, &pitch);

        assert_eq!(start.x, 12.5);
        assert_eq!(end.x, 12.5);
        assert_eq!(start.y, -pitch.half_height);
        assert_eq!(end.y, pitch.half_height);
    }
}
