use crate::SimulationSet;
use crate::diagnostics::{MatchFact, MatchTelemetry};
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_time::prelude::*;
use football_domain::TeamId;
use football_domain::{
    MatchPhase, MatchRegulations, MatchState, MovementIntent, Player, SetPiece, Velocity,
};
use std::time::Duration;

/// Law 7: the clock and the phases of a match. It owns every phase transition
/// and runs first in a tick — whether there is a match to simulate at all is
/// decided before anyone moves.
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

/// Milliseconds since the app started: what cooldowns and timestamps outside the
/// match clock are measured against. Saturating, because the alternative to 584
/// million years of match is a panic.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "clamped into u64's range on the line above the cast"
)]
pub fn engine_elapsed_ms(time: &Time) -> u64 {
    let millis = (time.elapsed_secs_f64() * 1000.0).clamp(0.0, u64::MAX as f64);
    millis as u64
}

/// Un periodo se acaba cuando se ha jugado lo que dura, y lo que se pasó parado
/// no se jugó (Ley 7). El reloj no se detiene en las reanudaciones, así que ese
/// tiempo vuelve alargando el periodo: el añadido del árbitro.
pub fn period_is_over(elapsed: Duration, stopped_for: Duration, regulation: Duration) -> bool {
    elapsed >= regulation + stopped_for
}

/// Time runs while a period is being played, including while play is stopped
/// for a restart — as it does in a real match, and the time lost comes back as
/// added time at the end of the period.
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
                match_state.period_elapsed = Duration::ZERO;
                match_state.stoppage_elapsed = Duration::ZERO;
                telemetry.record(MatchFact::PhaseEntered(MatchPhase::FirstHalf));
            }
        }

        MatchPhase::FirstHalf => {
            let elapsed = advance_period(&time, &mut match_state);
            if period_is_over(
                elapsed,
                match_state.stoppage_elapsed,
                regulations.half_duration,
            ) {
                match_state.phase = MatchPhase::HalfTime;
                match_state.period_elapsed = Duration::ZERO;
                // Law 8: the teams change ends and the other one kicks off.
                match_state.sides = match_state.sides.swapped();
                let second_half_kick_off = match_state.opening_kick_off_team.opponent();
                stop_play_for_kick_off(
                    &mut match_state,
                    second_half_kick_off,
                    regulations.half_time_interval,
                );
                telemetry.record(MatchFact::PhaseEntered(MatchPhase::HalfTime));
            }
        }

        // The interval is not playing time; the second half starts when the
        // referee restarts play.
        MatchPhase::HalfTime => {
            if match_state.set_piece == SetPiece::None {
                match_state.phase = MatchPhase::SecondHalf;
                match_state.period_elapsed = Duration::ZERO;
                match_state.stoppage_elapsed = Duration::ZERO;
                telemetry.record(MatchFact::PhaseEntered(MatchPhase::SecondHalf));
            }
        }

        MatchPhase::SecondHalf => {
            let elapsed = advance_period(&time, &mut match_state);
            if period_is_over(
                elapsed,
                match_state.stoppage_elapsed,
                regulations.half_duration,
            ) {
                match_state.phase = MatchPhase::FullTime;
                // Play stops for good: the referee never restarts after this,
                // so the pending kick-off is one that will never be taken.
                let kicking_team = match_state.opening_kick_off_team;
                stop_play_for_kick_off(&mut match_state, kicking_team, Duration::ZERO);
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

/// At the final whistle the players stop, and their state has to say so. Nobody
/// would move either way, but a body left carrying 8 m/s is a lie the
/// diagnostics would faithfully draw.
fn still_the_players_at_full_time(
    match_state: Res<MatchState>,
    mut players: Query<(&mut Velocity, &mut MovementIntent), With<Player>>,
) {
    if !match_state.phase.is_over() {
        return;
    }
    // también lo pedido: el motor persigue el intent, y uno vivo los pondría a
    // correr otra vez con el partido acabado
    for (mut velocity, mut intent) in players.iter_mut() {
        if velocity.0 != Vec3::ZERO {
            velocity.0 = Vec3::ZERO;
        }
        if intent.0 != Vec3::ZERO {
            intent.0 = Vec3::ZERO;
        }
    }
}

fn advance_period(time: &Time, match_state: &mut MatchState) -> Duration {
    match_state.period_elapsed += time.delta();
    if match_state.set_piece != SetPiece::None {
        match_state.stoppage_elapsed += time.delta();
    }
    match_state.period_elapsed
}

fn stop_play_for_kick_off(match_state: &mut MatchState, kicking_team: TeamId, delay: Duration) {
    match_state.set_piece = SetPiece::KickOff;
    match_state.set_piece_team = Some(kicking_team);
    match_state.restart_in = delay;
    match_state.restart_pos = Vec3::ZERO;
    match_state.possession_player = None;
    match_state.possession_team = None;
    match_state.pass_target = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sin paradas, un periodo dura lo que dice el reglamento.
    #[test]
    fn a_period_without_stoppages_lasts_exactly_its_regulation() {
        let regulation = Duration::from_secs(45 * 60);

        assert!(!period_is_over(
            regulation - Duration::from_millis(10),
            Duration::ZERO,
            regulation
        ));
        assert!(period_is_over(regulation, Duration::ZERO, regulation));
    }

    /// Y con ellas dura lo que se jugó, no lo que marcó el reloj.
    #[test]
    fn time_lost_comes_back_at_the_end_of_the_period() {
        let regulation = Duration::from_secs(45 * 60);
        let stopped_for = Duration::from_secs(4 * 60);

        assert!(
            !period_is_over(regulation, stopped_for, regulation),
            "pitó el final con cuatro minutos sin jugar"
        );
        assert!(period_is_over(
            regulation + stopped_for,
            stopped_for,
            regulation
        ));
    }
}
