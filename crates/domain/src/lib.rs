//! Domain: the vocabulary of the simulated game.
//!
//! Types, units, rules, facts, intents and configuration. Everything here is
//! data: no system, no plugin and no engine subsystem beyond ECS and maths.

pub mod ball;
pub mod match_state;
pub mod math;
pub mod player;
pub mod scenario;
pub mod spatial;

pub use ball::{BALL_HISTORY_STEPS, BALL_PREDICTION_STEPS, BALL_RADIUS, Ball};
pub use match_state::{
    BallTouched, MatchPhase, MatchRegulations, MatchRng, MatchState, OffsideRecords, PitchConfig,
    PossessionDesignation, SetPiece,
};
pub use player::{Player, PlayerRole, PlayerStats};
pub use scenario::{Scenario, ScenarioOutcome};
pub use spatial::{Facing, Position, Velocity};
