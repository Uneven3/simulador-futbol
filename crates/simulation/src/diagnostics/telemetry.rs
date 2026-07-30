//! The per-tick fact stream.
//!
//! Separate from the snapshot on purpose: a snapshot is the present, and it
//! would only ever keep the last of a sequence. These describe moments, so they
//! are appended and read in order — by a console sink, by an overlay, or by a
//! test that wants to count what a match did without running its own bookkeeping.

use bevy_ecs::prelude::*;
use football_domain::diagnostics::{DiagnosticChannel, DiagnosticChannels, MatchFact};
use std::collections::VecDeque;

/// A fact and the fixed tick it happened on. The tick is the correlation key:
/// without it, two channels enabled at once cannot be read against each other.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Recorded {
    pub tick: u64,
    pub fact: MatchFact,
}

/// What the match reported this tick, and optionally a window of history.
///
/// Recording is unconditional and allocation-free after warm-up: facts are
/// `Copy` and the tick buffer is reused. What is conditional is what happens to
/// them — the console sink only writes enabled channels, and history is only
/// kept when someone asked for it.
#[derive(Resource, Debug, Default)]
pub struct MatchTelemetry {
    tick: u64,
    this_tick: Vec<MatchFact>,
    history: VecDeque<Recorded>,
    /// How many facts to keep. Zero means the stream is live-only.
    retention: usize,
}

impl MatchTelemetry {
    /// Keep the last `facts` recorded, so a run can be examined after it ends.
    /// A headless test enables this; a running match has no reason to.
    pub fn retaining(facts: usize) -> Self {
        Self {
            retention: facts,
            ..Default::default()
        }
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// Report something that happened. Called from the kernel systems; ordering
    /// within a tick follows system order, which is itself fixed.
    pub fn record(&mut self, fact: MatchFact) {
        self.this_tick.push(fact);
    }

    /// What was reported during the tick currently being processed.
    pub fn this_tick(&self) -> &[MatchFact] {
        &self.this_tick
    }

    /// Everything retained, oldest first.
    pub fn history(&self) -> impl DoubleEndedIterator<Item = &Recorded> {
        self.history.iter()
    }

    /// Facts of one kind, for a consumer that only cares about one question.
    pub fn recorded_on(
        &self,
        channel: DiagnosticChannel,
    ) -> impl DoubleEndedIterator<Item = &Recorded> {
        self.history
            .iter()
            .filter(move |recorded| recorded.fact.channel() == channel)
    }

    /// Closes the tick: retains what was asked for and clears the live buffer.
    /// The counter advances here, so everything recorded during a tick shares
    /// one number.
    fn close_tick(&mut self) {
        if self.retention > 0 {
            for fact in self.this_tick.drain(..) {
                if self.history.len() == self.retention {
                    self.history.pop_front();
                }
                self.history.push_back(Recorded {
                    tick: self.tick,
                    fact,
                });
            }
        } else {
            self.this_tick.clear();
        }
        self.tick += 1;
    }
}

/// Console sink: writes the tick's facts for the channels someone asked about.
///
/// The only formatting happens here. A producer that formatted its own line
/// would be a second sink with its own idea of the truth.
pub(super) fn write_facts_to_the_log(
    channels: Res<DiagnosticChannels>,
    telemetry: Res<MatchTelemetry>,
) {
    if !channels.any_enabled() {
        return;
    }
    for fact in telemetry.this_tick() {
        if channels.is_enabled(fact.channel()) {
            bevy_log::info!("[t{:06}] {fact}", telemetry.tick());
        }
    }
}

/// Runs last in the tick, after every sink has read the stream.
pub(super) fn close_telemetry_tick(mut telemetry: ResMut<MatchTelemetry>) {
    telemetry.close_tick();
}

#[cfg(test)]
mod tests {
    use super::*;
    use football_domain::{MatchPhase, PlayerId};

    fn a_phase(phase: MatchPhase) -> MatchFact {
        MatchFact::PhaseEntered(phase)
    }

    #[test]
    fn facts_of_one_tick_share_a_tick_number() {
        let mut telemetry = MatchTelemetry::retaining(16);
        telemetry.record(a_phase(MatchPhase::FirstHalf));
        telemetry.record(MatchFact::Touched {
            player: PlayerId::home(9),
            deliberate: true,
        });
        telemetry.close_tick();
        telemetry.record(a_phase(MatchPhase::HalfTime));
        telemetry.close_tick();

        let ticks: Vec<u64> = telemetry.history().map(|r| r.tick).collect();
        assert_eq!(ticks, vec![0, 0, 1]);
    }

    /// A match runs for hundreds of thousands of ticks, so retention has to be
    /// a window and not a leak.
    #[test]
    fn history_is_a_window_not_a_leak() {
        let mut telemetry = MatchTelemetry::retaining(2);
        for _ in 0..10 {
            telemetry.record(a_phase(MatchPhase::FirstHalf));
            telemetry.close_tick();
        }
        assert_eq!(telemetry.history().count(), 2);
        assert_eq!(telemetry.history().next().unwrap().tick, 8);
    }

    #[test]
    fn without_retention_nothing_survives_the_tick() {
        let mut telemetry = MatchTelemetry::default();
        telemetry.record(a_phase(MatchPhase::FirstHalf));
        assert_eq!(telemetry.this_tick().len(), 1);
        telemetry.close_tick();
        assert_eq!(telemetry.this_tick().len(), 0);
        assert_eq!(telemetry.history().count(), 0);
    }
}
