//! Law 10: what the match remembers is keyed by domain identity, never by the
//! ECS slot a body happens to occupy.

use football_domain::{Player, PlayerRegistry};
use gameplayfootball::{PlayerId, ScenarioRunner, TeamId, scenarios};

/// Every body on the pitch is reachable from its identity, and no two players
/// share one. A duplicate would make the registry silently lose a player.
#[test]
fn every_player_is_reachable_from_his_identity() {
    let mut runner = ScenarioRunner::headless(scenarios::opening_minute());
    runner.advance();

    let world = runner.world_mut();
    let identities: Vec<PlayerId> = world
        .query::<&Player>()
        .iter(world)
        .map(|player| player.id)
        .collect();

    assert_eq!(identities.len(), 22, "a match is eleven a side");

    let world = runner.world_mut();
    let registry = world.resource::<PlayerRegistry>();
    assert_eq!(registry.len(), identities.len(), "a body went unregistered");
    for id in &identities {
        assert!(registry.body(*id).is_some(), "{id} resolves to nobody");
    }

    for team in TeamId::BOTH {
        let mut shirts: Vec<u8> = identities
            .iter()
            .filter(|id| id.team == team)
            .map(|id| id.shirt)
            .collect();
        shirts.sort_unstable();
        shirts.dedup();
        assert_eq!(shirts.len(), 11, "{team} has two players in one shirt");
    }
}

/// The possession bookkeeping names people, not entities. If this ever reads an
/// `Entity` again, a substitution or a replay would hand the ball to a stranger.
#[test]
fn possession_is_recorded_against_a_person() {
    let scenario = scenarios::opening_minute();
    let ticks = scenario.ticks();
    let mut runner = ScenarioRunner::headless(scenario);

    // Sampled while the match runs, not at the final tick: whether the ball
    // happens to be loose when the window closes says nothing about who held it.
    let mut possessor = None;
    for _ in 0..ticks {
        runner.advance();
        if let Some(holder) = runner
            .world_mut()
            .resource::<gameplayfootball::MatchState>()
            .possession_player
        {
            possessor = Some(holder);
            break;
        }
    }
    let Some(possessor) = possessor else {
        panic!("nobody held the ball in a minute of play");
    };

    let world = runner.world_mut();
    let registry = world.resource::<PlayerRegistry>();
    let body = registry
        .body(possessor)
        .unwrap_or_else(|| panic!("{possessor} held the ball but is on no pitch"));
    let player = world.get::<Player>(body).expect("a body without a player");

    assert_eq!(player.id, possessor);
}
