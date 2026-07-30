//! Filling the snapshot from the authoritative state.
//!
//! The snapshot itself is domain vocabulary, so presentation can draw it
//! without knowing this crate exists. What belongs here is the reading: the
//! state and the ledger are the kernel's.

use crate::diagnostics::ledger::MatchLedger;
use bevy_ecs::prelude::*;
use football_domain::diagnostics::{Field, MatchSnapshot, ReleaseKind, SectionId};
use football_domain::{
    MatchPhase, MatchRegulations, MatchState, PossessionDesignation, SetPiece, TeamId,
};
use std::time::Duration;

/// The time a broadcast clock would show: minutes counted from the start of the
/// match, not of the period, so the second half opens at 45:00.
pub fn displayed_clock(
    phase: MatchPhase,
    period_elapsed: Duration,
    regulations: &MatchRegulations,
) -> Duration {
    match phase {
        MatchPhase::PreMatch => Duration::ZERO,
        MatchPhase::FirstHalf => period_elapsed,
        // during the interval the clock rests on the half-time whistle
        MatchPhase::HalfTime => regulations.half_duration,
        MatchPhase::SecondHalf => regulations.half_duration + period_elapsed,
        MatchPhase::FullTime => regulations.half_duration * 2,
        // extra time is not modelled yet; keep the clock honest rather than
        // invent a number for it
        MatchPhase::FirstExtraTime | MatchPhase::SecondExtraTime | MatchPhase::Penalties => {
            regulations.half_duration * 2 + period_elapsed
        }
    }
}

pub fn format_clock(elapsed: Duration) -> String {
    let total_seconds = elapsed.as_secs();
    format!("{:02}:{:02}", total_seconds / 60, total_seconds % 60)
}

pub fn phase_label(phase: MatchPhase) -> &'static str {
    match phase {
        MatchPhase::PreMatch => "Before kick-off",
        MatchPhase::FirstHalf => "First half",
        MatchPhase::HalfTime => "Half time",
        MatchPhase::SecondHalf => "Second half",
        MatchPhase::FirstExtraTime => "First half of extra time",
        MatchPhase::SecondExtraTime => "Second half of extra time",
        MatchPhase::Penalties => "Kicks from the penalty mark",
        MatchPhase::FullTime => "Full time",
    }
}

/// What play is stopped for, or nothing when the ball is live.
pub fn restart_label(set_piece: SetPiece, team: Option<TeamId>) -> Option<String> {
    let awarded = match set_piece {
        SetPiece::None => return None,
        SetPiece::KickOff => "Kick-off",
        SetPiece::GoalKick => "Goal kick",
        SetPiece::FreeKick => "Free kick",
        SetPiece::Corner => "Corner",
        SetPiece::ThrowIn => "Throw-in",
        SetPiece::Penalty => "Penalty",
    };
    Some(match team {
        Some(TeamId::Home) => format!("{awarded}: home"),
        Some(TeamId::Away) => format!("{awarded}: away"),
        _ => awarded.to_string(),
    })
}

/// The match as a broadcast would state it: score, clock, and what part of the
/// match this is.
pub fn scoreboard_fields(match_state: &MatchState, regulations: &MatchRegulations) -> Vec<Field> {
    let elapsed = match_state.period_elapsed;
    vec![
        Field::new(
            "score",
            format!("{}-{}", match_state.home_score, match_state.away_score),
        ),
        Field::volatile(
            "clock",
            format_clock(displayed_clock(match_state.phase, elapsed, regulations)),
        ),
        Field::new("phase", phase_label(match_state.phase)),
    ]
}

/// Fills the snapshot from the authoritative state and the ledger. Reads only.
pub(super) fn collect_snapshot(
    mut snapshot: ResMut<MatchSnapshot>,
    match_state: Res<MatchState>,
    regulations: Res<MatchRegulations>,
    designation: Res<PossessionDesignation>,
    ledger: Res<MatchLedger>,
) {
    let elapsed = match_state.period_elapsed;
    snapshot.set(
        SectionId::Scoreboard,
        scoreboard_fields(&match_state, &regulations),
    );

    let holder = match match_state.possession_player {
        Some(player) => player.to_string(),
        None => "loose".to_string(),
    };
    snapshot.set(
        SectionId::Possession,
        vec![
            Field::new("holder", holder),
            Field::new(
                "designated",
                format!(
                    "{} / {}",
                    designated_name(designation.designated[TeamId::Home]),
                    designated_name(designation.designated[TeamId::Away])
                ),
            ),
            Field::new("changes", ledger.possession_changes.to_string()),
            Field::volatile(
                "per_min",
                format!("{:.1}", ledger.changes_per_minute(elapsed)),
            ),
            Field::new(
                "longest",
                format!("{:.1}s", ledger.longest_spell.as_secs_f32()),
            ),
        ],
    );

    snapshot.set(
        SectionId::Passing,
        vec![
            Field::new(
                "lost_passes",
                ledger.turnovers_of(ReleaseKind::Pass).to_string(),
            ),
            Field::new("at_receiver", ledger.pass_turnovers_near.to_string()),
            Field::new("en_route", ledger.pass_turnovers_far.to_string()),
            Field::new("touchers", ledger.distinct_touchers().to_string()),
        ],
    );

    match restart_label(match_state.set_piece, match_state.set_piece_team) {
        None => snapshot.clear(SectionId::Restart),
        Some(awarded) => snapshot.set(
            SectionId::Restart,
            vec![
                Field::new("awarded", awarded),
                Field::volatile(
                    "in",
                    format!("{:.1}s", match_state.restart_in.as_secs_f32()),
                ),
            ],
        ),
    }
}

fn designated_name(designated: Option<football_domain::PlayerId>) -> String {
    match designated {
        Some(player) => format!("#{}", player.shirt),
        None => "-".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use football_domain::SetPiece;

    fn short_regulations() -> MatchRegulations {
        MatchRegulations {
            half_duration: Duration::from_secs(45 * 60),
            half_time_interval: Duration::from_secs(15 * 60),
        }
    }

    #[test]
    fn the_second_half_clock_continues_from_forty_five_minutes() {
        let regulations = short_regulations();
        let shown = displayed_clock(
            MatchPhase::SecondHalf,
            Duration::from_secs(90),
            &regulations,
        );
        assert_eq!(format_clock(shown), "46:30");
    }

    #[test]
    fn the_clock_rests_on_the_half_time_whistle() {
        let regulations = short_regulations();
        let shown = displayed_clock(MatchPhase::HalfTime, Duration::from_secs(600), &regulations);
        assert_eq!(
            format_clock(shown),
            "45:00",
            "the interval must not run the match clock on"
        );
    }

    #[test]
    fn a_match_that_has_not_started_shows_zero() {
        let shown = displayed_clock(
            MatchPhase::PreMatch,
            Duration::from_secs(30),
            &short_regulations(),
        );
        assert_eq!(format_clock(shown), "00:00");
    }

    #[test]
    fn a_live_ball_has_no_restart_label() {
        assert_eq!(restart_label(SetPiece::None, Some(TeamId::Home)), None);
        assert_eq!(
            restart_label(SetPiece::ThrowIn, Some(TeamId::Away)).as_deref(),
            Some("Throw-in: away")
        );
    }

    /// Both sinks read this, so it is the one place that decides how a match
    /// states itself.
    #[test]
    fn the_scoreboard_states_score_clock_and_phase() {
        let state = MatchState {
            home_score: 2,
            away_score: 1,
            phase: MatchPhase::FirstHalf,
            period_elapsed: Duration::from_secs(61),
            ..Default::default()
        };

        let mut snapshot = MatchSnapshot::default();
        snapshot.set(
            SectionId::Scoreboard,
            scoreboard_fields(&state, &short_regulations()),
        );

        assert_eq!(
            snapshot.line(SectionId::Scoreboard).as_deref(),
            Some("match: score=2-1  clock=01:01  phase=First half")
        );
        assert_eq!(
            snapshot.stable_line(SectionId::Scoreboard).as_deref(),
            Some("match: score=2-1  phase=First half"),
            "a running clock must not count as a change"
        );
    }
}
