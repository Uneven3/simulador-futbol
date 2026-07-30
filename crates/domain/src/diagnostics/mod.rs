//! The vocabulary of watching a match.
//!
//! Facts and the snapshot are data, so they live where every layer can read
//! them: the kernel reports, the HUD draws and the console writes, and none of
//! the three needs to know about the others. What is *not* here is any
//! production or sinking of them — that is `football_simulation::diagnostics`.

pub mod facts;
pub mod snapshot;

pub use facts::{
    DiagnosticChannel, DiagnosticChannels, MatchFact, PossessionCause, ReleaseKind,
};
pub use snapshot::{Field, MatchSnapshot, SectionId};
