//! Each rule of play, demonstrated by running a situation to its end.
//!
//! The scenario carries its own claims, so a test here is a name plus a run: if
//! the referee stops deciding correctly, the scenario says so in its own terms.

use gameplayfootball::{ScenarioRunner, scenarios};

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
