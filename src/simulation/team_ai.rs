//! Port of `TeamAIController` (onthepitch/teamAIcontroller.cpp): the per-team
//! tactical brain. Computes possession amounts, the offside trap line, adapted
//! formation positions (`GetAdaptedFormationPosition` + the underlying
//! `AI_GetAdaptedFormationPosition` from AIfunctions.cpp), man marking
//! (`CalculateManMarking` / `CalculateMarkingQuality`), attacking runs and the
//! forward support player.
//!
//! Deliberate simplifications vs the original:
//! - `CalculateDynamicRoles` (Hungarian assignment of formation spots) is NOT
//!   ported; dynamic roles equal static formation roles.
//! - Team tactics are the original's `baseTeamTactics` constants; the per-team
//!   user tactics modifiers (database properties) don't exist here.
//! - `ApplyTeamPressure` is not triggered (its trigger is commented out in the
//!   original as well).

use bevy::prelude::*;
use std::collections::VecDeque;

use crate::data::{
    Ball, MatchState, Player, PlayerRole, Position, PossessionDesignation, SetPiece, Velocity,
};
use crate::math::{
    curve, line_distance_to_point_2d, normalized_clamp, normalized_or_2d, rotated_2d, what_side_2d,
};

// ---------------------------------------------------------------------------
// gamedefines.hpp velocity constants
// ---------------------------------------------------------------------------
// (the switch thresholds are consumed once human controls arrive in Phase 4)
#[allow(dead_code)]
pub const IDLE_VELOCITY: f32 = 0.0;
pub const DRIBBLE_VELOCITY: f32 = 3.5;
pub const WALK_VELOCITY: f32 = 5.0;
pub const SPRINT_VELOCITY: f32 = 8.0;
#[allow(dead_code)]
pub const IDLE_DRIBBLE_SWITCH: f32 = 1.8;
#[allow(dead_code)]
pub const DRIBBLE_WALK_SWITCH: f32 = 4.2;
#[allow(dead_code)]
pub const WALK_SPRINT_SWITCH: f32 = 6.0;
pub const DISTANCE_TO_VELOCITY_MULTIPLIER: f32 = 2.6;
pub const PITCH_HALF_W: f32 = 55.0;
pub const PITCH_HALF_H: f32 = 36.0;

/// The x-side each team defends (original `Team::GetSide()`): team 0 defends
/// x < 0 and attacks +x, so its side is -1.
pub fn team_side(team: u32) -> f32 {
    if team == 0 { -1.0 } else { 1.0 }
}

/// The engine's `clamp()` semantics: applies min first, then max, so an
/// inverted range returns `max` instead of panicking like Rust's `clamp`.
pub fn cpp_clamp(v: f32, min: f32, max: f32) -> f32 {
    let mut v = v;
    if v < min {
        v = min;
    }
    if v > max {
        v = max;
    }
    v
}

// ---------------------------------------------------------------------------
// baseTeamTactics (TeamAIController constructor)
// ---------------------------------------------------------------------------
const OFFENSE_DEPTH_FACTOR: f32 = 0.9;
const DEFENSE_DEPTH_FACTOR: f32 = 0.75;
const OFFENSE_WIDTH_FACTOR: f32 = 0.9;
const DEFENSE_WIDTH_FACTOR: f32 = 0.8;
const OFFENSE_OWNHALF_FACTOR: f32 = 0.52;
const DEFENSE_OWNHALF_FACTOR: f32 = 0.54;
const OFFENSE_MIDFIELDFOCUS: f32 = 0.6;
const DEFENSE_MIDFIELDFOCUS: f32 = 0.5;
const OFFENSE_MIDFIELDFOCUS_STRENGTH: f32 = 0.35;
const DEFENSE_MIDFIELDFOCUS_STRENGTH: f32 = 0.35;
const OFFENSE_SIDEFOCUS_STRENGTH: f32 = 0.1;
const DEFENSE_SIDEFOCUS_STRENGTH: f32 = 0.4;
const OFFENSE_MICROFOCUS_STRENGTH: f32 = 0.7;
const DEFENSE_MICROFOCUS_STRENGTH: f32 = 0.8;
const FORMATION_DEPTH: f32 = 0.45; // TeamAIController::depth
const FORMATION_WIDTH: f32 = 0.95; // TeamAIController::width

/// Per-role offsets on the base tactics (port of `mixup()`).
fn mixup(base: f32, varname: &str, role: PlayerRole) -> f32 {
    let value: Option<f32> = match role {
        PlayerRole::CB => match varname {
            "position_offense_width_factor" => Some(0.2), // wider defense
            _ => None,
        },
        PlayerRole::LB | PlayerRole::RB => match varname {
            "position_defense_ownhalf_factor" => Some(-0.075), // go forward
            "position_offense_width_factor" => Some(0.2),
            "position_offense_ownhalf_factor" => Some(-0.1),
            _ => None,
        },
        PlayerRole::LM | PlayerRole::RM => match varname {
            "position_defense_ownhalf_factor" => Some(-0.05),
            "position_offense_ownhalf_factor" => Some(-0.1),
            _ => None,
        },
        PlayerRole::AM | PlayerRole::CF => match varname {
            "position_defense_depth_factor" => Some(0.125),
            _ => None,
        },
        _ => None,
    };
    match value {
        Some(v) => (base + v).clamp(0.0, 1.0),
        None => base,
    }
}

// ---------------------------------------------------------------------------
// Resource
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TeamAi {
    pub offside_trap_x: f32,
    /// 0 = fully defensive stance, 1 = fully offensive (score/time driven).
    pub offensiveness_bias: f32,
    pub team_possession_amount: f32,
    pub fading_team_possession_amount: f32,
    pub attacking_run_player: Option<Entity>,
    pub end_attacking_run_ms: u64,
    pub forward_support_player: Option<Entity>,
    /// Opponents sorted most dangerous first (port of `tacticalOpponentInfo`).
    pub dangerous_opponents: Vec<Entity>,
}

impl Default for TeamAi {
    fn default() -> Self {
        Self {
            offside_trap_x: 0.0,
            offensiveness_bias: 0.5,
            team_possession_amount: 1.0,
            fading_team_possession_amount: 1.0,
            attacking_run_player: None,
            end_attacking_run_ms: 0,
            forward_support_player: None,
            dangerous_opponents: Vec::new(),
        }
    }
}

#[derive(Resource, Debug, Clone, Default)]
pub struct TeamAis {
    pub team: [TeamAi; 2],
    /// One sample per 10 ms tick of `(fading0 - 0.5) * side0 + (fading1 - 0.5) * side1`
    /// (port of `Match::possessionSideHistory`, 6 s deep).
    pub possession_side_history: VecDeque<f32>,
}

impl TeamAis {
    /// Port of `Match::GetAveragePossessionSide(time_ms)`.
    pub fn average_possession_side(&self, time_ms: f32) -> f32 {
        let mut total = 0u32;
        let mut sum = 0.0;
        for v in self.possession_side_history.iter().rev() {
            sum += *v;
            total += 1;
            if (total * 10) as f32 > time_ms {
                break;
            }
        }
        if total > 0 { sum / total as f32 } else { 0.0 }
    }
}

/// Minimal per-player snapshot shared by the team AI and the Eliza controller.
#[derive(Debug, Clone, Copy)]
pub struct PlayerSnap {
    pub entity: Entity,
    pub team: u32,
    pub role: PlayerRole,
    pub pos: Vec2,
    pub vel: Vec2,
    pub formation_pos: Vec2,
}

/// Closest player of `team` to `position`, excluding `exclude` (port of
/// `AI_GetClosestPlayer`). Set `skip_keeper` to ignore the GK.
pub fn closest_player(
    snaps: &[PlayerSnap],
    team: u32,
    position: Vec2,
    exclude: Option<Entity>,
    skip_keeper: bool,
) -> Option<Entity> {
    let mut best: Option<(Entity, f32)> = None;
    for s in snaps {
        if s.team != team || Some(s.entity) == exclude {
            continue;
        }
        if skip_keeper && s.role == PlayerRole::GK {
            continue;
        }
        let d = s.pos.distance_squared(position);
        if best.is_none_or(|(_, bd)| d < bd) {
            best = Some((s.entity, d));
        }
    }
    best.map(|(e, _)| e)
}

/// The `count` closest players of `team` to `position` (port of
/// `AI_GetClosestPlayers`).
pub fn closest_players(
    snaps: &[PlayerSnap],
    team: u32,
    position: Vec2,
    exclude: Option<Entity>,
    count: usize,
) -> Vec<PlayerSnap> {
    let mut list: Vec<(f32, PlayerSnap)> = snaps
        .iter()
        .filter(|s| s.team == team && Some(s.entity) != exclude)
        .map(|s| (s.pos.distance_squared(position), *s))
        .collect();
    list.sort_by(|a, b| a.0.total_cmp(&b.0));
    list.truncate(count);
    list.into_iter().map(|(_, s)| s).collect()
}

// ---------------------------------------------------------------------------
// The team AI process system (port of TeamAIController::Process)
// ---------------------------------------------------------------------------

pub fn team_ai_update(
    time: Res<Time>,
    match_state: Res<MatchState>,
    designation: Res<PossessionDesignation>,
    mut team_ais: ResMut<TeamAis>,
    ball_query: Query<&Ball>,
    mut player_query: Query<(Entity, &Position, &mut Player, &Velocity)>,
) {
    let Ok(ball) = ball_query.single() else {
        return;
    };
    let now_ms = (time.elapsed_secs_f64() * 1000.0) as u64;
    let in_play = match_state.set_piece == SetPiece::None;

    let snaps: Vec<PlayerSnap> = player_query
        .iter()
        .map(|(entity, position, p, v)| PlayerSnap {
            entity,
            team: p.team_index,
            role: p.role,
            pos: position.on_pitch(),
            vel: Vec2::new(v.0.x, v.0.y),
            formation_pos: p.formation_pos,
        })
        .collect();

    // -- possession amounts (port of Team::Process) --
    for t in 0..2usize {
        let my_time = designation.time_to_ball_ms[t].min(60000.0);
        let opp_time = designation.time_to_ball_ms[1 - t].min(60000.0);
        let tp = (opp_time + 1500.0) / (my_time + 1500.0);
        let ai = &mut team_ais.team[t];
        ai.team_possession_amount = tp;
        if in_play {
            let tmp = ai.fading_team_possession_amount * 0.995 + tp.clamp(0.5, 1.5) * 0.005;
            let delta = (tmp - ai.fading_team_possession_amount).clamp(-0.005, 0.005);
            ai.fading_team_possession_amount += delta;
        } else {
            // during a dead ball, possession belongs to the restarting team
            let amount = if match_state.set_piece_team == Some(t as u32) {
                1.5
            } else {
                0.5
            };
            ai.team_possession_amount = amount;
            ai.fading_team_possession_amount = amount;
        }
    }

    // -- offensiveness bias (simplified UpdateTactics: goals + match time) --
    for t in 0..2usize {
        let (goals, opp_goals) = if t == 0 {
            (match_state.home_score as f32, match_state.away_score as f32)
        } else {
            (match_state.away_score as f32, match_state.home_score as f32)
        };
        let goal_factor = (0.5 + (opp_goals - goals) * 0.25).clamp(0.0, 1.0);
        let time_factor = 0.5 + 0.5 * (now_ms as f32 / 6_300_000.0).clamp(0.0, 1.0);
        let offense_bias = (0.5 + (goal_factor - 0.5) * time_factor).clamp(0.0, 1.0);
        let recent_possession_bias =
            normalized_clamp(team_ais.team[t].fading_team_possession_amount, 0.5, 1.5);
        team_ais.team[t].offensiveness_bias = offense_bias * 0.5 + recent_possession_bias * 0.5;
    }

    // -- possession side history (port of Match::Process) --
    if in_play {
        let sample = (team_ais.team[0].fading_team_possession_amount - 0.5) * team_side(0)
            + (team_ais.team[1].fading_team_possession_amount - 0.5) * team_side(1);
        team_ais.possession_side_history.push_back(sample);
        if team_ais.possession_side_history.len() > 600 {
            team_ais.possession_side_history.pop_front();
        }
    }

    for t in 0..2u32 {
        let side = team_side(t);
        let opp_designated = designation.designated[(1 - t) as usize]
            .and_then(|e| snaps.iter().find(|s| s.entity == e))
            .copied();

        // -- offside trap line (port of the deepestDanger computation) --
        let offensiveness = team_ais.team[t as usize].offensiveness_bias;
        let start_distance = 30.0 + 20.0 * offensiveness;
        let force_distance = 6.0;
        let mut deepest_danger = (PITCH_HALF_W - start_distance) * side;

        // ball as max
        let mut adapted_ball_x = ball.predictions[0].x * side; // > 0 == on our half
        let offset_x = 20.0 + 10.0 * (1.0 - offensiveness);
        let start_to_forced = normalized_clamp(
            adapted_ball_x,
            PITCH_HALF_W - start_distance - offset_x,
            PITCH_HALF_W - force_distance,
        );
        adapted_ball_x += offset_x * (1.0 - start_to_forced);
        adapted_ball_x *= side; // back to absolute space
        if adapted_ball_x * side > deepest_danger * side {
            deepest_danger = adapted_ball_x;
        }

        // ball future as max
        let future_x = ball.predictions[70.min(ball.predictions.len() - 1)].x;
        if future_x * side > deepest_danger * side {
            deepest_danger = future_x;
        }

        // opponent designated possession player as max
        if let Some(opp) = opp_designated {
            let caution = 4.0 * side;
            if (opp.pos.x + opp.vel.x * 0.15 + caution) * side > deepest_danger * side {
                deepest_danger = opp.pos.x + opp.vel.x * 0.1 + caution;
            }
        }

        // slacking teammate as max: our own defensive line (one-but-deepest of us)
        let line_x = own_defensive_line_x(&snaps, t);
        let allow_slack_distance = 4.0;
        if line_x * side - allow_slack_distance > deepest_danger * side {
            deepest_danger = line_x - allow_slack_distance * side;
        }

        team_ais.team[t as usize].offside_trap_x = deepest_danger;

        // -- who's dangerous (port of tacticalOpponentInfo) --
        let most_dangerous_pos = Vec2::new((PITCH_HALF_W - 2.0) * side, 0.0) * 0.8
            + Vec2::new(
                ball.predictions[10.min(ball.predictions.len() - 1)].x,
                ball.predictions[10.min(ball.predictions.len() - 1)].y,
            ) * 0.2;
        let mut danger: Vec<(Entity, f32)> = snaps
            .iter()
            .filter(|s| s.team != t)
            .map(|s| {
                let mut factor = 1.0
                    - normalized_clamp(
                        (s.pos - most_dangerous_pos).length(),
                        0.0,
                        PITCH_HALF_W * 2.0,
                    );
                factor *= 0.95;
                if designation.designated[(1 - t) as usize] == Some(s.entity) {
                    factor += 0.05;
                }
                (s.entity, factor)
            })
            .collect();
        danger.sort_by(|a, b| b.1.total_cmp(&a.1));
        team_ais.team[t as usize].dangerous_opponents = danger.iter().map(|(e, _)| *e).collect();

        // -- attacking runs (every 500 ms) --
        if in_play
            && now_ms % 500 < 10
            && team_ais.team[t as usize].end_attacking_run_ms <= now_ms
            && best_possession_team(&designation) == Some(t)
        {
            if let Some(designated) = designation.designated[t as usize]
                .and_then(|e| snaps.iter().find(|s| s.entity == e))
            {
                // SelectAttackingRunPlayer: closest to a spot 26 m ahead of the ball carrier
                let focus = designated.pos + Vec2::new(-side * 26.0, 0.0);
                if let Some(runner_e) =
                    closest_player(&snaps, t, focus, Some(designated.entity), true)
                {
                    let runner = snaps.iter().find(|s| s.entity == runner_e).unwrap();
                    let distance = (runner.pos - designated.pos).length();
                    let distance_rating = (1.0 - normalized_clamp(distance, 0.0, 40.0)).powf(0.5);

                    let spot =
                        Vec2::new(runner.pos.x, runner.pos.y * 0.8) + Vec2::new(side * 10.0, 0.0);
                    let mut opp_density_rating = 1.0;
                    for opp in closest_players(&snaps, 1 - t, spot, None, 4) {
                        let opp_distance = (opp.pos - spot).length();
                        let inv =
                            curve(1.0 - normalized_clamp(opp_distance, 0.0, 15.0), 1.0).powf(0.5);
                        opp_density_rating -= inv * 0.3;
                    }

                    if distance_rating * opp_density_rating >= 0.5 {
                        team_ais.team[t as usize].end_attacking_run_ms = now_ms + 4000;
                        team_ais.team[t as usize].attacking_run_player = Some(runner_e);
                        debug!("team {t}: tactics induced attacking run");
                    }
                }
            }
        }

        // -- forward support player (every 1500 ms) --
        if now_ms % 1500 < 10 {
            if let Some(designated) = designation.designated[t as usize]
                .and_then(|e| snaps.iter().find(|s| s.entity == e))
            {
                team_ais.team[t as usize].forward_support_player = closest_player(
                    &snaps,
                    t,
                    designated.pos + Vec2::new(-side * 1.5, 0.0),
                    Some(designated.entity),
                    false,
                );
            }
        }
    }

    // -- man marking (port of CalculateManMarking) --
    let mut assignments: Vec<(Entity, Option<Entity>)> =
        snaps.iter().map(|s| (s.entity, None)).collect();
    for t in 0..2u32 {
        let num_marked = 3usize;
        let dangerous = team_ais.team[t as usize].dangerous_opponents.clone();
        let mut available: Vec<&PlayerSnap> = snaps
            .iter()
            .filter(|s| s.team == t && s.role != PlayerRole::GK)
            .collect();
        for opp_entity in dangerous.iter().take(num_marked) {
            let Some(opp) = snaps.iter().find(|s| s.entity == *opp_entity) else {
                continue;
            };
            let mut best: Option<(usize, f32)> = None;
            for (i, marker) in available.iter().enumerate() {
                let quality = marking_quality(marker, opp, t);
                if best.is_none_or(|(_, bq)| quality > bq) {
                    best = Some((i, quality));
                }
            }
            if let Some((i, _)) = best {
                let marker = available.remove(i);
                if let Some(slot) = assignments.iter_mut().find(|(e, _)| *e == marker.entity) {
                    slot.1 = Some(*opp_entity);
                }
            }
            if available.is_empty() {
                break;
            }
        }
    }
    for (entity, marking) in assignments {
        if let Ok((_, _, mut player, _)) = player_query.get_mut(entity) {
            player.man_marking = marking;
        }
    }
}

fn best_possession_team(designation: &PossessionDesignation) -> Option<u32> {
    if designation.time_to_ball_ms[0] <= designation.time_to_ball_ms[1] {
        Some(0)
    } else {
        Some(1)
    }
}

/// One-but-deepest player of `team` on their own defensive side (used for the
/// "slacking teammate" clamp of the trap line).
fn own_defensive_line_x(snaps: &[PlayerSnap], team: u32) -> f32 {
    let side = team_side(team);
    let mut deepest: Option<usize> = None;
    let list: Vec<&PlayerSnap> = snaps.iter().filter(|s| s.team == team).collect();
    for (i, s) in list.iter().enumerate() {
        if deepest.is_none_or(|d| s.pos.x * side > list[d].pos.x * side) {
            deepest = Some(i);
        }
    }
    let mut line = 0.0f32;
    for (i, s) in list.iter().enumerate() {
        if Some(i) == deepest {
            continue;
        }
        if s.pos.x * side > line * side {
            line = s.pos.x;
        }
    }
    line
}

/// Port of `TeamAIController::CalculateMarkingQuality(player, opp)`.
fn marking_quality(marker: &PlayerSnap, opp: &PlayerSnap, team: u32) -> f32 {
    let side = team_side(team);
    let opp_position = opp.pos + opp.vel * 0.1;
    let player_position = marker.pos + marker.vel * 0.1;

    let goal_pos = Vec2::new(PITCH_HALF_W * side, 0.0);
    let to_goal = goal_pos - player_position;
    let line_length = to_goal.length().clamp(4.0, 14.0);
    let to_goal_norm = normalized_or_2d(to_goal, Vec2::new(side, 0.0));
    let safety_vec = -to_goal_norm * 0.5;
    let half_pi = std::f32::consts::FRAC_PI_2;
    let v0 = player_position + safety_vec + rotated_2d(to_goal_norm, -half_pi) * line_length;
    let v1 = player_position + safety_vec + rotated_2d(to_goal_norm, half_pi) * line_length;

    let opp_is_on_right_side = what_side_2d(v0, v1, opp_position);
    let (opp_from_line_distance, u) = line_distance_to_point_2d(v0, v1, opp_position);

    let adapted = if opp_is_on_right_side {
        (opp_from_line_distance - 2.0).abs()
    } else {
        opp_from_line_distance
    };

    let opp_from_line_factor = normalized_clamp(adapted, 0.0, 60.0).powf(0.5);
    let opp_on_line_factor = (u * 2.0 - 1.0).abs().clamp(0.0, 1.0).powf(0.5);

    let mut result: f32 = 1.0;
    result -= opp_from_line_factor * 0.5;
    result -= opp_on_line_factor * 0.5;
    result = result.clamp(0.0, 1.0);

    if !opp_is_on_right_side {
        result *= 0.6; // he's passed us already
    }

    let opp_distance = 1.0
        - normalized_clamp(
            (player_position - opp_position).length(),
            0.0,
            PITCH_HALF_W * 2.0,
        );
    result * 0.8 + opp_distance * 0.2
}

// ---------------------------------------------------------------------------
// Adapted formation position (GetAdaptedFormationPosition +
// AI_GetAdaptedFormationPosition)
// ---------------------------------------------------------------------------

/// Port of `TeamAIController::GetAdaptedFormationPosition(player)`. `focal_point`
/// is the designated possession player's position (either team).
pub fn get_adapted_formation_position(
    team_ais: &TeamAis,
    team: u32,
    player_pos: Vec2,
    formation_pos: Vec2,
    role: PlayerRole,
    focal_point: Vec2,
    ball: &Ball,
) -> Vec2 {
    let side = team_side(team);
    let ai = &team_ais.team[team as usize];

    let urgency_bias = 1.0 - normalized_clamp((focal_point - player_pos).length(), 2.0, 30.0);
    let ball_x = ball.average_position(3500.0 * (1.0 - urgency_bias * 0.7)).x;
    let ball_y = ball.average_position(4000.0 * (1.0 - urgency_bias * 0.5)).y;

    let offense_depth = mixup(OFFENSE_DEPTH_FACTOR, "position_offense_depth_factor", role);
    let defense_depth = mixup(DEFENSE_DEPTH_FACTOR, "position_defense_depth_factor", role);
    let offense_width = mixup(OFFENSE_WIDTH_FACTOR, "position_offense_width_factor", role);
    let defense_width = mixup(DEFENSE_WIDTH_FACTOR, "position_defense_width_factor", role);
    let offense_ownhalf = mixup(
        OFFENSE_OWNHALF_FACTOR,
        "position_offense_ownhalf_factor",
        role,
    );
    let defense_ownhalf = mixup(
        DEFENSE_OWNHALF_FACTOR,
        "position_defense_ownhalf_factor",
        role,
    );
    let offense_midfocus = mixup(
        OFFENSE_MIDFIELDFOCUS,
        "position_offense_midfieldfocus",
        role,
    );
    let defense_midfocus = mixup(
        DEFENSE_MIDFIELDFOCUS,
        "position_defense_midfieldfocus",
        role,
    );
    let offense_midfocus_str = mixup(
        OFFENSE_MIDFIELDFOCUS_STRENGTH,
        "position_offense_midfieldfocus_strength",
        role,
    );
    let defense_midfocus_str = mixup(
        DEFENSE_MIDFIELDFOCUS_STRENGTH,
        "position_defense_midfieldfocus_strength",
        role,
    );
    let mut offense_sidefocus_str = mixup(
        OFFENSE_SIDEFOCUS_STRENGTH,
        "position_offense_sidefocus_strength",
        role,
    );
    let mut defense_sidefocus_str = mixup(
        DEFENSE_SIDEFOCUS_STRENGTH,
        "position_defense_sidefocus_strength",
        role,
    );
    let offense_microfocus_str = mixup(
        OFFENSE_MICROFOCUS_STRENGTH,
        "position_offense_microfocus_strength",
        role,
    );
    let defense_microfocus_str = mixup(
        DEFENSE_MICROFOCUS_STRENGTH,
        "position_defense_microfocus_strength",
        role,
    );

    let mind_set = role.mind_set();
    offense_sidefocus_str += (-0.5 + mind_set) * 0.2;
    defense_sidefocus_str += (0.5 - mind_set) * 0.2;
    offense_sidefocus_str =
        (offense_sidefocus_str - 0.3 + ai.offensiveness_bias * 0.3).clamp(0.0, 1.0);
    defense_sidefocus_str =
        (defense_sidefocus_str - 0.3 + (1.0 - ai.offensiveness_bias) * 0.3).clamp(0.0, 1.0);

    let possession_amount_bias = normalized_clamp(ai.fading_team_possession_amount - 0.5, 0.3, 0.7);
    let ball_bias = normalized_clamp((ball_x / PITCH_HALF_W) * -side, -0.7, 0.7);
    let mut ball_bias_bias = 1.0 - (possession_amount_bias * 2.0 - 1.0).abs();
    ball_bias_bias *= 0.6;
    let mut possession_bias =
        possession_amount_bias * (1.0 - ball_bias_bias) + ball_bias * ball_bias_bias;
    possession_bias = (possession_bias + (ai.offensiveness_bias - 0.5) * 0.3).clamp(0.0, 1.0);

    let avg3000 = ball.average_position(3000.0);
    let avg2000 = ball.average_position(2000.0);
    let focal = Vec2::new(avg3000.x, avg3000.y) * possession_bias
        + (Vec2::new(avg2000.x, avg2000.y) * 0.5 + focal_point * 0.5) * (1.0 - possession_bias);

    let adapted_depth = FORMATION_DEPTH
        * (offense_depth * possession_bias + defense_depth * (1.0 - possession_bias));
    let adapted_width = FORMATION_WIDTH
        * (offense_width * possession_bias + defense_width * (1.0 - possession_bias));

    let offset_x = PITCH_HALF_W
        * side
        * ((offense_ownhalf * 2.0 - 1.0) * possession_bias
            + (defense_ownhalf * 2.0 - 1.0) * (1.0 - possession_bias));

    let side_focus_strength =
        offense_sidefocus_str * possession_bias + defense_sidefocus_str * (1.0 - possession_bias);
    let side_focus = possession_bias * 2.0 - 1.0;

    let avg_possession_side = team_ais.average_possession_side(6000.0);
    let side_x =
        0.2 * side_focus * -side * PITCH_HALF_W + 0.8 * -avg_possession_side * PITCH_HALF_W;
    let mut center_x = ((ball_x * (1.0 - side_focus_strength) + side_x * side_focus_strength)
        + offset_x)
        .clamp(-PITCH_HALF_W, PITCH_HALF_W);
    let mut center_y = ball_y.clamp(-PITCH_HALF_H, PITCH_HALF_H);

    let adapt_center_depth_bias = 0.95;
    let adapt_center_width_bias = 0.9;
    center_x *= (1.0 - adapted_depth) * adapt_center_depth_bias + (1.0 - adapt_center_depth_bias);
    center_y *= (1.0 - adapted_width) * adapt_center_width_bias + (1.0 - adapt_center_width_bias);

    let mut back_x_bound = center_x - adapted_depth * PITCH_HALF_W * -side;
    let front_x_bound = center_x + adapted_depth * PITCH_HALF_W * -side;
    let low_y_bound = center_y - adapted_width * PITCH_HALF_H;
    let high_y_bound = center_y + adapted_width * PITCH_HALF_H;

    if back_x_bound * side > ai.offside_trap_x * side {
        back_x_bound = ai.offside_trap_x;
    }

    let y_focus = ball_y;
    let y_focus_strength = 0.5 * possession_bias + 0.2 * (1.0 - possession_bias);

    let micro_focus_base = focal;
    let defensive_focus_pos = Vec2::new(
        // original: clamp(microFocus.x + side * 2, -pitchHalfW, backXBound * side);
        // C++ clamp tolerates max < min (returns max), Rust's panics — replicate
        cpp_clamp(
            micro_focus_base.x + side * 2.0,
            -PITCH_HALF_W,
            back_x_bound * side,
        ),
        micro_focus_base.y * 0.9,
    );
    let micro_focus = Vec2::new(
        (micro_focus_base.x - side * 1.0).clamp(-PITCH_HALF_W, PITCH_HALF_W),
        micro_focus_base.y * 0.9,
    ) * possession_bias
        + defensive_focus_pos * (1.0 - possession_bias);
    let mut micro_focus_strength =
        offense_microfocus_str * possession_bias + defense_microfocus_str * (1.0 - possession_bias);

    let mut micro_focus_side_bias = normalized_clamp((ball_x / PITCH_HALF_W) * -side, -0.7, 0.7);
    micro_focus_side_bias = micro_focus_side_bias * 0.7 + 0.3;
    let auto_micro_focus_strength = micro_focus_side_bias.powf(0.8) * possession_bias
        + (1.0 - micro_focus_side_bias).powf(0.6) * (1.0 - possession_bias);
    micro_focus_strength *= 0.2 + 0.8 * auto_micro_focus_strength;

    let manual_midfield_focus =
        offense_midfocus * possession_bias + defense_midfocus * (1.0 - possession_bias);
    let auto_midfield_focus = normalized_clamp((ball_x / PITCH_HALF_W) * -side, -0.8, 0.8);
    let midfield_focus = manual_midfield_focus * 0.7 + auto_midfield_focus * 0.3;
    let midfield_focus_strength =
        offense_midfocus_str * possession_bias + defense_midfocus_str * (1.0 - possession_bias);

    let desired = adapted_formation_position(
        formation_pos,
        side,
        back_x_bound,
        front_x_bound,
        low_y_bound,
        high_y_bound,
        0.0,
        0.0,
        y_focus,
        y_focus_strength,
        micro_focus,
        micro_focus_strength,
        midfield_focus,
        midfield_focus_strength,
    );

    Vec2::new(
        desired.x.clamp(-PITCH_HALF_W, PITCH_HALF_W),
        desired.y.clamp(-PITCH_HALF_H, PITCH_HALF_H),
    )
}

/// Port of `AI_GetAdaptedFormationPosition` (aifunctions.cpp): maps the
/// normalized formation entry into the [backX..frontX] × [lowY..highY] block,
/// then applies the x/y/micro/midfield focus magnets.
#[allow(clippy::too_many_arguments)]
pub fn adapted_formation_position(
    formation_pos: Vec2,
    side: f32,
    back_x_bound: f32,
    front_x_bound: f32,
    low_y_bound: f32,
    high_y_bound: f32,
    x_focus: f32,
    x_focus_strength: f32,
    y_focus: f32,
    y_focus_strength: f32,
    micro_focus: Vec2,
    micro_focus_strength: f32,
    midfield_focus: f32,
    midfield_focus_strength: f32,
) -> Vec2 {
    let pi = std::f32::consts::PI;
    let mut position = formation_pos;

    // stretch midfield into defending or attack position
    if midfield_focus_strength > 0.0 {
        let midfield_position_factor = midfield_focus * 2.0 - 1.0;
        let mut stretch_bias = (1.0 - (position.x * 1.2).abs()).clamp(0.0, 1.0);
        stretch_bias = curve(stretch_bias, 1.0);
        stretch_bias *= midfield_focus_strength;
        position.x = position.x * (1.0 - stretch_bias) + midfield_position_factor * stretch_bias;
    }

    let x_length = front_x_bound - back_x_bound;
    position.x = back_x_bound + (position.x * 0.5 + 0.5) * x_length;
    let y_length = high_y_bound - low_y_bound;
    position.y = low_y_bound + (position.y * -side * 0.5 + 0.5) * y_length;

    let pure_position = position;

    if x_focus_strength > 0.0 {
        let mut bias = 1.0
            - ((x_focus - position.x).abs() / (back_x_bound - front_x_bound).abs()).clamp(0.0, 1.0);
        bias = -(bias * pi).cos() * 0.5 + 0.5;
        bias = bias.powf(0.8);
        bias *= x_focus_strength;
        position.x = position.x * (1.0 - bias) + x_focus * bias;
    }

    if y_focus_strength > 0.0 {
        let distance =
            ((y_focus - position.y).abs() / (high_y_bound - low_y_bound).abs()).clamp(0.0, 1.0);
        let mut bias = 1.0 - distance;
        bias *= 0.2 + 0.8 * y_focus.abs() / PITCH_HALF_H;
        bias *= y_focus_strength;
        position.y = position.y * (1.0 - bias) + y_focus * bias;
    }

    if micro_focus_strength > 0.0 {
        let homogeneous_y_influence_bias = 0.2;
        let homogeneous_y_position_bias = 0.4;

        let delta =
            (micro_focus - pure_position) * Vec2::new(1.0, 1.0 - homogeneous_y_influence_bias);
        let dist = delta.length() / 50.0;

        if dist < 1.0 {
            let mut micro_focus_bias = 1.0 - dist;
            micro_focus_bias = curve(micro_focus_bias, 0.3);

            // extra short distance peak
            let peak_location = 0.15;
            let peak_width = 0.25;
            let peak_height = 0.1;
            micro_focus_bias += (1.0
                - normalized_clamp((dist - peak_location).abs(), 0.0, peak_width))
                * peak_height;
            micro_focus_bias = micro_focus_bias.clamp(0.0, 1.0);

            micro_focus_bias *= micro_focus_strength;

            let micro_focus_position = micro_focus
                * Vec2::new(1.0, 1.0 - homogeneous_y_position_bias)
                + position * Vec2::new(0.0, homogeneous_y_position_bias);
            position =
                position * (1.0 - micro_focus_bias) + micro_focus_position * micro_focus_bias;
        }
    }

    position
}

/// Port of `TeamAIController::ApplyOffsideTrap(position)` (smooth version).
pub fn apply_offside_trap(team_ais: &TeamAis, team: u32, position: &mut Vec2) {
    let side = team_side(team);
    let offside_trap_x = team_ais.team[team as usize].offside_trap_x;

    let area_half_length = 2.0;
    let abs_pos_x = position.x * side;
    let abs_offside_trap_x = offside_trap_x * side;

    if abs_pos_x > abs_offside_trap_x - area_half_length {
        let area_front = abs_offside_trap_x - area_half_length;
        let pos_from_area_front = abs_pos_x - area_front;
        let pos_factor = (pos_from_area_front / (area_half_length * 2.0)).clamp(0.0, 1.0);
        let abs_result = area_front + area_half_length * pos_factor;
        position.x = abs_result * side;
    }
}
