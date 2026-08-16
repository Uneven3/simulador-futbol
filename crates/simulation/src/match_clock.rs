use crate::SimulationSet;
use crate::diagnostics::{MatchFact, MatchTelemetry};
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_time::prelude::*;
use football_domain::TeamId;
use football_domain::{
    MatchPhase, MatchRegulations, MatchRng, MatchState, MovementIntent, PenaltyShootout, Player,
    SetPiece, Velocity,
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
            (
                advance_match_clock,
                resolve_kicks_from_the_penalty_mark,
                still_the_players_at_full_time,
            )
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
                if match_state.home_score == match_state.away_score
                    && regulations.extra_time_half_duration.is_some()
                {
                    let kicking_team = match_state.opening_kick_off_team;
                    enter_extra_time_half(&mut match_state, kicking_team, &mut telemetry);
                } else {
                    finish_match(&mut match_state, &mut telemetry);
                }
            }
        }

        MatchPhase::FirstExtraTime => {
            let elapsed = advance_period(&time, &mut match_state);
            if let Some(duration) = regulations.extra_time_half_duration
                && period_is_over(elapsed, match_state.stoppage_elapsed, duration)
            {
                match_state.phase = MatchPhase::SecondExtraTime;
                match_state.period_elapsed = Duration::ZERO;
                match_state.stoppage_elapsed = Duration::ZERO;
                match_state.sides = match_state.sides.swapped();
                let kicking_team = match_state.opening_kick_off_team.opponent();
                stop_play_for_kick_off(
                    &mut match_state,
                    kicking_team,
                    regulations.extra_time_interval,
                );
                telemetry.record(MatchFact::PhaseEntered(MatchPhase::SecondExtraTime));
            }
        }

        MatchPhase::SecondExtraTime => {
            let elapsed = advance_period(&time, &mut match_state);
            if let Some(duration) = regulations.extra_time_half_duration
                && period_is_over(elapsed, match_state.stoppage_elapsed, duration)
            {
                // La tanda es una fase distinta, no un empate escondido tras
                // `FullTime`; su resolución se instala por el árbitro.
                if match_state.home_score == match_state.away_score
                    && regulations.kicks_from_penalty_mark_if_draw
                {
                    match_state.phase = MatchPhase::Penalties;
                    match_state.set_piece = SetPiece::None;
                    match_state.set_piece_team = None;
                    match_state.lose_possession_to_a_loose_ball();
                    match_state.penalty_shootout =
                        Some(PenaltyShootout::new(match_state.opening_kick_off_team));
                    telemetry.record(MatchFact::PhaseEntered(MatchPhase::Penalties));
                } else {
                    finish_match(&mut match_state, &mut telemetry);
                }
            }
        }

        MatchPhase::Penalties | MatchPhase::FullTime => {}
    }
}

/// A shoot-out is kept out of open-play systems: each tick resolves one
/// explicitly recorded kick, alternates sides and stops as soon as Law 10 says
/// the other side cannot catch up. It is intentionally a compact competition
/// mechanism until individual penalty-taking profiles are introduced.
fn resolve_kicks_from_the_penalty_mark(
    mut match_state: ResMut<MatchState>,
    regulations: Res<MatchRegulations>,
    mut rng: ResMut<MatchRng>,
    mut telemetry: ResMut<MatchTelemetry>,
) {
    if match_state.phase != MatchPhase::Penalties {
        return;
    }
    let Some(mut shootout) = match_state.penalty_shootout.take() else {
        return;
    };
    let team = shootout.next_team;
    shootout.taken[team] = shootout.taken[team].saturating_add(1);
    if rng.range(0.0, 1.0) < regulations.shootout_conversion_probability {
        shootout.scored[team] = shootout.scored[team].saturating_add(1);
    }
    shootout.next_team = team.opponent();

    let shootout_is_over = shootout.winner().is_some();
    match_state.penalty_shootout = Some(shootout);
    if shootout_is_over {
        finish_match(&mut match_state, &mut telemetry);
    }
}

fn enter_extra_time_half(
    match_state: &mut MatchState,
    kicking_team: TeamId,
    telemetry: &mut MatchTelemetry,
) {
    match_state.extra_time_started = true;
    match_state.phase = MatchPhase::FirstExtraTime;
    match_state.period_elapsed = Duration::ZERO;
    match_state.stoppage_elapsed = Duration::ZERO;
    stop_play_for_kick_off(match_state, kicking_team, Duration::ZERO);
    telemetry.record(MatchFact::PhaseEntered(MatchPhase::FirstExtraTime));
}

fn finish_match(match_state: &mut MatchState, telemetry: &mut MatchTelemetry) {
    match_state.end_match();
    telemetry.record(MatchFact::PhaseEntered(MatchPhase::FullTime));
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

    #[test]
    fn extra_time_is_opt_in_competition_data() {
        let ordinary = MatchRegulations::default();
        assert_eq!(ordinary.extra_time_half_duration, None);
        assert!(!ordinary.kicks_from_penalty_mark_if_draw);

        let knockout = MatchRegulations {
            extra_time_half_duration: Some(Duration::from_secs(15 * 60)),
            kicks_from_penalty_mark_if_draw: true,
            ..ordinary
        };
        assert_eq!(
            knockout.extra_time_half_duration,
            Some(Duration::from_secs(900))
        );
        assert!(knockout.kicks_from_penalty_mark_if_draw);
    }

    #[test]
    fn shootout_ends_early_or_in_sudden_death_only_when_legal() {
        let mut early = PenaltyShootout::new(TeamId::Home);
        early.taken = football_domain::ByTeam::new(3, 2);
        early.scored = football_domain::ByTeam::new(0, 3);
        assert_eq!(early.winner(), Some(TeamId::Away));

        let mut sudden_death = PenaltyShootout::new(TeamId::Home);
        sudden_death.taken = football_domain::ByTeam::new(6, 6);
        sudden_death.scored = football_domain::ByTeam::new(5, 6);
        assert_eq!(sudden_death.winner(), Some(TeamId::Away));
    }

    #[test]
    fn final_whistle_clears_play_state_and_reports_full_time() {
        let mut state = MatchState {
            phase: MatchPhase::SecondHalf,
            set_piece: SetPiece::KickOff,
            set_piece_team: Some(TeamId::Home),
            restart_in: Duration::from_secs(3),
            restart_taker: Some(football_domain::PlayerId::home(9)),
            possession_player: Some(football_domain::PlayerId::home(9)),
            possession_team: Some(TeamId::Home),
            penalty_shootout: Some(PenaltyShootout::new(TeamId::Away)),
            ..Default::default()
        };
        let mut telemetry = MatchTelemetry::default();

        finish_match(&mut state, &mut telemetry);

        assert_eq!(state.phase, MatchPhase::FullTime);
        assert_eq!(state.set_piece, SetPiece::None);
        assert_eq!(state.set_piece_team, None);
        assert_eq!(state.restart_in, Duration::ZERO);
        assert_eq!(state.restart_taker, None);
        assert_eq!(state.possession_player, None);
        assert_eq!(state.possession_team, None);
        assert_eq!(state.penalty_shootout, None);
        assert_eq!(
            telemetry.this_tick(),
            &[MatchFact::PhaseEntered(MatchPhase::FullTime)]
        );
    }
}
