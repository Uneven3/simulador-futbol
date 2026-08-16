//! App: composition and lifecycle.
//!
//! Nothing here decides anything about football. It assembles the authoritative
//! kernel, optionally attaches presentation, and runs a scenario to its end.

pub mod scenarios;

use bevy::prelude::*;
use football_presentation::{
    DebugHubPlugin, DiagnosticOverlaysPlugin, LowPolyVisualsPlugin, MatchHudPlugin,
    PrimitiveVisualsPlugin,
};

pub use football_domain::scenario;
pub use football_domain::{
    ByTeam, MatchPhase, MatchState, PlayerId, Scenario, ScenarioOutcome, SetPiece, TeamId,
};
pub use football_simulation::ScenarioRunner;

/// El mismo escenario con las primitivas puestas encima.
///
/// Las dos formas de correr una situación siguen siendo la misma corrida: esto
/// parte de `ScenarioRunner::headless` y solo deja mirar a la presentación, así
/// que cualquier divergencia entre las dos es un fallo de presentación y no una
/// diferencia de configuración. Los assets se registran sin renderer —la
/// presentación necesita dónde poner las mallas, no una ventana—, y por eso
/// esto sigue corriendo en un test o en una máquina sin pantalla.
///
/// Es una función y no un constructor de `ScenarioRunner` porque el runner vive
/// una capa más abajo, donde no se conoce la presentación. Que este sea el único
/// sitio del proyecto que enlaza el renderer es justo lo que se busca.
pub fn with_primitives(scenario: Scenario) -> ScenarioRunner {
    let mut runner = ScenarioRunner::headless(scenario);
    let app = runner.app_mut();
    app.add_plugins(AssetPlugin::default());
    app.init_asset::<Mesh>();
    app.init_asset::<StandardMaterial>();
    app.add_plugins((
        PrimitiveVisualsPlugin,
        DiagnosticOverlaysPlugin,
        DebugHubPlugin,
        MatchHudPlugin,
    ));
    runner
}

/// The same headless match with the skinned low-poly presentation attached.
/// As with [`with_primitives`], this composition must never alter a verdict.
pub fn with_low_poly(scenario: Scenario) -> ScenarioRunner {
    let mut runner = ScenarioRunner::headless(scenario);
    let app = runner.app_mut();
    app.add_plugins(AssetPlugin::default());
    app.init_asset::<Mesh>();
    app.init_asset::<StandardMaterial>();
    app.init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();
    app.add_plugins((
        LowPolyVisualsPlugin,
        DiagnosticOverlaysPlugin,
        DebugHubPlugin,
        MatchHudPlugin,
    ));
    runner
}
