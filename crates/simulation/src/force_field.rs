//! Where a player wants to stand, as a sum of attractors and repellers.
//!
//! Port of `ForceSpot` and `AI_GetForceFieldMovement`. Both the off-ball
//! positioning of `player_decisions` and the carrier's dribble in
//! `player_movement` resolve the same field; only what they do with the result
//! differs.

use bevy_math::Vec2;

/// How a spot's pull fades with distance.
///
/// A variant carries only the numbers its own shape needs, so a constant spot
/// cannot claim a radius it never uses (§10).
#[derive(Debug, Clone, Copy)]
pub enum Falloff {
    /// Full intensity everywhere; the spot pulls the same from any distance.
    Constant,
    /// Fades linearly, reaching zero `radius` metres away.
    Linear { radius: f32 },
    /// Linear fade bent by an exponent: below 1.0 it holds its grip longer.
    Curved { radius: f32, exponent: f32 },
}

impl Falloff {
    fn intensity_at(self, distance: f32) -> f32 {
        match self {
            Self::Constant => 1.0,
            Self::Linear { radius } => Self::linear(distance, radius),
            Self::Curved { radius, exponent } => Self::linear(distance, radius).powf(exponent),
        }
    }

    fn linear(distance: f32, radius: f32) -> f32 {
        (1.0 - distance / radius).clamp(0.0, 1.0)
    }
}

pub struct ForceSpot {
    pub origin: Vec2,
    pub repel: bool,
    pub power: f32,
    pub falloff: Falloff,
}

/// Weighted mean direction of the whole field, or `None` when no spot reaches
/// `at` — the caller decides what standing in a dead field means.
///
/// Attractors closer than `attractor_damping_distance` lose pull, so a player
/// settles onto the spot instead of orbiting it.
pub fn resolve(field: &[ForceSpot], at: Vec2, attractor_damping_distance: f32) -> Option<Vec2> {
    let mut cumulative_direction = Vec2::ZERO;
    let mut cumulative_force = 0.0;

    for spot in field {
        let distance = spot.origin.distance(at);
        let intensity = spot.falloff.intensity_at(distance);
        if intensity <= 0.0 {
            continue;
        }

        let mut direction = (spot.origin - at).normalize_or_zero();
        if spot.repel {
            direction = -direction;
        } else if distance < attractor_damping_distance {
            direction *= distance / attractor_damping_distance;
        }

        let force = spot.power * intensity;
        cumulative_direction += direction * force;
        cumulative_force += force;
    }

    (cumulative_force > 0.0).then(|| cumulative_direction / cumulative_force)
}
