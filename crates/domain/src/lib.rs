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
    BallTouched, MatchPhase, MatchRegulations, MatchRng, MatchState, OffsideRecords,
    PenaltyShootout, PitchConfig, PossessionDesignation, PotentialFoul, SetPiece,
};
pub use perception::{
    EXTRAPOLATION_HORIZON, HIDDEN_BLUR, Judgement, Observation, ObservationMemory,
    PerceptionProfile, SHADOW_NEEDS_DEPTH, SHOUTED_BLUR, Senses, TOTAL_LOSS, Vision, can_see,
    hidden_by, perception_profile, perception_profiles,
};
pub use player::{
    Attributes, Card, DefensiveAction, Discipline, FatigueState, Mentality, PLAYER_BODY_RADIUS,
    Player, PlayerMatchState, PlayingPosition, PositionFamiliarity, ResponsibilityKind,
    RoleFamiliarity, TacticalPlan, TacticalPlans, TacticalResponsibility, TacticalRole,
    player_attributes, tactical_familiarity,
};
pub use scenario::{
    CounterfactualOverlay, CounterfactualOverlayAlternative, InitialObservation, MovementProposal,
    ObservationSubject, PlayerPlacement, Scenario, ScenarioOutcome, Substitution,
};
pub use spatial::{Facing, Gaze, Looking, MovementIntent, Position, Stance, Velocity};
pub use tuning::{MatchTuning, TuningVersion};
