//! Each rule of play, demonstrated by running a situation to its end.
//!
//! The scenario carries its own claims, so a test here is a name plus a run: if
//! the referee stops deciding correctly, the scenario says so in its own terms.

use gameplayfootball::{MatchPhase, ScenarioRunner, scenarios};

#[test]
fn a_shot_over_the_goal_line_is_a_goal() {
    ScenarioRunner::headless(scenarios::shot_crossing_the_goal_line()).assert_scenario_holds();
}

#[test]
fn a_ball_over_the_touchline_is_a_throw_in() {
    ScenarioRunner::headless(scenarios::ball_over_the_touchline()).assert_scenario_holds();
}

#[test]
fn a_ball_over_the_opponents_goal_line_is_a_goal_kick() {
    ScenarioRunner::headless(scenarios::ball_over_the_opponents_goal_line())
        .assert_scenario_holds();
}

#[test]
fn a_ball_over_your_own_goal_line_is_a_corner() {
    ScenarioRunner::headless(scenarios::ball_over_own_goal_line()).assert_scenario_holds();
}

#[test]
fn play_restarts_after_the_opening_whistle() {
    ScenarioRunner::headless(scenarios::opening_minute()).assert_scenario_holds();
}

#[test]
fn a_match_runs_from_kick_off_to_full_time() {
    ScenarioRunner::headless(scenarios::short_match()).assert_scenario_holds();
}

#[test]
fn a_shot_at_speed_does_not_tunnel_through_the_goal_line() {
    ScenarioRunner::headless(scenarios::goal_at_high_speed()).assert_scenario_holds();
}

#[test]
fn hitting_the_woodwork_is_not_a_goal() {
    ScenarioRunner::headless(scenarios::shot_off_the_crossbar()).assert_scenario_holds();
    ScenarioRunner::headless(scenarios::shot_off_the_post()).assert_scenario_holds();
}

#[test]
fn a_ball_stopping_on_the_line_is_still_in_play() {
    ScenarioRunner::headless(scenarios::ball_stopping_on_the_goal_line()).assert_scenario_holds();
}

/// The clock must move, and it must stop when the match does.
#[test]
fn the_clock_runs_during_play_and_stops_at_full_time() {
    let scenario = scenarios::short_match();
    let half = scenario.regulations.half_duration;
    let outcome = ScenarioRunner::headless(scenario).run();

    assert_eq!(
        outcome.final_phase,
        MatchPhase::FullTime,
        "the match should have finished within the window"
    );
    assert!(
        outcome.period_elapsed >= half,
        "the second half ran {:?}, less than its {half:?}",
        outcome.period_elapsed
    );
}

/// Law 11 of this project's architecture, not of football: the same scenario and
/// seed must produce the same match, or nothing measured on it means anything.
#[test]
fn a_scenario_replays_identically() {
    let first = ScenarioRunner::headless(scenarios::opening_minute()).run();
    let second = ScenarioRunner::headless(scenarios::opening_minute()).run();

    assert_eq!(
        first.score, second.score,
        "the same seed produced different scores"
    );
    assert_eq!(
        first.set_pieces, second.set_pieces,
        "the same seed produced a different sequence of restarts"
    );
}

/// Every catalogued scenario holds. This is the suite that will grow as IFAB
/// coverage does, so a new rule arrives with the situation that proves it.
#[test]
fn the_whole_catalogue_holds() {
    for scenario in scenarios::all() {
        let name = scenario.name.clone();
        let expectations = scenario.expectations.clone();
        let outcome = ScenarioRunner::headless(scenario).run();
        let mismatches = outcome.mismatches(&expectations);
        assert!(
            mismatches.is_empty(),
            "scenario '{name}' did not hold:\n  - {}",
            mismatches.join("\n  - ")
        );
    }
}
