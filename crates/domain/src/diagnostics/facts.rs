//! What the kernel reports having happened, as typed values.
//!
//! Facts are not log lines. A line has to be parsed to be counted, so the
//! forensic tests grew their own private collection instead of reading what the
//! match already knew. These are `Copy` values with no allocation: a consumer
//! counts them, a sink renders them, and adding a consumer costs nothing.

use crate::{Card, MatchPhase, PlayerId, SetPiece, TeamId};

/// How the ball left a player deliberately. Recorded when he releases it, read
/// when the other team gets it: a turnover is only meaningful attributed to the
/// action that lost it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReleaseKind {
    Pass,
    DribbleKnock,
    Clearance,
    Shot,
}

impl ReleaseKind {
    pub fn label(self) -> &'static str {
        match self {
            ReleaseKind::Pass => "pass",
            ReleaseKind::DribbleKnock => "knock-on",
            ReleaseKind::Clearance => "clearance",
            ReleaseKind::Shot => "shot",
        }
    }
}

/// How possession changed hands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PossessionCause {
    /// Taken off an opponent who was carrying it.
    Tackle,
    /// Collected while nobody held it.
    LooseBall,
}

impl PossessionCause {
    pub fn label(self) -> &'static str {
        match self {
            PossessionCause::Tackle => "tackle",
            PossessionCause::LooseBall => "loose",
        }
    }
}

/// One thing that happened, at the tick it happened.
///
/// Deliberately flat and `Copy`: a fact that owned a `String` would allocate on
/// a hot path for the benefit of one sink.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MatchFact {
    PossessionGained {
        player: PlayerId,
        from: Option<PlayerId>,
        cause: PossessionCause,
        /// Where the ball was won, so a lost pass can be measured against where
        /// it was aimed.
        at: bevy_math::Vec2,
    },
    PossessionLost {
        player: PlayerId,
        /// Where the ball was when it stopped being under control.
        at: bevy_math::Vec2,
    },
    BallReleased {
        player: PlayerId,
        kind: ReleaseKind,
        /// Where the ball was aimed. For a knock-on, where it was knocked.
        aim: bevy_math::Vec2,
    },
    /// The other team ended up with a ball this one deliberately released.
    Turnover {
        lost_by: TeamId,
        won_by: PlayerId,
        release: ReleaseKind,
        /// Distance in metres from where the ball was aimed, so a failed
        /// reception (near) can be told from an interception en route (far).
        metres_from_aim: f32,
    },
    Touched {
        player: PlayerId,
        deliberate: bool,
    },
    Goal {
        scored_by: TeamId,
    },
    RestartAwarded {
        set_piece: SetPiece,
        team: TeamId,
    },
    OffsideGiven {
        against: PlayerId,
    },
    PhaseEntered(MatchPhase),
    /// A team sent someone beyond the ball to be found.
    AttackingRun {
        runner: PlayerId,
    },
    /// A shot that was going in did not: the keeper reached it, and either kept
    /// it or pushed it away.
    ShotSaved {
        keeper: PlayerId,
        caught: bool,
    },
    /// The referee whistled a contact.
    FoulGiven {
        by: PlayerId,
        on: PlayerId,
    },
    CardShown {
        to: PlayerId,
        card: Card,
    },
    /// ...and let this one go, because stopping play would have punished the
    /// side that was fouled (Law 5).
    AdvantagePlayed {
        to: TeamId,
    },
}

impl MatchFact {
    /// Which question this fact belongs to. A channel is off until someone is
    /// asking it, so the fact has to know where it lands.
    pub fn channel(self) -> DiagnosticChannel {
        match self {
            MatchFact::PossessionGained { .. } | MatchFact::PossessionLost { .. } => {
                DiagnosticChannel::Possession
            }
            MatchFact::BallReleased { .. } | MatchFact::Turnover { .. } => {
                DiagnosticChannel::PassOutcomes
            }
            MatchFact::Touched { .. } | MatchFact::ShotSaved { .. } => DiagnosticChannel::Touches,
            MatchFact::Goal { .. }
            | MatchFact::RestartAwarded { .. }
            | MatchFact::FoulGiven { .. }
            | MatchFact::CardShown { .. }
            | MatchFact::AdvantagePlayed { .. }
            | MatchFact::OffsideGiven { .. } => DiagnosticChannel::RefereeDecisions,
            MatchFact::PhaseEntered(_) => DiagnosticChannel::PhaseTransitions,
            MatchFact::AttackingRun { .. } => DiagnosticChannel::Formation,
        }
    }
}

impl std::fmt::Display for MatchFact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            MatchFact::PossessionGained {
                player,
                from,
                cause,
                ..
            } => match from {
                Some(from) => write!(f, "{player} took the ball off {from} ({})", cause.label()),
                None => write!(f, "{player} collected the ball ({})", cause.label()),
            },
            MatchFact::PossessionLost { player, at } => {
                write!(f, "{player} lost the ball at ({:.1}, {:.1})", at.x, at.y)
            }
            MatchFact::FoulGiven { by, on } => write!(f, "{by} fouled {on}"),
            MatchFact::CardShown { to, card } => write!(f, "{card:?} card to {to}"),
            MatchFact::AdvantagePlayed { to } => write!(f, "advantage to {to}"),
            MatchFact::ShotSaved { keeper, caught } => {
                let how = if caught {
                    "caught it"
                } else {
                    "pushed it away"
                };
                write!(f, "{keeper} saved the shot and {how}")
            }
            MatchFact::BallReleased { player, kind, aim } => write!(
                f,
                "{player} played a {} towards ({:.1}, {:.1})",
                kind.label(),
                aim.x,
                aim.y
            ),
            MatchFact::Turnover {
                lost_by,
                won_by,
                release,
                metres_from_aim,
            } => write!(
                f,
                "{lost_by} lost a {} to {won_by}, {metres_from_aim:.1} m from the aim",
                release.label()
            ),
            MatchFact::Touched { player, deliberate } => {
                let how = if deliberate { "played" } else { "deflected" };
                write!(f, "{player} {how} the ball")
            }
            MatchFact::Goal { scored_by } => write!(f, "GOAL for {scored_by}"),
            MatchFact::RestartAwarded { set_piece, team } => {
                write!(f, "{set_piece:?} to {team}")
            }
            MatchFact::OffsideGiven { against } => write!(f, "offside against {against}"),
            MatchFact::PhaseEntered(phase) => write!(f, "{phase:?}"),
            MatchFact::AttackingRun { runner } => write!(f, "{runner} is making a run"),
        }
    }
}

/// The questions the diagnostics can answer. Each is off until asked: a log
/// that is always on is a log nobody reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticChannel {
    Possession,
    RefereeDecisions,
    Touches,
    PassOutcomes,
    PhaseTransitions,
    Formation,
    Performance,
}

impl DiagnosticChannel {
    pub const ALL: [DiagnosticChannel; 7] = [
        DiagnosticChannel::Possession,
        DiagnosticChannel::RefereeDecisions,
        DiagnosticChannel::Touches,
        DiagnosticChannel::PassOutcomes,
        DiagnosticChannel::PhaseTransitions,
        DiagnosticChannel::Formation,
        DiagnosticChannel::Performance,
    ];

    /// What the channel is for, phrased as the question it answers.
    pub fn question(self) -> &'static str {
        match self {
            DiagnosticChannel::Possession => "who has the ball, and why did it change hands?",
            DiagnosticChannel::RefereeDecisions => "what did the referee see and decide?",
            DiagnosticChannel::Touches => "who touched the ball, when, and how?",
            DiagnosticChannel::PassOutcomes => "do passes arrive, and where are they lost?",
            DiagnosticChannel::PhaseTransitions => "when did the state of the match change?",
            DiagnosticChannel::Formation => "does the block hold shape, or does everyone chase?",
            DiagnosticChannel::Performance => "what does a tick cost?",
        }
    }

    /// What it costs, so a cheap toggle can be told from one that distorts what
    /// it is measuring.
    pub fn cost(self) -> &'static str {
        match self {
            DiagnosticChannel::Possession => "~20 lines/min",
            DiagnosticChannel::RefereeDecisions => "quiet unless play stops",
            DiagnosticChannel::Touches => "~2 lines/s — the loudest one",
            DiagnosticChannel::PassOutcomes => "~40 lines/min",
            DiagnosticChannel::PhaseTransitions => "a handful per match",
            DiagnosticChannel::Formation => "throttled to 4 Hz",
            DiagnosticChannel::Performance => "throttled to 1 Hz",
        }
    }

    fn index(self) -> usize {
        match self {
            DiagnosticChannel::Possession => 0,
            DiagnosticChannel::RefereeDecisions => 1,
            DiagnosticChannel::Touches => 2,
            DiagnosticChannel::PassOutcomes => 3,
            DiagnosticChannel::PhaseTransitions => 4,
            DiagnosticChannel::Formation => 5,
            DiagnosticChannel::Performance => 6,
        }
    }
}

/// Which channels are currently being asked. Everything off by default.
#[derive(bevy_ecs::prelude::Resource, Debug, Clone, Copy, Default)]
pub struct DiagnosticChannels {
    enabled: [bool; DiagnosticChannel::ALL.len()],
}

impl DiagnosticChannels {
    pub fn is_enabled(&self, channel: DiagnosticChannel) -> bool {
        self.enabled[channel.index()]
    }

    pub fn set(&mut self, channel: DiagnosticChannel, on: bool) {
        self.enabled[channel.index()] = on;
    }

    pub fn toggle(&mut self, channel: DiagnosticChannel) -> bool {
        let now = !self.is_enabled(channel);
        self.set(channel, now);
        now
    }

    pub fn any_enabled(&self) -> bool {
        self.enabled.iter().any(|on| *on)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_on_until_it_is_asked_for() {
        let channels = DiagnosticChannels::default();
        assert!(!channels.any_enabled());
        for channel in DiagnosticChannel::ALL {
            assert!(!channels.is_enabled(channel), "{channel:?} defaults on");
        }
    }

    /// Every channel occupies its own slot: a duplicated index would make two
    /// channels share a switch, and one of them would look permanently stuck.
    #[test]
    fn every_channel_has_its_own_switch() {
        let mut channels = DiagnosticChannels::default();
        for channel in DiagnosticChannel::ALL {
            channels.set(channel, true);
            let on: Vec<_> = DiagnosticChannel::ALL
                .into_iter()
                .filter(|c| channels.is_enabled(*c))
                .collect();
            assert_eq!(on, vec![channel], "{channel:?} shares a switch");
            channels.set(channel, false);
        }
    }
}
