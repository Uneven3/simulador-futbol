//! Presentation: how a match is shown, never how it happens.
//!
//! Every entity created here is a disposable representation linked to an
//! authoritative body with `VisualOf`. This layer reads domain state and writes
//! only its own transforms, meshes and materials; it decides no rule and
//! corrects no state.

pub mod camera;
pub mod lighting;
pub mod pitch_mesh;
pub mod visuals;

pub use camera::GameCameraPlugin;
pub use lighting::StadiumLightingPlugin;
pub use pitch_mesh::PitchMeshPlugin;
pub use visuals::{PrimitiveVisualsPlugin, VisualOf};
