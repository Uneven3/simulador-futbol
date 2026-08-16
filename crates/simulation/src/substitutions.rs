//! Legal substitutions: a scenario queues a team-sheet change and this system
//! serves it only while play is stopped. It owns the replacement body, so a
//! visual follows `Added<Player>` without ever becoming authoritative.

use crate::{SimulationSet, match_setup::spawn_player_body};
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use football_domain::{
    ByTeam, MatchRegulations, MatchState, Player, Position, Scenario, SetPiece, Substitution,
};

pub struct SubstitutionPlugin;

impl Plugin for SubstitutionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingSubstitutions>()
            .add_systems(Startup, queue_scenario_substitutions)
            .add_systems(
                FixedUpdate,
                serve_substitution_at_a_stoppage.in_set(SimulationSet::MatchLifecycle),
            );
    }
}

/// Changes waiting for the next legal opportunity and changes already used.
/// It is inspectable state, not hidden mutable state in the referee.
#[derive(Resource, Debug, Default)]
pub struct PendingSubstitutions {
    waiting: Vec<Substitution>,
    pub used: ByTeam<u8>,
}

fn queue_scenario_substitutions(
    scenario: Res<Scenario>,
    mut pending: ResMut<PendingSubstitutions>,
) {
    pending.waiting.clone_from(&scenario.substitutions);
}

fn serve_substitution_at_a_stoppage(
    mut commands: Commands,
    scenario: Res<Scenario>,
    regulations: Res<MatchRegulations>,
    match_state: Res<MatchState>,
    mut pending: ResMut<PendingSubstitutions>,
    players: Query<(Entity, &Player, &Position)>,
) {
    if match_state.phase.is_over() || match_state.set_piece == SetPiece::None {
        return;
    }
    let Some(change) = pending.waiting.first().copied() else {
        return;
    };
    let Some(team) = change.team() else {
        pending.waiting.remove(0);
        return;
    };
    if pending.used[team] >= regulations.maximum_substitutions
        || players
            .iter()
            .any(|(_, player, _)| player.id == change.incoming)
    {
        pending.waiting.remove(0);
        return;
    }
    let Some((outgoing_body, outgoing, position)) = players
        .iter()
        .find(|(_, player, _)| player.id == change.outgoing)
    else {
        pending.waiting.remove(0);
        return;
    };

    // The entrant inherits the current tactical slot; capacity, perception and
    // familiarity come from their own PlayerId in `spawn_player_body`.
    spawn_player_body(
        &mut commands,
        &scenario,
        change.incoming,
        outgoing.position,
        outgoing.role,
        outgoing.formation_slot,
        *position,
    );
    commands.entity(outgoing_body).despawn();
    pending.used[team] = pending.used[team].saturating_add(1);
    pending.waiting.remove(0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use football_domain::{PlayerId, Scenario, TeamId};

    #[test]
    fn a_change_has_one_team_or_is_invalid() {
        assert_eq!(
            Substitution::new(PlayerId::home(9), PlayerId::home(19)).team(),
            Some(TeamId::Home)
        );
        assert_eq!(
            Substitution::new(PlayerId::home(9), PlayerId::away(19)).team(),
            None
        );
    }

    #[test]
    fn a_queued_change_replaces_only_the_named_body_at_a_stoppage() {
        let scenario = Scenario::kick_off().with_substitutions(vec![Substitution::new(
            PlayerId::home(9),
            PlayerId::home(19),
        )]);
        let mut runner = crate::ScenarioRunner::headless(scenario);
        runner.advance();
        runner.advance();

        let world = runner.world_mut();
        let mut players = world.query::<&Player>();
        let ids: Vec<PlayerId> = players.iter(world).map(|player| player.id).collect();
        assert_eq!(ids.len(), 22, "a substitution must preserve eleven a side");
        assert!(!ids.contains(&PlayerId::home(9)));
        assert!(ids.contains(&PlayerId::home(19)));
        assert_eq!(
            world.resource::<PendingSubstitutions>().used[TeamId::Home],
            1
        );
    }
}
