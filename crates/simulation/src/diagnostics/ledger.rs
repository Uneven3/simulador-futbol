//! What the facts add up to.
//!
//! The kernel reports single events; the questions worth asking are about the
//! accumulation ("do passes arrive?", "how often does the ball change hands?").
//! This is where that arithmetic lives, so the kernel keeps no counters and no
//! test has to rebuild them from scratch.

use super::telemetry::MatchTelemetry;
use bevy_ecs::prelude::*;
use bevy_math::{Vec2, Vec3};
use football_domain::diagnostics::{MatchFact, PossessionCause, ReleaseKind};
use football_domain::scenario::TICK;
use football_domain::{Ball, ByTeam, PitchConfig, PlayerId, TeamId};
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
    /// Shots struck, and those whose trajectory was entering the goal when they
    /// left the boot. "On target" is measured before anyone intervenes: it is a
    /// property of the strike, not of what the defence did about it.
    pub shots: ByTeam<u32>,
    pub shots_on_target: ByTeam<u32>,
    /// Passes played, and those the other team ended up with.
    pub passes: ByTeam<u32>,
    pub passes_lost: ByTeam<u32>,
    /// Time each team spent as the last team to have taken the ball. It is
    /// possession in the broadcast sense (who is on the ball), not time of
    /// controlled contact.
    pub possession_time: ByTeam<Duration>,
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

    /// Share of the time each team was the team on the ball, in 0..1. Sums to
    /// less than one by the opening seconds nobody had touched it yet.
    pub fn possession_share(&self) -> ByTeam<f32> {
        let total =
            (self.possession_time[TeamId::Home] + self.possession_time[TeamId::Away]).as_secs_f32();
        if total <= 0.0 {
            return ByTeam::default();
        }
        let mut share = ByTeam::default();
        for team in TeamId::BOTH {
            share[team] = self.possession_time[team].as_secs_f32() / total;
        }
        share
    }

    /// Passes that reached their own team, per team.
    pub fn passes_completed(&self) -> ByTeam<u32> {
        let mut completed = ByTeam::default();
        for team in TeamId::BOTH {
            completed[team] = self.passes[team].saturating_sub(self.passes_lost[team]);
        }
        completed
    }

    /// One tick of match time passed with `holding_team` on the ball.
    fn advance_possession_clock(&mut self) {
        if let Some(team) = self.holding_team {
            self.possession_time[team] += TICK;
        }
    }

    fn record_shot(&mut self, by: TeamId, on_target: bool) {
        self.shots[by] += 1;
        if on_target {
            self.shots_on_target[by] += 1;
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
                if kind == ReleaseKind::Pass {
                    self.passes[player.team] += 1;
                }
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
                            self.passes_lost[release.by] += 1;
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

/// Whether a struck ball was going in, judged on the trajectory the physics
/// just computed for it.
///
/// This is what "on target" means here: the shot was entering the goal mouth
/// when it left the boot. Whether a keeper then reaches it is a different
/// question, and answering both with one counter would hide which of the two is
/// broken.
pub fn trajectory_enters_goal(
    predictions: &[Vec3],
    attacking_towards_x: f32,
    pitch: &PitchConfig,
) -> bool {
    let goal_line = pitch.half_width * attacking_towards_x.signum();
    predictions.windows(2).any(|pair| {
        let (before, after) = (pair[0], pair[1]);
        // the step in which the ball crosses the goal line, if any
        let crossing = (before.x - goal_line) * (after.x - goal_line) <= 0.0
            && (before.x - goal_line).abs() + (after.x - goal_line).abs() > 0.0;
        if !crossing {
            return false;
        }
        let travelled = (after.x - before.x).abs();
        let fraction = if travelled > 0.0 {
            (goal_line - before.x).abs() / travelled
        } else {
            0.0
        };
        let y = before.y + (after.y - before.y) * fraction;
        let z = before.z + (after.z - before.z) * fraction;
        y.abs() <= pitch.goal_half_width && z <= pitch.goal_height
    })
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
    pitch: Res<PitchConfig>,
    ball_query: Query<&Ball>,
) {
    let tick = telemetry.tick();
    let facts: Vec<MatchFact> = telemetry.this_tick().to_vec();
    let mut derived = Vec::new();
    for fact in facts {
        // A shot is classified here and not at the boot because this runs after
        // ball physics: the prediction already is the trajectory of THIS strike.
        if let MatchFact::BallReleased {
            player,
            kind: ReleaseKind::Shot,
            ..
        } = fact
            && let Ok(ball) = ball_query.single()
        {
            let attacking_towards_x = -crate::team_tactics::team_side(player.team);
            let on_target = trajectory_enters_goal(&ball.predictions, attacking_towards_x, &pitch);
            ledger.record_shot(player.team, on_target);
        }
        if let Some(turnover) = ledger.absorb(tick, fact) {
            derived.push(turnover);
        }
    }
    ledger.advance_possession_clock();
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

    /// A straight flight towards the goal, sampled every 10 ms as the physics
    /// does. What separates the four cases is only where it crosses.
    fn flight_towards(from: Vec3, to_goal_line_at: Vec3) -> Vec<Vec3> {
        (0..=20)
            .map(|step| {
                let t = step as f32 / 20.0;
                from + (to_goal_line_at - from) * t
            })
            .collect()
    }

    #[test]
    fn a_shot_is_on_target_when_its_flight_crosses_between_the_posts() {
        let pitch = PitchConfig::default();
        let from = Vec3::new(40.0, 0.0, 0.2);

        let inside = flight_towards(from, Vec3::new(55.0, 2.0, 1.0));
        assert!(trajectory_enters_goal(&inside, 1.0, &pitch));

        let wide = flight_towards(from, Vec3::new(55.0, 6.0, 1.0));
        assert!(!trajectory_enters_goal(&wide, 1.0, &pitch), "6 m wide");

        let over = flight_towards(from, Vec3::new(55.0, 0.0, 4.0));
        assert!(!trajectory_enters_goal(&over, 1.0, &pitch), "over the bar");

        // dying short of the line is not a shot on target, however well aimed
        let short: Vec<Vec3> = flight_towards(from, Vec3::new(50.0, 0.0, 0.2));
        assert!(!trajectory_enters_goal(&short, 1.0, &pitch));
    }

    /// The same flight belongs to the team attacking that way, and to nobody
    /// else: a clearance towards one's own goal is not a shot on target.
    #[test]
    fn on_target_is_judged_against_the_goal_being_attacked() {
        let pitch = PitchConfig::default();
        let flight = flight_towards(Vec3::new(40.0, 0.0, 0.2), Vec3::new(55.0, 1.0, 1.0));

        assert!(trajectory_enters_goal(&flight, 1.0, &pitch));
        assert!(!trajectory_enters_goal(&flight, -1.0, &pitch));
    }

    #[test]
    fn possession_share_splits_the_time_on_the_ball() {
        let mut ledger = MatchLedger {
            holding_team: Some(TeamId::Home),
            ..Default::default()
        };
        for _ in 0..300 {
            ledger.advance_possession_clock();
        }
        ledger.holding_team = Some(TeamId::Away);
        for _ in 0..100 {
            ledger.advance_possession_clock();
        }

        let share = ledger.possession_share();
        assert!((share[TeamId::Home] - 0.75).abs() < 1e-5);
        assert!((share[TeamId::Away] - 0.25).abs() < 1e-5);
    }

    #[test]
    fn the_longest_spell_is_measured_between_changes() {
        let mut ledger = MatchLedger::default();
        for (tick, team) in [
            (0u64, TeamId::Home),
            (500, TeamId::Away),
            (600, TeamId::Home),
        ] {
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
