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

/// Half a metre wide of the post is not a goal, and the scenario that proves
/// the goal must be told apart from the one that proves the miss.
#[test]
fn a_shot_into_the_side_netting_is_not_a_goal() {
    ScenarioRunner::headless(scenarios::shot_into_the_side_netting()).assert_scenario_holds();
}

/// A scenario is a claim, and a claim nobody could satisfy is worse than none:
/// it fails forever, or passes for the wrong reason.
#[test]
fn a_scenario_that_contradicts_itself_is_caught_before_it_runs() {
    use gameplayfootball::ByTeam;

    let impossible = scenarios::shot_crossing_the_goal_line().expecting(
        gameplayfootball::scenario::Expectations {
            score: Some(ByTeam::new(1, 0)),
            play_never_stops: true,
            ..Default::default()
        },
    );
    assert!(
        !impossible.contradictions().is_empty(),
        "a scenario expecting both a goal and uninterrupted play was accepted"
    );

    // The opening scenario describes a match, not a situation: asserting it
    // would simulate 540,000 ticks.
    assert!(
        !gameplayfootball::Scenario::kick_off()
            .contradictions()
            .is_empty(),
        "a 90-minute window was accepted into a suite"
    );

    for scenario in scenarios::all() {
        let name = scenario.name.clone();
        assert!(
            scenario.contradictions().is_empty(),
            "catalogued scenario '{name}' contradicts itself: {:?}",
            scenario.contradictions()
        );
    }
}

/// After the final whistle nobody plays on. The ball is still integrated — it
/// rolls to a stop, as it does when the whistle catches it — but no player
/// decides anything and the score cannot change.
#[test]
fn nobody_plays_on_after_the_final_whistle() {
    use bevy::prelude::{Vec3, With};
    use football_domain::{Player, Position, Velocity};

    let scenario = scenarios::short_match();
    let ticks = scenario.ticks();
    let mut runner = ScenarioRunner::headless(scenario);
    for _ in 0..ticks {
        runner.advance();
    }

    let world = runner.world_mut();
    assert_eq!(
        world.resource::<gameplayfootball::MatchState>().phase,
        MatchPhase::FullTime
    );

    let moving = world
        .query::<&Velocity>()
        .iter(world)
        .filter(|velocity| velocity.0.length() > 0.01)
        .count();
    assert_eq!(
        moving, 0,
        "{moving} players were still running after full time"
    );

    // And they stay where the whistle found them.
    let before: Vec<Vec3> = world
        .query_filtered::<&Position, With<Player>>()
        .iter(world)
        .map(|position| position.0)
        .collect();
    for _ in 0..100 {
        runner.advance();
    }
    let world = runner.world_mut();
    let after: Vec<Vec3> = world
        .query_filtered::<&Position, With<Player>>()
        .iter(world)
        .map(|position| position.0)
        .collect();
    assert_eq!(before, after, "a body moved after the match had ended");
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
