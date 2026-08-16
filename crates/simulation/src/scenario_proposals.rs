//! La alternativa explícita de una situación: modifica una intención, nunca el
//! cuerpo ni la pelota. El motor sigue resolviendo lo que esa intención alcanza.

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_math::Vec3;
use football_domain::{MovementIntent, Player, Scenario};

use crate::SimulationSet;

pub struct ScenarioProposalPlugin;

impl Plugin for ScenarioProposalPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            apply_movement_proposals
                .in_set(SimulationSet::Players)
                .after(crate::player_decisions::select_player_movement)
                .before(crate::locomotion::drive_bodies),
        );
    }
}

/// Sustituye solamente las intenciones que la alternativa nombra. El resto de
/// los veintiuno sigue decidiendo sobre lo que percibe en esta misma corrida.
pub fn apply_movement_proposals(
    scenario: Res<Scenario>,
    mut players: Query<(&Player, &mut MovementIntent)>,
) {
    for (player, mut intent) in players.iter_mut() {
        if let Some(proposal) = scenario
            .movement_proposals
            .iter()
            .find(|proposal| proposal.player == player.id)
        {
            intent.0 = Vec3::new(
                proposal.desired_velocity.x,
                proposal.desired_velocity.y,
                0.0,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_math::Vec2;
    use football_domain::{MovementProposal, PlayerId, PlayingPosition};

    #[test]
    fn a_proposal_replaces_only_the_named_intent() {
        let proposal = MovementProposal {
            player: PlayerId::home(6),
            desired_velocity: Vec2::new(3.0, -2.0),
        };
        let scenario = Scenario::kick_off().with_movement_proposals(vec![proposal]);
        let mut app = App::new();
        app.insert_resource(scenario);
        let named = app
            .world_mut()
            .spawn((
                Player::new(
                    proposal.player,
                    PlayingPosition::CentreMidfielder,
                    Vec2::ZERO,
                ),
                MovementIntent(Vec3::X),
            ))
            .id();
        let other = app
            .world_mut()
            .spawn((
                Player::new(
                    PlayerId::home(7),
                    PlayingPosition::CentreMidfielder,
                    Vec2::ZERO,
                ),
                MovementIntent(Vec3::Y),
            ))
            .id();
        app.add_systems(Update, apply_movement_proposals);
        app.update();

        assert_eq!(
            app.world()
                .entity(named)
                .get::<MovementIntent>()
                .map(|intent| intent.0),
            Some(Vec3::new(3.0, -2.0, 0.0))
        );
        assert_eq!(
            app.world()
                .entity(other)
                .get::<MovementIntent>()
                .map(|intent| intent.0),
            Some(Vec3::Y)
        );
    }
}
