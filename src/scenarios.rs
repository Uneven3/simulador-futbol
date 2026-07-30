//! Catalogue of reproducible situations.
//!
//! Each one states an initial state, a window and what must happen, so a rule is
//! demonstrated rather than asserted. The rule-of-play scenarios deliberately
//! field no players: an incident that depends on nobody's behaviour is the
//! smallest thing that can prove a referee decision.

use bevy::math::Vec3;
use football_domain::scenario::{BallSetup, Expectations, PlayerSetup};
use football_domain::{ByTeam, MatchPhase, MatchRegulations, Scenario, SetPiece, TeamId};
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
                .last_touched_by(TeamId::Home),
        )
        .already_in_play()
        // 8 s of celebration plus the restart, per the original's timings
        .for_duration(Duration::from_secs(12))
        .expecting(Expectations {
            score: Some(ByTeam::new(1, 0)),
            set_pieces: vec![SetPiece::KickOff],
            play_resumes: true,
            ..Default::default()
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
                .last_touched_by(TeamId::Home),
        )
        .already_in_play()
        .for_duration(Duration::from_secs(8))
        .expecting(Expectations {
            score: Some(ByTeam::new(0, 0)),
            set_pieces: vec![SetPiece::ThrowIn],
            play_resumes: true,
            ..Default::default()
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
                .last_touched_by(TeamId::Home),
        )
        .already_in_play()
        .for_duration(Duration::from_secs(8))
        .expecting(Expectations {
            score: Some(ByTeam::new(0, 0)),
            set_pieces: vec![SetPiece::GoalKick],
            play_resumes: true,
            ..Default::default()
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
                .last_touched_by(TeamId::Home),
        )
        .already_in_play()
        .for_duration(Duration::from_secs(8))
        .expecting(Expectations {
            score: Some(ByTeam::new(0, 0)),
            set_pieces: vec![SetPiece::Corner],
            play_resumes: true,
            ..Default::default()
        })
}

/// Law 7: a whole match, from before kick-off to full time, with the periods
/// shortened to seconds.
///
/// Shortening them is the point of holding regulation lengths as competition
/// data: the phases are the same ones a ninety-minute match goes through.
pub fn short_match() -> Scenario {
    Scenario::kick_off()
        .named("short match")
        .with_regulations(MatchRegulations {
            half_duration: Duration::from_secs(20),
            half_time_interval: Duration::from_secs(3),
        })
        .for_duration(Duration::from_secs(50))
        .expecting(Expectations {
            phases: vec![
                MatchPhase::FirstHalf,
                MatchPhase::HalfTime,
                MatchPhase::SecondHalf,
                MatchPhase::FullTime,
            ],
            play_resumes: true,
            ..Default::default()
        })
}

/// Law 9 at speed: a shot travelling 0.4 m per tick must not tunnel through the
/// goal line between two ticks. The referee sweeps the segment for this reason.
pub fn goal_at_high_speed() -> Scenario {
    Scenario::kick_off()
        .named("goal at high speed")
        .with_players(PlayerSetup::BallOnly)
        .with_ball(
            BallSetup::travelling_from(Vec3::new(45.0, 1.0, 1.0), Vec3::new(40.0, 0.5, 0.0))
                .last_touched_by(TeamId::Home),
        )
        .already_in_play()
        .for_duration(Duration::from_secs(12))
        .expecting(Expectations {
            score: Some(ByTeam::new(1, 0)),
            set_pieces: vec![SetPiece::KickOff],
            play_resumes: true,
            ..Default::default()
        })
}

/// Law 10: hitting the crossbar is not a goal, however close it looks.
pub fn shot_off_the_crossbar() -> Scenario {
    Scenario::kick_off()
        .named("shot off the crossbar")
        .with_players(PlayerSetup::BallOnly)
        .with_ball(
            BallSetup::travelling_from(Vec3::new(50.0, 0.0, 2.4), Vec3::new(22.0, 0.0, 1.5))
                .last_touched_by(TeamId::Home),
        )
        .already_in_play()
        .for_duration(Duration::from_secs(6))
        .expecting(Expectations {
            score: Some(ByTeam::new(0, 0)),
            ..Default::default()
        })
}

/// Law 10: hitting the post is not a goal either — the ball comes back into
/// play instead.
///
/// The shot is aimed straight down the axis of the post, which is the only
/// aim that is unambiguously woodwork: a hand's breadth to either side and the
/// question becomes whether the ball was inside or outside the post, which is a
/// different claim (and the one `shot_crossing_the_goal_line` already makes).
pub fn shot_off_the_post() -> Scenario {
    Scenario::kick_off()
        .named("shot off the post")
        .with_players(PlayerSetup::BallOnly)
        .with_ball(
            BallSetup::travelling_from(Vec3::new(52.0, 3.7, 0.5), Vec3::new(10.0, 0.0, 0.0))
                .last_touched_by(TeamId::Home),
        )
        .already_in_play()
        .for_duration(Duration::from_secs(6))
        .expecting(Expectations {
            score: Some(ByTeam::new(0, 0)),
            play_never_stops: true,
            ..Default::default()
        })
}

/// Law 9 and 10: the ball must pass WHOLLY over the line. A ball that stops on
/// the line is neither a goal nor out of play — it is still live.
pub fn ball_stopping_on_the_goal_line() -> Scenario {
    Scenario::kick_off()
        .named("ball stopping on the goal line")
        .with_players(PlayerSetup::BallOnly)
        .with_ball(
            BallSetup::travelling_from(Vec3::new(54.9, 0.0, 0.11), Vec3::new(0.7, 0.0, 0.0))
                .last_touched_by(TeamId::Home),
        )
        .already_in_play()
        .for_duration(Duration::from_secs(6))
        .expecting(Expectations {
            score: Some(ByTeam::new(0, 0)),
            play_never_stops: true,
            ..Default::default()
        })
}

/// Every scenario in the catalogue, for suites that run them all.
pub fn all() -> Vec<Scenario> {
    vec![
        opening_minute(),
        short_match(),
        shot_crossing_the_goal_line(),
        goal_at_high_speed(),
        shot_off_the_crossbar(),
        shot_off_the_post(),
        ball_stopping_on_the_goal_line(),
        ball_over_the_touchline(),
        ball_over_the_opponents_goal_line(),
        ball_over_own_goal_line(),
    ]
}
