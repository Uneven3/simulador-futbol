use crate::SimulationSet;
use crate::diagnostics::{MatchFact, MatchTelemetry};
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_time::prelude::*;
use football_domain::TeamId;
use football_domain::{MatchPhase, MatchRegulations, MatchState, Player, SetPiece, Velocity};

/// Law 7: the clock and the phases of a match.
///
/// The clock owns every phase transition, and it is the first thing to run in a
/// tick — the match lifecycle decides whether there is a match to simulate at
/// all before anyone moves.
pub struct MatchClockPlugin;

impl Plugin for MatchClockPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (advance_match_clock, still_the_players_at_full_time)
                .chain()
                .in_set(SimulationSet::MatchLifecycle),
        );
    }
}

/// Time runs while a period is being played, including while play is stopped
/// for a restart — as it does in a real match. What is missing is the allowance
/// for time lost, which the referee adds at the end of each period; without it a
/// half here is exactly its regulation length.
fn advance_match_clock(
    time: Res<Time>,
    regulations: Res<MatchRegulations>,
    mut match_state: ResMut<MatchState>,
    mut telemetry: ResMut<MatchTelemetry>,
) {
    match match_state.phase {
        // The match begins when the referee first sets it in motion.
        MatchPhase::PreMatch => {
            if match_state.set_piece == SetPiece::None {
                match_state.phase = MatchPhase::FirstHalf;
                match_state.period_elapsed_ms = 0;
                telemetry.record(MatchFact::PhaseEntered(MatchPhase::FirstHalf));
            }
        }

        MatchPhase::FirstHalf => {
            let elapsed = advance_period(&time, &mut match_state);
            if elapsed >= regulations.half_duration.as_millis() as u64 {
                match_state.phase = MatchPhase::HalfTime;
                match_state.period_elapsed_ms = 0;
                // Law 8: the other team kicks off the second half.
                let second_half_kick_off = match_state.opening_kick_off_team.opponent();
                stop_play_for_kick_off(
                    &mut match_state,
                    second_half_kick_off,
                    regulations.half_time_interval.as_secs_f32(),
                );
                telemetry.record(MatchFact::PhaseEntered(MatchPhase::HalfTime));
            }
        }

        // The interval is not playing time; the second half starts when the
        // referee restarts play.
        MatchPhase::HalfTime => {
            if match_state.set_piece == SetPiece::None {
                match_state.phase = MatchPhase::SecondHalf;
                match_state.period_elapsed_ms = 0;
                telemetry.record(MatchFact::PhaseEntered(MatchPhase::SecondHalf));
            }
        }

        MatchPhase::SecondHalf => {
            let elapsed = advance_period(&time, &mut match_state);
            if elapsed >= regulations.half_duration.as_millis() as u64 {
                match_state.phase = MatchPhase::FullTime;
                // Play stops for good: the referee never restarts after this,
                // so the pending kick-off is one that will never be taken.
                let kicking_team = match_state.opening_kick_off_team;
                stop_play_for_kick_off(&mut match_state, kicking_team, 0.0);
                telemetry.record(MatchFact::PhaseEntered(MatchPhase::FullTime));
            }
        }

        // Extra time and kicks from the penalty mark belong to MVP 2; a match
        // that reaches full time simply stays there.
        MatchPhase::FirstExtraTime
        | MatchPhase::SecondExtraTime
        | MatchPhase::Penalties
        | MatchPhase::FullTime => {}
    }
}

/// At the final whistle the players stop, and their state has to say so.
///
/// Their decision systems have already stopped running, so nobody would move
/// either way — but a body left carrying 8 m/s is a lie the diagnostics would
/// faithfully draw: twenty-two velocity arrows on a pitch where the match is
/// over.
fn still_the_players_at_full_time(
    match_state: Res<MatchState>,
    mut players: Query<&mut Velocity, With<Player>>,
) {
    if !match_state.phase.is_over() {
        return;
    }
    for mut velocity in players.iter_mut() {
        if velocity.0 != Vec3::ZERO {
            velocity.0 = Vec3::ZERO;
        }
    }
}

fn advance_period(time: &Time, match_state: &mut MatchState) -> u64 {
    let tick_ms = (time.delta_secs_f64() * 1000.0).round() as u64;
    match_state.period_elapsed_ms += tick_ms;
    match_state.period_elapsed_ms
}

fn stop_play_for_kick_off(match_state: &mut MatchState, kicking_team: TeamId, delay_seconds: f32) {
    match_state.set_piece = SetPiece::KickOff;
    match_state.set_piece_team = Some(kicking_team);
    match_state.set_piece_timer = delay_seconds;
    match_state.restart_pos = Vec3::ZERO;
    match_state.possession_player = None;
    match_state.possession_team = None;
    match_state.pass_target = None;
}
