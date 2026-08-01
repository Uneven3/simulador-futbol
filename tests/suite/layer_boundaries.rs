//! The layer contract, checked where both sides are visible at once.
//!
//! Only the app can name a `Mesh` *and* a `Position`, so this is where the
//! separation between authoritative bodies and their representations is proven.
//! Cargo already forbids the reverse dependency: `football_simulation` does not
//! depend on `bevy`, so no rule can be expressed in terms of a mesh.

use bevy::prelude::*;
use football_domain::{Ball, Player, Position};
use football_presentation::VisualOf;
use gameplayfootball::{ScenarioRunner, scenarios, with_primitives};

fn body_positions(runner: &mut ScenarioRunner) -> Vec<Vec3> {
    let world = runner.world_mut();
    let mut query = world.query::<&Position>();
    let mut positions: Vec<Vec3> = query.iter(world).map(|position| position.0).collect();
    positions.sort_by(|a, b| {
        a.to_array()
            .iter()
            .zip(b.to_array().iter())
            .map(|(left, right)| left.total_cmp(right))
            .find(|ordering| ordering.is_ne())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    positions
}

/// MVP 1's core claim: the same situation, headless and rendered, is the same
/// situation. Presentation reads and interpolates; it never nudges the truth.
#[test]
fn presentation_does_not_change_the_match() {
    let mut headless = ScenarioRunner::headless(scenarios::opening_minute());
    let mut rendered = with_primitives(scenarios::opening_minute());

    for tick in 0..600 {
        headless.advance();
        rendered.advance();

        let (a, b) = (body_positions(&mut headless), body_positions(&mut rendered));
        assert_eq!(
            a.len(),
            b.len(),
            "tick {tick}: the two runs hold a different number of bodies"
        );
        for (index, (headless_pos, rendered_pos)) in a.iter().zip(b.iter()).enumerate() {
            assert_eq!(
                headless_pos, rendered_pos,
                "tick {tick}: body {index} diverged once visuals were attached \
                 ({headless_pos:?} headless vs {rendered_pos:?} rendered): \
                 presentation is writing authoritative state"
            );
        }
    }
}

/// The rendered run must also reach the same verdict, not just the same
/// positions: a scenario is judged the same way with or without a screen.
#[test]
fn a_rendered_scenario_reaches_the_same_verdict() {
    let scenario = scenarios::shot_crossing_the_goal_line();
    let expectations = scenario.expectations.clone();

    let headless = ScenarioRunner::headless(scenario.clone()).run();
    let rendered = with_primitives(scenario).run();

    assert!(
        rendered.mismatches(&expectations).is_empty(),
        "the rendered run failed the scenario: {:?}",
        rendered.mismatches(&expectations)
    );
    assert_eq!(headless.score, rendered.score);
    assert_eq!(headless.set_pieces, rendered.set_pieces);
}

/// Law 2: an authoritative entity carries no visuals, and every body gets
/// exactly one disposable representation pointing back at it.
#[test]
fn bodies_and_their_visuals_stay_separate() {
    let mut runner = with_primitives(scenarios::opening_minute());
    for _ in 0..10 {
        runner.advance();
    }
    let world = runner.world_mut();

    let mut bodies = world.query::<(Entity, &Position)>();
    let body_entities: Vec<Entity> = bodies.iter(world).map(|(entity, _)| entity).collect();
    assert_eq!(
        body_entities.len(),
        23,
        "expected one ball and two elevens as authoritative bodies"
    );

    let mut visual_bodies = world.query::<(&Position, &Mesh3d)>();
    assert_eq!(
        visual_bodies.iter(world).count(),
        0,
        "an authoritative body carries a mesh: visuals must live on their own entities"
    );

    let mut visuals = world.query::<(&VisualOf, &Transform)>();
    let represented: Vec<Entity> = visuals
        .iter(world)
        .map(|(visual_of, _)| visual_of.0)
        .collect();
    assert_eq!(
        represented.len(),
        body_entities.len(),
        "every body must have exactly one representation"
    );
    for body in &body_entities {
        assert!(
            represented.contains(body),
            "body {body:?} has no visual representation"
        );
    }

    // the ball's representation tracks the ball it points at
    let mut ball_body = world.query::<(Entity, &Ball, &Position)>();
    let (ball_entity, _, ball_position) = ball_body
        .single(world)
        .expect("a match has exactly one ball");
    let (ball_entity, ball_position) = (ball_entity, ball_position.0);
    let mut ball_visuals = world.query::<(&VisualOf, &Transform)>();
    let ball_visual = ball_visuals
        .iter(world)
        .find(|(visual_of, _)| visual_of.0 == ball_entity)
        .map(|(_, transform)| transform.translation)
        .expect("the ball has no visual");
    assert!(
        ball_visual.distance(ball_position) < 0.5,
        "the ball's visual sits at {ball_visual:?} while the ball is at {ball_position:?}"
    );

    // and so does every player's
    let mut players = world.query::<(Entity, &Player)>();
    let player_entities: Vec<Entity> = players.iter(world).map(|(entity, _)| entity).collect();
    assert_eq!(player_entities.len(), 22);
    for player in player_entities {
        assert!(
            represented.contains(&player),
            "player {player:?} has no visual representation"
        );
    }
}
