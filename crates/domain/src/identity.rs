//! Who is who, independently of how the ECS happens to store them.
//!
//! Law 10: domain identities are newtypes and `Entity` is transitory. An
//! `Entity` is a slot in a world — it is not stable across a despawn, cannot be
//! serialised into a scenario and means nothing in a log. Everything the match
//! remembers about a participant is keyed by the types here.

use bevy_ecs::prelude::*;
use bevy_reflect::prelude::*;
use std::collections::HashMap;
use std::fmt;

/// One of the two sides contesting the match.
///
/// This is identity, not geography: which half a team defends changes at the
/// interval (Law 8) and is `TeamSide`, not this.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum TeamId {
    Home,
    Away,
}

impl TeamId {
    /// Both teams, in the order the per-team arrays are indexed.
    pub const BOTH: [TeamId; 2] = [TeamId::Home, TeamId::Away];

    /// Index into a two-slot per-team array.
    pub fn index(self) -> usize {
        match self {
            TeamId::Home => 0,
            TeamId::Away => 1,
        }
    }

    /// The other team. Replaces the `1 - team` arithmetic the port used.
    pub fn opponent(self) -> Self {
        match self {
            TeamId::Home => TeamId::Away,
            TeamId::Away => TeamId::Home,
        }
    }
}

impl fmt::Display for TeamId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TeamId::Home => f.write_str("Home"),
            TeamId::Away => f.write_str("Away"),
        }
    }
}

/// Which half of the pitch a team defends.
///
/// Left is -x, right is +x. Not yet used to place anyone: today the home team
/// defends the left for the whole match, and swapping sides at the interval is
/// the opening item of MVP 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum TeamSide {
    Left,
    Right,
}

/// A participant in the match: a team and a shirt number.
///
/// Shirt numbers are unique within a team and are how football itself names
/// players, so this reads in a log without a lookup table (`Away #9`) and
/// survives being written to disk, which `Entity` does not.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub struct PlayerId {
    pub team: TeamId,
    pub shirt: u8,
}

impl PlayerId {
    pub fn new(team: TeamId, shirt: u8) -> Self {
        Self { team, shirt }
    }

    pub fn home(shirt: u8) -> Self {
        Self::new(TeamId::Home, shirt)
    }

    pub fn away(shirt: u8) -> Self {
        Self::new(TeamId::Away, shirt)
    }
}

impl fmt::Display for PlayerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} #{}", self.team, self.shirt)
    }
}

/// One value per team, addressed by identity instead of by array index.
///
/// The port carried a dozen `[T; 2]` indexed with `team as usize` and reached
/// the opponent's slot with `1 - team`. Both are silent when wrong.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
pub struct ByTeam<T> {
    pub home: T,
    pub away: T,
}

impl<T> ByTeam<T> {
    pub fn new(home: T, away: T) -> Self {
        Self { home, away }
    }

    pub fn iter(&self) -> impl Iterator<Item = (TeamId, &T)> {
        [(TeamId::Home, &self.home), (TeamId::Away, &self.away)].into_iter()
    }
}

impl<T: Clone> ByTeam<T> {
    /// The same value for both teams.
    pub fn splat(value: T) -> Self {
        Self {
            home: value.clone(),
            away: value,
        }
    }
}

impl<T> std::ops::Index<TeamId> for ByTeam<T> {
    type Output = T;

    fn index(&self, team: TeamId) -> &T {
        match team {
            TeamId::Home => &self.home,
            TeamId::Away => &self.away,
        }
    }
}

impl<T> std::ops::IndexMut<TeamId> for ByTeam<T> {
    fn index_mut(&mut self, team: TeamId) -> &mut T {
        match team {
            TeamId::Home => &mut self.home,
            TeamId::Away => &mut self.away,
        }
    }
}

/// Resolves a domain identity back to the body currently representing it.
///
/// The match state names players by `PlayerId`; the systems that have to read
/// their position need an `Entity`. This is the only place the two are related,
/// and it is maintained from the world rather than written by hand, so a body
/// that leaves the pitch stops resolving instead of leaving a dangling id.
#[derive(Resource, Debug, Clone, Default)]
pub struct PlayerRegistry {
    bodies: HashMap<PlayerId, Entity>,
}

impl PlayerRegistry {
    /// The body currently on the pitch for this identity, if any.
    pub fn body(&self, id: PlayerId) -> Option<Entity> {
        self.bodies.get(&id).copied()
    }

    pub fn insert(&mut self, id: PlayerId, body: Entity) {
        self.bodies.insert(id, body);
    }

    /// Forgets a body, but only if it is still the one registered: a substitute
    /// may already have claimed the identity by the time the old body is
    /// cleaned up.
    pub fn remove_body(&mut self, body: Entity) {
        self.bodies.retain(|_, registered| *registered != body);
    }

    pub fn len(&self) -> usize {
        self.bodies.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bodies.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_player_reads_as_football_names_him() {
        assert_eq!(PlayerId::away(9).to_string(), "Away #9");
    }

    #[test]
    fn the_opponent_of_the_opponent_is_yourself() {
        for team in TeamId::BOTH {
            assert_eq!(team.opponent().opponent(), team);
            assert_ne!(team.opponent().index(), team.index());
        }
    }

    #[test]
    fn a_per_team_value_is_addressed_by_who_owns_it() {
        let mut scores = ByTeam::splat(0u32);
        scores[TeamId::Away] += 1;

        assert_eq!(scores[TeamId::Home], 0);
        assert_eq!(scores[TeamId::Away], 1);
        assert_eq!(scores[TeamId::Home.opponent()], 1);
    }

    /// A substitution replaces the body but not the identity, and the identity
    /// must keep resolving to whoever is on the pitch now.
    #[test]
    fn replacing_a_body_keeps_the_identity_resolvable() {
        let mut registry = PlayerRegistry::default();
        let first = Entity::from_raw_u32(1).unwrap();
        let second = Entity::from_raw_u32(2).unwrap();

        registry.insert(PlayerId::home(9), first);
        registry.insert(PlayerId::home(9), second);
        registry.remove_body(first);

        assert_eq!(registry.body(PlayerId::home(9)), Some(second));
    }
}
