//! Domain: the vocabulary of the simulated game.
//!
//! Types, units, rules, facts, intents and configuration. Everything here is
//! data: no system, no plugin and no engine subsystem beyond ECS and maths.

pub mod ball;
pub mod diagnostics;
pub mod identity;
pub mod match_state;
pub mod math;
pub mod perception;
pub mod player;
pub mod scenario;
pub mod spatial;
pub mod tuning;

pub use ball::{BALL_HISTORY_STEPS, BALL_PREDICTION_STEPS, BALL_RADIUS, Ball};
pub use identity::{ByTeam, PitchSides, PlayerId, PlayerRegistry, TeamId, TeamSide};
pub use match_state::{
    BallTouched, MatchPhase, MatchRegulations, MatchRng, MatchState, OffsideRecords, PitchConfig,
    PossessionDesignation, PotentialFoul, SetPiece,
};
pub use perception::{
    EXTRAPOLATION_HORIZON, Observation, ObservationMemory, SHADOW_NEEDS_DEPTH, TOTAL_LOSS, Vision,
    blocks_the_view, can_see,
};
pub use player::{
    Attributes, FatigueState, Mentality, PLAYER_BODY_RADIUS, Player, PlayerMatchState,
    PlayingPosition, TacticalRole,
};
pub use scenario::{Scenario, ScenarioOutcome};
pub use spatial::{Facing, Gaze, Looking, MovementIntent, Position, Stance, Velocity};
pub use tuning::{MatchTuning, TuningVersion};
