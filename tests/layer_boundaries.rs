//! The layer contract, checked where both sides are visible at once.
//!
//! Only the app can name a `Mesh` *and* a `Position`, so this is where the
//! separation between authoritative bodies and their representations is proven.
//! Cargo already forbids the reverse dependency: `football_simulation` does not
//! depend on `bevy`, so no rule can be expressed in terms of a mesh.

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use football_domain::{Ball, BallTouched, MatchState, PitchConfig, Player, Position};
use football_presentation::{PrimitiveVisualsPlugin, VisualOf};
use football_simulation::{
    BallCollisionPlugin, BallPhysicsPlugin, MatchSetupPlugin, PlayerMovementPlugin, RefereePlugin,
    SimulationOrderPlugin,
};
use std::time::Duration;

/// The authoritative match, with one fixed tick per `update()`.
fn simulation_only_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(Time::<Fixed>::from_hz(100.0));
    app.insert_resource(MatchState::default());
    app.insert_resource(PitchConfig::default());
    app.add_message::<BallTouched>();
    app.add_plugins((
        MatchSetupPlugin,
        SimulationOrderPlugin,
        BallPhysicsPlugin,
        BallCollisionPlugin,
        RefereePlugin,
        PlayerMovementPlugin,
    ));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        10,
    )));
    app
}

/// The same match with primitive visuals attached. Assets are registered without
/// a renderer: presentation needs somewhere to put meshes, not a window.
fn simulation_with_visuals_app() -> App {
    let mut app = simulation_only_app();
    app.add_plugins(AssetPlugin::default());
    app.init_asset::<Mesh>();
    app.init_asset::<StandardMaterial>();
    app.add_plugins(PrimitiveVisualsPlugin);
    app
}

fn body_positions(app: &mut App) -> Vec<Vec3> {
    let mut query = app.world_mut().query::<&Position>();
    let mut positions: Vec<Vec3> = query.iter(app.world()).map(|position| position.0).collect();
    positions.sort_by(|a, b| a.to_array().partial_cmp(&b.to_array()).unwrap());
    positions
}

/// MVP 1's core claim: the same situation, headless and rendered, is the same
/// situation. Presentation reads and interpolates; it never nudges the truth.
#[test]
fn presentation_does_not_change_the_match() {
    let mut headless = simulation_only_app();
    let mut rendered = simulation_with_visuals_app();

    for tick in 0..600 {
        headless.update();
        rendered.update();

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

/// Law 2: an authoritative entity carries no visuals, and every body gets
/// exactly one disposable representation pointing back at it.
#[test]
fn bodies_and_their_visuals_stay_separate() {
    let mut app = simulation_with_visuals_app();
    for _ in 0..10 {
        app.update();
    }

    let mut bodies = app.world_mut().query::<(Entity, &Position)>();
    let body_entities: Vec<Entity> = bodies.iter(app.world()).map(|(entity, _)| entity).collect();
    assert_eq!(
        body_entities.len(),
        23,
        "expected one ball and two elevens as authoritative bodies"
    );

    let mut visual_bodies = app.world_mut().query::<(&Position, &Mesh3d)>();
    assert_eq!(
        visual_bodies.iter(app.world()).count(),
        0,
        "an authoritative body carries a mesh: visuals must live on their own entities"
    );

    let mut visuals = app.world_mut().query::<(&VisualOf, &Transform)>();
    let represented: Vec<Entity> = visuals
        .iter(app.world())
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

    // and the representations track the bodies they point at
    let mut ball_body = app.world_mut().query::<(Entity, &Ball, &Position)>();
    let (ball_entity, _, ball_position) = ball_body.single(app.world()).unwrap();
    let (ball_entity, ball_position) = (ball_entity, ball_position.0);
    let mut ball_visuals = app.world_mut().query::<(&VisualOf, &Transform)>();
    let ball_visual = ball_visuals
        .iter(app.world())
        .find(|(visual_of, _)| visual_of.0 == ball_entity)
        .map(|(_, transform)| transform.translation)
        .expect("the ball has no visual");
    assert!(
        ball_visual.distance(ball_position) < 0.5,
        "the ball's visual sits at {ball_visual:?} while the ball is at {ball_position:?}"
    );

    // players are represented too, and their visuals are separate entities
    let mut players = app.world_mut().query::<(Entity, &Player)>();
    let player_entities: Vec<Entity> = players.iter(app.world()).map(|(e, _)| e).collect();
    for player in player_entities {
        assert!(
            !represented.is_empty() && represented.contains(&player),
            "player {player:?} has no visual representation"
        );
    }
}
