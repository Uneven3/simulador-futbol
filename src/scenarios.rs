//! Catalogue of reproducible situations.
//!
//! Each one states an initial state, a window and what must happen, so a rule is
//! demonstrated rather than asserted. The rule-of-play scenarios deliberately
//! field no players: an incident that depends on nobody's behaviour is the
//! smallest thing that can prove a referee decision.

use bevy::math::Vec3;
use football_domain::scenario::{BallSetup, Expectations, PlayerSetup};
use football_domain::{Scenario, SetPiece};
use std::time::Duration;

/// A full match from the opening whistle, both teams formed up.
pub fn kick_off() -> Scenario {
    Scenario::kick_off()
}

/// The first minutes of open play. Same situation as [`kick_off`], short enough
/// to run in a test.
pub fn opening_minute() -> Scenario {
    Scenario::kick_off()
        .named("opening minute")
        .for_duration(Duration::from_secs(60))
}

/// Law 10: the whole ball passes over the goal line between the posts and under
/// the crossbar, so a goal is awarded and play restarts with a kick-off.
pub fn shot_crossing_the_goal_line() -> Scenario {
    Scenario::kick_off()
        .named("shot crossing the goal line")
        .with_players(PlayerSetup::BallOnly)
        .with_ball(
            BallSetup::travelling_from(Vec3::new(50.0, 0.0, 0.6), Vec3::new(30.0, 0.0, 0.0))
                .last_touched_by(0),
        )
        .already_in_play()
        // 8 s of celebration plus the restart, per the original's timings
        .for_duration(Duration::from_secs(12))
        .expecting(Expectations {
            score: Some([1, 0]),
            set_pieces: vec![SetPiece::KickOff],
            play_resumes: true,
        })
}

/// Law 9: the whole ball crosses the touchline, so the throw-in goes to the
/// team that did not touch it last.
pub fn ball_over_the_touchline() -> Scenario {
    Scenario::kick_off()
        .named("ball over the touchline")
        .with_players(PlayerSetup::BallOnly)
        .with_ball(
            BallSetup::travelling_from(Vec3::new(0.0, 34.0, 0.11), Vec3::new(0.0, 12.0, 0.0))
                .last_touched_by(0),
        )
        .already_in_play()
        .for_duration(Duration::from_secs(8))
        .expecting(Expectations {
            score: Some([0, 0]),
            set_pieces: vec![SetPiece::ThrowIn],
            play_resumes: true,
        })
}

/// Law 16: home plays the ball over the opponents' goal line outside the goal,
/// so the defending side gets a goal kick.
pub fn ball_over_the_opponents_goal_line() -> Scenario {
    Scenario::kick_off()
        .named("ball over the opponents' goal line")
        .with_players(PlayerSetup::BallOnly)
        .with_ball(
            BallSetup::travelling_from(Vec3::new(52.0, 20.0, 0.11), Vec3::new(12.0, 0.0, 0.0))
                .last_touched_by(0),
        )
        .already_in_play()
        .for_duration(Duration::from_secs(8))
        .expecting(Expectations {
            score: Some([0, 0]),
            set_pieces: vec![SetPiece::GoalKick],
            play_resumes: true,
        })
}

/// Law 17: home puts the ball over its own goal line outside the goal, so the
/// attacking side gets a corner.
pub fn ball_over_own_goal_line() -> Scenario {
    Scenario::kick_off()
        .named("ball over own goal line")
        .with_players(PlayerSetup::BallOnly)
        .with_ball(
            BallSetup::travelling_from(Vec3::new(-52.0, 20.0, 0.11), Vec3::new(-12.0, 0.0, 0.0))
                .last_touched_by(0),
        )
        .already_in_play()
        .for_duration(Duration::from_secs(8))
        .expecting(Expectations {
            score: Some([0, 0]),
            set_pieces: vec![SetPiece::Corner],
            play_resumes: true,
        })
}

/// Every scenario in the catalogue, for suites that run them all.
pub fn all() -> Vec<Scenario> {
    vec![
        opening_minute(),
        shot_crossing_the_goal_line(),
        ball_over_the_touchline(),
        ball_over_the_opponents_goal_line(),
        ball_over_own_goal_line(),
    ]
}
