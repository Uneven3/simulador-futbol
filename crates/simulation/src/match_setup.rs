use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_time::prelude::*;
use football_domain::scenario::{PlayState, PlayerSetup, SIMULATION_HZ, Scenario};
use football_domain::{
    Attributes, Ball, BallTouched, Facing, FatigueState, MatchRng, MatchState, Mentality,
    MovementIntent, PitchSides, Player, PlayerId, PlayerMatchState, PlayerRegistry,
    PlayingPosition, Position, SetPiece, TeamId, TeamSide, Velocity,
};
use std::time::Duration;

/// Installs a scenario: the state the match starts from, its seed, its pitch and
/// its bodies.
///
/// The scenario is the single source of the initial situation, so a run is
/// reproducible from one value. Nothing here is renderable: these entities carry
/// domain state only, and their visual representations are created independently
/// by presentation and linked back with `VisualOf`.
pub struct MatchSetupPlugin {
    scenario: Scenario,
}

impl MatchSetupPlugin {
    pub fn new(scenario: Scenario) -> Self {
        Self { scenario }
    }
}

impl Plugin for MatchSetupPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.scenario.clone())
            .insert_resource(initial_match_state(&self.scenario))
            .insert_resource(self.scenario.pitch.clone())
            .insert_resource(self.scenario.regulations.clone())
            .insert_resource(self.scenario.tuning.clone())
            .insert_resource(MatchRng::seeded(self.scenario.seed))
            .insert_resource(PlayerRegistry::default())
            .insert_resource(Time::<Fixed>::from_hz(SIMULATION_HZ))
            .add_message::<BallTouched>()
            .add_systems(Startup, (spawn_scenario_ball, spawn_scenario_players))
            .add_systems(PreUpdate, (register_bodies, forget_departed_bodies));
    }
}

/// The match state a scenario opens in: stopped awaiting a restart, or live.
fn initial_match_state(scenario: &Scenario) -> MatchState {
    let mut state = MatchState::default();
    match scenario.play_state {
        PlayState::AwaitingRestart {
            set_piece,
            team,
            delay,
        } => {
            state.set_piece = set_piece;
            state.set_piece_team = Some(team);
            state.restart_in = delay;
            state.restart_pos = Vec3::ZERO;
            state.opening_kick_off_team = team;
        }
        PlayState::InPlay => {
            state.set_piece = SetPiece::None;
            state.set_piece_team = None;
            state.restart_in = Duration::ZERO;
        }
    }
    state
}

/// Height of an outfield body in metres. Provisional until anthropometry
/// becomes per-player data (MVP 3).
pub const PLAYER_HEIGHT: f32 = 1.8;

/// Keeps the identity → body index in step with the world.
///
/// Runs once per frame rather than per tick: bodies are only created at setup
/// today, and substitutions (MVP 2) will arrive as a request the kernel serves
/// at a restart, not mid-tick.
fn register_bodies(
    mut registry: ResMut<PlayerRegistry>,
    arrived: Query<(Entity, &Player), Added<Player>>,
) {
    for (body, player) in &arrived {
        registry.insert(player.id, body);
    }
}

fn forget_departed_bodies(
    mut registry: ResMut<PlayerRegistry>,
    mut departed: RemovedComponents<Player>,
) {
    for body in departed.read() {
        registry.remove_body(body);
    }
}

fn spawn_scenario_ball(mut commands: Commands, scenario: Res<Scenario>) {
    let setup = scenario.ball;
    let mut ball = Ball::placed_at(setup.position, setup.momentum);
    ball.last_touch_team = setup.last_touched_by_team;

    commands.spawn((Name::new("Ball"), ball, Position(setup.position)));
}

/// The default 4-4-2 for both teams: eleven bodies each, at their base
/// formation positions, standing still and facing the opponent goal.
fn spawn_scenario_players(mut commands: Commands, scenario: Res<Scenario>) {
    if scenario.players == PlayerSetup::BallOnly {
        return;
    }

    /// The default 4-4-2, shirt 1 to 11 in the order football numbers them.
    const LINE_UP: [PlayingPosition; 11] = [
        PlayingPosition::Goalkeeper,
        PlayingPosition::LeftBack,
        PlayingPosition::CentreBack,
        PlayingPosition::CentreBack,
        PlayingPosition::RightBack,
        PlayingPosition::LeftMidfielder,
        PlayingPosition::CentreMidfielder,
        PlayingPosition::CentreMidfielder,
        PlayingPosition::RightMidfielder,
        PlayingPosition::CentreForward,
        PlayingPosition::CentreForward,
    ];

    let sides = PitchSides::opening();
    for team in TeamId::BOTH {
        // a team faces the goal it attacks
        let attacking_direction = if sides.attacking_x(team) > 0.0 {
            Dir2::X
        } else {
            Dir2::NEG_X
        };

        for (index, position) in LINE_UP.iter().enumerate() {
            if scenario.players == PlayerSetup::GoalkeepersOnly
                && *position != PlayingPosition::Goalkeeper
            {
                continue;
            }
            let shirt_number = u8::try_from(index + 1).expect("LINE_UP is eleven players");
            let id = PlayerId::new(team, shirt_number);
            let base = base_formation_position(id, *position, sides);
            commands.spawn((
                Name::new(format!("{id} - {position:?}")),
                Player::new(id, *position, normalized_formation_position(id, *position)),
                Attributes {
                    height: PLAYER_HEIGHT,
                    ..Default::default()
                },
                Mentality::default(),
                PlayerMatchState::default(),
                Velocity::default(),
                MovementIntent::default(),
                FatigueState::default(),
                Facing(attacking_direction),
                Position::from_pitch(base, 0.0),
            ));
        }
    }
}

/// Normalized formation entry (original `FormationEntry::position`, -1..1 in
/// both axes) for the default 4-4-2; `AI_GetAdaptedFormationPosition` scales
/// this into the dynamic team block.
///
/// Paired positions are told apart by shirt, the way a team sheet does it: 3 is
/// the left-sided centre back, 7 the left-sided centre midfielder, 10 the
/// left-sided forward.
pub fn normalized_formation_position(id: PlayerId, position: PlayingPosition) -> Vec2 {
    match position {
        PlayingPosition::Goalkeeper => Vec2::new(-1.0, 0.0),
        PlayingPosition::LeftBack => Vec2::new(-1.0, -1.0),
        PlayingPosition::CentreBack => Vec2::new(-1.0, if id.shirt == 3 { -0.33 } else { 0.33 }),
        PlayingPosition::RightBack => Vec2::new(-1.0, 1.0),
        PlayingPosition::LeftMidfielder => Vec2::new(0.0, -1.0),
        PlayingPosition::CentreMidfielder => {
            Vec2::new(0.0, if id.shirt == 7 { -0.33 } else { 0.33 })
        }
        PlayingPosition::RightMidfielder => Vec2::new(0.0, 1.0),
        PlayingPosition::DefensiveMidfielder => Vec2::new(-0.5, 0.0),
        PlayingPosition::AttackingMidfielder => Vec2::new(0.5, 0.0),
        PlayingPosition::CentreForward => Vec2::new(1.0, if id.shirt == 10 { -0.4 } else { 0.4 }),
    }
}

/// Base (kick-off) position on the pitch in metres, used both to spawn and to
/// re-form the teams for a restart.
pub fn base_formation_position(id: PlayerId, position: PlayingPosition, sides: PitchSides) -> Vec2 {
    let home_position = match position {
        PlayingPosition::Goalkeeper => Vec2::new(-54.0, 0.0),
        PlayingPosition::LeftBack => Vec2::new(-35.0, -18.0),
        PlayingPosition::CentreBack => {
            if id.shirt == 3 {
                Vec2::new(-35.0, -6.0)
            } else {
                Vec2::new(-35.0, 6.0)
            }
        }
        PlayingPosition::RightBack => Vec2::new(-35.0, 18.0),
        PlayingPosition::LeftMidfielder => Vec2::new(-20.0, -20.0),
        PlayingPosition::CentreMidfielder => {
            if id.shirt == 7 {
                Vec2::new(-20.0, -7.0)
            } else {
                Vec2::new(-20.0, 7.0)
            }
        }
        PlayingPosition::RightMidfielder => Vec2::new(-20.0, 20.0),
        PlayingPosition::DefensiveMidfielder => Vec2::new(-25.0, 0.0),
        PlayingPosition::AttackingMidfielder => Vec2::new(-15.0, 0.0),
        PlayingPosition::CentreForward => {
            if id.shirt == 10 {
                Vec2::new(-7.0, -10.0)
            } else {
                Vec2::new(-7.0, 10.0)
            }
        }
    };

    // La plantilla se escribe defendiendo a la izquierda; el equipo que defiende
    // a la derecha la ve rotada media vuelta. Con esto, cambiar de mitad en el
    // descanso no necesita otra tabla de posiciones.
    if sides.defended_by(id.team) == TeamSide::Left {
        home_position
    } else {
        Vec2::new(-home_position.x, -home_position.y)
    }
}
