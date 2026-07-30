use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum PlayerRole {
    GK, // Goalkeeper
    CB, // Center back
    LB, // Left back
    RB, // Right back
    DM, // Defensive midfielder
    CM, // Center midfielder
    LM, // Left midfielder
    RM, // Right midfielder
    AM, // Attacking midfielder
    CF, // Center forward
}

impl PlayerRole {
    /// Port of `AI_GetMindSet` (aifunctions.cpp): 0 = fully defensive-minded,
    /// 1 = fully offensive-minded. Drives laziness, hunt ranges and pass bias.
    pub fn mind_set(self) -> f32 {
        match self {
            PlayerRole::GK | PlayerRole::CB => 0.0,
            PlayerRole::LB | PlayerRole::RB | PlayerRole::DM => 0.25,
            PlayerRole::LM | PlayerRole::CM | PlayerRole::RM => 0.5,
            PlayerRole::AM => 0.75,
            PlayerRole::CF => 1.0,
        }
    }
}

#[derive(Component, Debug, Clone, Reflect)]
pub struct Player {
    pub id: u32,
    pub team_index: u32, // 0 for home, 1 for away
    pub jersey_number: u32,
    pub role: PlayerRole,
    pub height: f32,
    /// Last time this player touched the ball (for collision/touch bias windows).
    pub last_touch_time_ms: u64,
    /// Normalized formation position (original `FormationEntry::position`):
    /// x in -1..1 (own goal → forward), y in -1..1 (team-space left → right).
    pub formation_pos: Vec2,
    /// Opponent this player man-marks (original `Player::manMarkingID`, assigned
    /// by the team AI's `CalculateManMarking` every tick).
    pub man_marking: Option<Entity>,
    /// ~10 s average of this player's speed (original `GetAverageVelocity(10)`),
    /// used by `GetLazyVelocity` to simulate catching one's breath.
    pub avg_velocity: f32,
}

#[derive(Component, Debug, Clone, Reflect)]
pub struct PlayerStats {
    /// Top speed in m/s (original `sprintVelocity` = 8.0).
    pub speed: f32,
    pub stamina: f32,
    pub acceleration: f32,
    pub agility: f32,
    /// Original stat `mental_workrate` (0..1), used by `GetLazyVelocity`.
    pub work_rate: f32,
    /// Original stat `technical_shot` (0..1): higher = tighter shot placement.
    pub technical_shot: f32,
}

impl Default for PlayerStats {
    fn default() -> Self {
        Self {
            speed: 8.0,   // sprintVelocity (gamedefines.hpp)
            stamina: 1.0, // fully fit (0.0 to 1.0)
            acceleration: 0.5,
            agility: 0.5,
            work_rate: 0.5,
            technical_shot: 0.5,
        }
    }
}
