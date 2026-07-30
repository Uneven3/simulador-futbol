use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_time::prelude::*;
use football_domain::scenario::{PlayState, PlayerSetup, SIMULATION_HZ, Scenario};
use football_domain::{
    Ball, BallTouched, Facing, MatchRng, MatchState, Player, PlayerRole, PlayerStats, Position,
    SetPiece, Velocity,
};

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
            .insert_resource(MatchRng::seeded(self.scenario.seed))
            .insert_resource(Time::<Fixed>::from_hz(SIMULATION_HZ))
            .add_message::<BallTouched>()
            .add_systems(Startup, (spawn_scenario_ball, spawn_scenario_players));
    }
}

/// The match state a scenario opens in: stopped awaiting a restart, or live.
fn initial_match_state(scenario: &Scenario) -> MatchState {
    let mut state = MatchState::default();
    match scenario.play_state {
        PlayState::AwaitingRestart {
            set_piece,
            team_index,
            delay,
        } => {
            state.set_piece = set_piece;
            state.set_piece_team = Some(team_index);
            state.set_piece_timer = delay.as_secs_f32();
            state.restart_pos = Vec3::ZERO;
            state.opening_kick_off_team = team_index;
        }
        PlayState::InPlay => {
            state.set_piece = SetPiece::None;
            state.set_piece_team = None;
            state.set_piece_timer = 0.0;
        }
    }
    state
}

/// Height of an outfield body in metres. Provisional until anthropometry
/// becomes per-player data (MVP 3).
pub const PLAYER_HEIGHT: f32 = 1.8;

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

    const ROLES: [PlayerRole; 11] = [
        PlayerRole::GK,
        PlayerRole::LB,
        PlayerRole::CB,
        PlayerRole::CB,
        PlayerRole::RB,
        PlayerRole::LM,
        PlayerRole::CM,
        PlayerRole::CM,
        PlayerRole::RM,
        PlayerRole::CF,
        PlayerRole::CF,
    ];

    for team_index in 0u32..2 {
        // home defends -x and therefore faces +x
        let attacking_direction = if team_index == 0 {
            Dir2::X
        } else {
            Dir2::NEG_X
        };
        let team_name = if team_index == 0 { "Home" } else { "Away" };

        for (index, role) in ROLES.iter().enumerate() {
            let base = base_formation_position(team_index, *role, index as u32);
            commands.spawn((
                Name::new(format!("{} Player {} - {:?}", team_name, index + 1, role)),
                Player {
                    id: index as u32,
                    team_index,
                    jersey_number: (index + 1) as u32,
                    role: *role,
                    height: PLAYER_HEIGHT,
                    last_touch_time_ms: 0,
                    formation_pos: normalized_formation_position(*role, index as u32),
                    man_marking: None,
                    avg_velocity: 0.0,
                },
                PlayerStats::default(),
                Velocity::default(),
                Facing(attacking_direction),
                Position::from_pitch(base, 0.0),
            ));
        }
    }
}

/// Normalized formation entry (original `FormationEntry::position`, -1..1 in
/// both axes) for the default 4-4-2; `AI_GetAdaptedFormationPosition` scales
/// this into the dynamic team block.
pub fn normalized_formation_position(role: PlayerRole, player_id: u32) -> Vec2 {
    match role {
        PlayerRole::GK => Vec2::new(-1.0, 0.0),
        PlayerRole::LB => Vec2::new(-1.0, -1.0),
        PlayerRole::CB => Vec2::new(-1.0, if player_id == 2 { -0.33 } else { 0.33 }),
        PlayerRole::RB => Vec2::new(-1.0, 1.0),
        PlayerRole::LM => Vec2::new(0.0, -1.0),
        PlayerRole::CM => Vec2::new(0.0, if player_id == 6 { -0.33 } else { 0.33 }),
        PlayerRole::RM => Vec2::new(0.0, 1.0),
        PlayerRole::DM => Vec2::new(-0.5, 0.0),
        PlayerRole::AM => Vec2::new(0.5, 0.0),
        PlayerRole::CF => Vec2::new(1.0, if player_id == 9 { -0.4 } else { 0.4 }),
    }
}

/// Base (kick-off) position on the pitch in metres, used both to spawn and to
/// re-form the teams for a restart.
pub fn base_formation_position(team_index: u32, role: PlayerRole, player_id: u32) -> Vec2 {
    let home_position = match role {
        PlayerRole::GK => Vec2::new(-54.0, 0.0),
        PlayerRole::LB => Vec2::new(-35.0, -18.0),
        PlayerRole::CB => {
            if player_id == 2 {
                Vec2::new(-35.0, -6.0)
            } else {
                Vec2::new(-35.0, 6.0)
            }
        }
        PlayerRole::RB => Vec2::new(-35.0, 18.0),
        PlayerRole::LM => Vec2::new(-20.0, -20.0),
        PlayerRole::CM => {
            if player_id == 6 {
                Vec2::new(-20.0, -7.0)
            } else {
                Vec2::new(-20.0, 7.0)
            }
        }
        PlayerRole::RM => Vec2::new(-20.0, 20.0),
        PlayerRole::DM => Vec2::new(-25.0, 0.0),
        PlayerRole::AM => Vec2::new(-15.0, 0.0),
        PlayerRole::CF => {
            if player_id == 9 {
                Vec2::new(-7.0, -10.0)
            } else {
                Vec2::new(-7.0, 10.0)
            }
        }
    };

    if team_index == 0 {
        home_position
    } else {
        Vec2::new(-home_position.x, -home_position.y) // mirrored for the away team
    }
}
