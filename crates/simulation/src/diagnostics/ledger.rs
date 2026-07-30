//! What the facts add up to.
//!
//! The kernel reports single events; the questions worth asking are about the
//! accumulation ("do passes arrive?", "how often does the ball change hands?").
//! This is where that arithmetic lives, so the kernel keeps no counters and no
//! test has to rebuild them from scratch.

use football_domain::diagnostics::{MatchFact, PossessionCause, ReleaseKind};
use super::telemetry::MatchTelemetry;
use bevy_ecs::prelude::*;
use bevy_math::Vec2;
use football_domain::scenario::TICK;
use football_domain::{ByTeam, PlayerId, TeamId};
use std::collections::HashSet;
use std::time::Duration;

/// A pass is judged near or far from where it was aimed: a ball lost within
/// this radius is a failed reception, beyond it an interception en route. The
/// distinction is the difference between fixing the trap and fixing the lane.
const RECEPTION_RADIUS_METRES: f32 = 2.5;

/// Running totals for the match. Derived from the fact stream, never written by
/// the kernel.
#[derive(Resource, Debug, Default)]
pub struct MatchLedger {
    /// Deliberate releases that ended with the other team on the ball.
    turnovers: [u32; 4],
    /// Of the lost passes, those lost at the receiver against those lost on the
    /// way to him.
    pub pass_turnovers_near: u32,
    pub pass_turnovers_far: u32,
    /// Times the ball changed teams.
    pub possession_changes: u32,
    tackles: u32,
    loose_balls: u32,
    touchers: HashSet<PlayerId>,
    pub goals: ByTeam<u32>,
    pub longest_spell: Duration,
    spell_started_at: Option<u64>,
    holding_team: Option<TeamId>,
    /// The release being tracked for attribution, if the ball is still loose.
    pending_release: Option<PendingRelease>,
}

#[derive(Debug, Clone, Copy)]
struct PendingRelease {
    by: TeamId,
    kind: ReleaseKind,
    aim: Vec2,
}

impl MatchLedger {
    pub fn turnovers_of(&self, kind: ReleaseKind) -> u32 {
        self.turnovers[release_index(kind)]
    }

    pub fn distinct_touchers(&self) -> usize {
        self.touchers.len()
    }

    pub fn tackles(&self) -> u32 {
        self.tackles
    }

    pub fn loose_balls(&self) -> u32 {
        self.loose_balls
    }

    /// Possession changes per minute of simulated play — the number that says
    /// whether the match is football or a stealing metronome.
    pub fn changes_per_minute(&self, elapsed: Duration) -> f32 {
        let minutes = elapsed.as_secs_f32() / 60.0;
        if minutes <= 0.0 {
            0.0
        } else {
            self.possession_changes as f32 / minutes
        }
    }

    fn absorb(&mut self, tick: u64, fact: MatchFact) -> Option<MatchFact> {
        match fact {
            MatchFact::BallReleased { player, kind, aim } => {
                self.pending_release = Some(PendingRelease {
                    by: player.team,
                    kind,
                    aim,
                });
            }
            MatchFact::Touched { player, .. } => {
                self.touchers.insert(player);
            }
            MatchFact::Goal { scored_by } => self.goals[scored_by] += 1,
            MatchFact::PossessionGained {
                player,
                cause,
                from,
                at,
            } => {
                match cause {
                    PossessionCause::Tackle => self.tackles += 1,
                    PossessionCause::LooseBall => self.loose_balls += 1,
                }
                let _ = from;

                if self.holding_team != Some(player.team) {
                    if let Some(started) = self.spell_started_at {
                        let spell = TICK * (tick.saturating_sub(started) as u32);
                        self.longest_spell = self.longest_spell.max(spell);
                    }
                    if self.holding_team.is_some() {
                        self.possession_changes += 1;
                    }
                    self.holding_team = Some(player.team);
                    self.spell_started_at = Some(tick);

                    // The team that released it did not get it back: attribute
                    // the loss to the action that gave it away.
                    if let Some(release) = self.pending_release.take()
                        && release.by != player.team
                    {
                        let metres_from_aim = release.aim.distance(at);
                        self.turnovers[release_index(release.kind)] += 1;
                        if release.kind == ReleaseKind::Pass {
                            if metres_from_aim < RECEPTION_RADIUS_METRES {
                                self.pass_turnovers_near += 1;
                            } else {
                                self.pass_turnovers_far += 1;
                            }
                        }
                        return Some(MatchFact::Turnover {
                            lost_by: release.by,
                            won_by: player,
                            release: release.kind,
                            metres_from_aim,
                        });
                    }
                }
            }
            _ => {}
        }
        None
    }
}

fn release_index(kind: ReleaseKind) -> usize {
    match kind {
        ReleaseKind::Pass => 0,
        ReleaseKind::DribbleKnock => 1,
        ReleaseKind::Clearance => 2,
        ReleaseKind::Shot => 3,
    }
}

/// Reads this tick's facts, updates the totals, and reports the turnovers it
/// derived back into the stream so the log can show them.
///
/// Runs before the console sink, so a derived fact lands in the same tick as
/// the events it was derived from.
pub(super) fn accumulate_facts(
    mut ledger: ResMut<MatchLedger>,
    mut telemetry: ResMut<MatchTelemetry>,
) {
    let tick = telemetry.tick();
    let facts: Vec<MatchFact> = telemetry.this_tick().to_vec();
    let mut derived = Vec::new();
    for fact in facts {
        if let Some(turnover) = ledger.absorb(tick, fact) {
            derived.push(turnover);
        }
    }
    for fact in derived {
        telemetry.record(fact);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ball_given_away_is_attributed_to_the_action_that_gave_it() {
        let mut ledger = MatchLedger::default();
        ledger.absorb(
            0,
            MatchFact::PossessionGained {
                player: PlayerId::home(8),
                from: None,
                cause: PossessionCause::LooseBall,
                at: Vec2::ZERO,
            },
        );
        ledger.absorb(
            10,
            MatchFact::BallReleased {
                player: PlayerId::home(8),
                kind: ReleaseKind::Pass,
                aim: Vec2::new(10.0, 0.0),
            },
        );
        let derived = ledger.absorb(
            40,
            MatchFact::PossessionGained {
                player: PlayerId::away(4),
                from: None,
                cause: PossessionCause::LooseBall,
                at: Vec2::new(10.4, 0.0),
            },
        );

        assert_eq!(ledger.turnovers_of(ReleaseKind::Pass), 1);
        assert_eq!(ledger.possession_changes, 1);
        assert!(matches!(
            derived,
            Some(MatchFact::Turnover {
                lost_by: TeamId::Home,
                release: ReleaseKind::Pass,
                ..
            })
        ));
    }

    /// Winning your own loose ball back is not a turnover, and the port's
    /// counter had no way to tell the two apart.
    #[test]
    fn recovering_your_own_pass_is_not_a_turnover() {
        let mut ledger = MatchLedger::default();
        ledger.absorb(
            0,
            MatchFact::BallReleased {
                player: PlayerId::home(8),
                kind: ReleaseKind::Pass,
                aim: Vec2::ZERO,
            },
        );
        let derived = ledger.absorb(
            30,
            MatchFact::PossessionGained {
                player: PlayerId::home(9),
                from: None,
                cause: PossessionCause::LooseBall,
                at: Vec2::ZERO,
            },
        );

        assert_eq!(ledger.turnovers_of(ReleaseKind::Pass), 0);
        assert!(derived.is_none());
        assert_eq!(ledger.possession_changes, 0, "the ball never changed teams");
    }

    #[test]
    fn the_longest_spell_is_measured_between_changes() {
        let mut ledger = MatchLedger::default();
        for (tick, team) in [(0u64, TeamId::Home), (500, TeamId::Away), (600, TeamId::Home)] {
            ledger.absorb(
                tick,
                MatchFact::PossessionGained {
                    player: PlayerId::new(team, 9),
                    from: None,
                    cause: PossessionCause::LooseBall,
                    at: Vec2::ZERO,
                },
            );
        }
        // 500 ticks of 10 ms
        assert_eq!(ledger.longest_spell, Duration::from_secs(5));
        assert_eq!(ledger.possession_changes, 2);
    }
}
