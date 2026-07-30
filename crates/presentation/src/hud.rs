//! The scoreboard: score, clock, phase and what the referee has stopped play
//! for. Read-only, like everything in this layer.

use bevy::prelude::*;
use football_domain::{MatchPhase, MatchRegulations, MatchState, SetPiece};
use std::time::Duration;

pub struct MatchHudPlugin;

impl Plugin for MatchHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_hud)
            .add_systems(Update, update_hud);
    }
}

#[derive(Component)]
struct HudText;

fn spawn_hud(mut commands: Commands) {
    commands.spawn((
        Name::new("Match HUD"),
        HudText,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(20.0),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        },
    ));
}

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
pub fn restart_label(set_piece: SetPiece, team_index: Option<u32>) -> Option<String> {
    let awarded = match set_piece {
        SetPiece::None => return None,
        SetPiece::KickOff => "Kick-off",
        SetPiece::GoalKick => "Goal kick",
        SetPiece::FreeKick => "Free kick",
        SetPiece::Corner => "Corner",
        SetPiece::ThrowIn => "Throw-in",
        SetPiece::Penalty => "Penalty",
    };
    Some(match team_index {
        Some(0) => format!("{awarded}: home"),
        Some(1) => format!("{awarded}: away"),
        _ => awarded.to_string(),
    })
}

pub fn hud_line(match_state: &MatchState, regulations: &MatchRegulations) -> String {
    let clock = displayed_clock(
        match_state.phase,
        Duration::from_millis(match_state.period_elapsed_ms),
        regulations,
    );
    let mut line = format!(
        "HOME {} - {} AWAY   {}   {}",
        match_state.home_score,
        match_state.away_score,
        format_clock(clock),
        phase_label(match_state.phase),
    );
    if let Some(restart) = restart_label(match_state.set_piece, match_state.set_piece_team) {
        line.push_str("   ");
        line.push_str(&restart);
    }
    line
}

fn update_hud(
    match_state: Res<MatchState>,
    regulations: Res<MatchRegulations>,
    mut hud: Query<&mut Text, With<HudText>>,
) {
    if !match_state.is_changed() {
        return;
    }
    let line = hud_line(&match_state, &regulations);
    for mut text in hud.iter_mut() {
        text.0 = line.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(restart_label(SetPiece::None, Some(0)), None);
        assert_eq!(
            restart_label(SetPiece::ThrowIn, Some(1)).as_deref(),
            Some("Throw-in: away")
        );
    }

    #[test]
    fn the_hud_line_reads_score_clock_phase_and_restart() {
        let state = MatchState {
            home_score: 2,
            away_score: 1,
            phase: MatchPhase::FirstHalf,
            period_elapsed_ms: 61_000,
            set_piece: SetPiece::Corner,
            set_piece_team: Some(0),
            ..Default::default()
        };

        assert_eq!(
            hud_line(&state, &short_regulations()),
            "HOME 2 - 1 AWAY   01:01   First half   Corner: home"
        );
    }
}
