//! Each rule of play, demonstrated by running a situation to its end.
//!
//! The scenario carries its own claims, so a test here is a name plus a run: if
//! the referee stops deciding correctly, the scenario says so in its own terms.

use gameplayfootball::{MatchPhase, ScenarioRunner, scenarios};

/// Ley 8: en el descanso los equipos cambian de mitad, y con ellos todo lo que
/// cuelga de qué lado defiende cada uno.
///
/// Se afirma sobre el cuerpo del portero y no solo sobre el estado: que el
/// registro diga que cambiaron y que los once sigan colocados como en la
/// primera parte sería la forma de que esto pareciese hecho sin estarlo.
#[test]
fn the_teams_change_ends_at_the_interval() {
    use football_domain::{MatchState, PlayerId, PlayingPosition, Position, TeamSide};

    let mut runner = ScenarioRunner::headless(scenarios::short_match());

    let keeper_x = |runner: &mut ScenarioRunner| -> f32 {
        let world = runner.world_mut();
        let mut keepers = world.query::<(&Position, &football_domain::Player)>();
        keepers
            .iter(world)
            .find(|(_, player)| {
                player.id == PlayerId::home(1) && player.position == PlayingPosition::Goalkeeper
            })
            .map(|(position, _)| position.0.x)
            .expect("el local tiene portero")
    };
    let phase = |runner: &mut ScenarioRunner| runner.world_mut().resource::<MatchState>().phase;
    let home_defends = |runner: &mut ScenarioRunner| {
        runner
            .world_mut()
            .resource::<MatchState>()
            .sides
            .defended_by(football_domain::TeamId::Home)
    };

    let mut first_half_x = None;
    let mut second_half_x = None;
    for _ in 0..scenarios::short_match().ticks() {
        runner.advance();
        match phase(&mut runner) {
            MatchPhase::FirstHalf => first_half_x = Some(keeper_x(&mut runner)),
            MatchPhase::SecondHalf => second_half_x = Some(keeper_x(&mut runner)),
            MatchPhase::PreMatch
            | MatchPhase::HalfTime
            | MatchPhase::FirstExtraTime
            | MatchPhase::SecondExtraTime
            | MatchPhase::Penalties
            | MatchPhase::FullTime => {}
        }
    }

    assert_eq!(
        home_defends(&mut runner),
        TeamSide::Right,
        "no cambiaron de lado"
    );
    let (first, second) = (
        first_half_x.expect("hubo primera parte"),
        second_half_x.expect("hubo segunda parte"),
    );
    assert!(
        first < 0.0 && second > 0.0,
        "el portero local no cambió de portería: {first:.1} -> {second:.1}"
    );
}

/// Ley 8: en el saque de centro los rivales se quedan fuera del círculo y cada
/// equipo en su mitad.
///
/// Se mira el tick exacto en que el árbitro pone el balón en juego, que es
/// cuando la ley se cumple o no; un tick después ya están todos corriendo.
#[test]
fn at_a_kick_off_the_opponents_stay_out_of_the_centre_circle() {
    use football_domain::{Ball, MatchState, Player, Position, SetPiece};

    /// Ley 8: el radio del círculo central, en metros.
    const CENTRE_CIRCLE: f32 = 9.15;

    let mut runner = ScenarioRunner::headless(scenarios::opening_minute());

    let mut checked = false;
    for _ in 0..300 {
        // quién saca hay que leerlo antes: al ejecutar la reanudación, el
        // árbitro lo borra junto con el resto del estado del saque
        let (was_stopped, taking) = {
            let state = runner.world_mut().resource::<MatchState>();
            (state.set_piece != SetPiece::None, state.set_piece_team)
        };
        runner.advance();
        let world = runner.world_mut();
        let state = world.resource::<MatchState>();
        if !was_stopped || state.set_piece != SetPiece::None {
            continue;
        }
        let mut balls = world.query_filtered::<&Position, bevy::prelude::With<Ball>>();
        let ball = balls.single(world).expect("hay balón").on_pitch();
        let mut players = world.query::<(&Position, &Player)>();
        for (position, player) in players.iter(world) {
            let at = position.on_pitch();
            if Some(player.id.team) == taking {
                continue;
            }
            assert!(
                at.distance(ball) >= CENTRE_CIRCLE - 0.01,
                "{} saca desde dentro del círculo, a {:.2} m del balón",
                player.id,
                at.distance(ball)
            );
        }
        checked = true;
        break;
    }
    assert!(checked, "el saque de centro nunca llegó a ejecutarse");
}

/// Ley 7: lo que se pierde en reanudaciones se añade al final del periodo.
///
/// Se afirma contra el tiempo que el partido estuvo parado de verdad, no contra
/// una cifra escrita a mano: un añadido fijo cumpliría un test con un número y
/// se despegaría del juego en cuanto cambiara el ritmo de reanudaciones.
#[test]
fn a_period_lasts_longer_when_play_was_stopped() {
    use football_domain::MatchState;
    use std::time::Duration;

    let scenario = scenarios::interrupted_half();
    let regulation = scenario.regulations.half_duration;
    let mut runner = ScenarioRunner::headless(scenario.clone());

    let mut first_half_lasted = Duration::ZERO;
    let mut stopped_for = None;
    for _ in 0..scenario.ticks() {
        runner.advance();
        let state = runner.world_mut().resource::<MatchState>();
        match state.phase {
            MatchPhase::FirstHalf => first_half_lasted = state.period_elapsed,
            MatchPhase::HalfTime => {
                stopped_for.get_or_insert(state.stoppage_elapsed);
            }
            MatchPhase::PreMatch
            | MatchPhase::SecondHalf
            | MatchPhase::FirstExtraTime
            | MatchPhase::SecondExtraTime
            | MatchPhase::Penalties
            | MatchPhase::FullTime => {}
        }
    }

    let stopped_for = stopped_for.expect("el partido llegó al descanso");
    assert!(
        stopped_for > Duration::ZERO,
        "no hubo ninguna reanudación: el test no puede decir nada"
    );
    // La medida tiene la resolución del tick: lo último que se ve como primera
    // parte es el tick anterior al pitido.
    let expected = regulation + stopped_for;
    let tick = football_domain::scenario::TICK;
    assert!(
        first_half_lasted + tick >= expected && first_half_lasted <= expected + tick,
        "la parte duró {first_half_lasted:?} y se jugaron {regulation:?} \
         con {stopped_for:?} parado"
    );
}

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

    let scenario = scenarios::interrupted_half();
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
    let scenario = scenarios::interrupted_half();
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
