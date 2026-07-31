//! How to watch a simulation nobody is looking at: [`telemetry`] is the fact
//! stream, [`snapshot`] the present as data, [`ledger`] what the facts add up
//! to, so no test accumulates its own totals (`docs/DIAGNOSTICS.md`).
//!
//! Off by default, and here rather than in presentation: seeing is not a
//! property of having a window.

mod collect;
pub mod ledger;
pub mod pitch_view;
pub mod telemetry;

use crate::SimulationSet;
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;

pub use football_domain::diagnostics::{
    DiagnosticChannel, DiagnosticChannels, Field, MatchFact, MatchSnapshot, PossessionCause,
    ReleaseKind, SectionId,
};
pub use ledger::MatchLedger;
pub use pitch_view::render_pitch;
pub use telemetry::{MatchTelemetry, Recorded};

/// Ordering inside the diagnostic tail of the tick. The kernel has already run
/// by this point: everything here reads.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticSet {
    /// Derive totals and any facts that follow from other facts.
    Accumulate,
    /// Render the present into pure data.
    Collect,
    /// Write to whoever is listening.
    Report,
    /// Close the tick. Nothing may read the stream after this.
    Close,
}

/// A live match keeps no history: the console sink reads each tick as it
/// happens, and retention is for a run somebody wants to examine afterwards.
#[derive(Default)]
pub struct MatchDiagnosticsPlugin {
    retention: usize,
}

impl MatchDiagnosticsPlugin {
    /// Keep the last `facts` of the run, so a headless run can be examined once
    /// it finishes.
    pub fn retaining(facts: usize) -> Self {
        Self { retention: facts }
    }
}

impl Plugin for MatchDiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DiagnosticChannels>()
            .init_resource::<MatchLedger>()
            .init_resource::<MatchSnapshot>()
            .insert_resource(MatchTelemetry::retaining(self.retention))
            .configure_sets(
                FixedUpdate,
                (
                    DiagnosticSet::Accumulate,
                    DiagnosticSet::Collect,
                    DiagnosticSet::Report,
                    DiagnosticSet::Close,
                )
                    .chain()
                    .after(SimulationSet::Referee),
            )
            .add_systems(
                FixedUpdate,
                (
                    ledger::accumulate_facts.in_set(DiagnosticSet::Accumulate),
                    collect::collect_snapshot.in_set(DiagnosticSet::Collect),
                    telemetry::write_facts_to_the_log.in_set(DiagnosticSet::Report),
                    telemetry::close_telemetry_tick.in_set(DiagnosticSet::Close),
                ),
            );
    }
}
