//! Qué marco táctico usa cada jugador: forma base, estado de bloque y
//! responsabilidades nacidas de sus creencias. El proceso global heredado aún
//! calcula la forma del bloque; la responsabilidad individual no arbitra un
//! reparto con el mapa verdadero.
//!
//! Ported from `TeamAIController` (onthepitch/teamAIcontroller.cpp). Dynamic
//! roles equal static ones — `CalculateDynamicRoles` is not ported — and the
//! tactics are the original's `baseTeamTactics` with no user modifiers.

use crate::perception::Beliefs;
use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use std::collections::VecDeque;

use football_domain::math::{curve, normalized_clamp};
use football_domain::{
    Ball, ByTeam, MatchState, PitchSides, Player, PlayerId, PlayingPosition, PositionFamiliarity,
    PossessionDesignation, ResponsibilityKind, RoleFamiliarity, SetPiece, TacticalPlans,
    TacticalResponsibility, TacticalRole, TeamId,
};

// ---------------------------------------------------------------------------
// gamedefines.hpp velocity constants
// ---------------------------------------------------------------------------
// (the switch thresholds are consumed once human controls arrive in Phase 4)
pub const IDLE_VELOCITY: f32 = 0.0;
pub const DRIBBLE_VELOCITY: f32 = 3.5;
pub const WALK_VELOCITY: f32 = 5.0;
pub const SPRINT_VELOCITY: f32 = 8.0;
pub const IDLE_DRIBBLE_SWITCH: f32 = 1.8;
pub const DRIBBLE_WALK_SWITCH: f32 = 4.2;
pub const WALK_SPRINT_SWITCH: f32 = 6.0;
pub const DISTANCE_TO_VELOCITY_MULTIPLIER: f32 = 2.6;
pub const PITCH_HALF_W: f32 = 55.0;
pub const PITCH_HALF_H: f32 = 36.0;

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
fn mixup(base: f32, varname: &str, role: PlayingPosition) -> f32 {
    let value: Option<f32> = match role {
        PlayingPosition::CentreBack => match varname {
            "position_offense_width_factor" => Some(0.2), // wider defense
            _ => None,
        },
        PlayingPosition::LeftBack | PlayingPosition::RightBack => match varname {
            "position_defense_ownhalf_factor" => Some(-0.075), // go forward
            "position_offense_width_factor" => Some(0.2),
            "position_offense_ownhalf_factor" => Some(-0.1),
            _ => None,
        },
        PlayingPosition::LeftMidfielder | PlayingPosition::RightMidfielder => match varname {
            "position_defense_ownhalf_factor" => Some(-0.05),
            "position_offense_ownhalf_factor" => Some(-0.1),
            _ => None,
        },
        PlayingPosition::AttackingMidfielder | PlayingPosition::CentreForward => match varname {
            "position_defense_depth_factor" => Some(0.125),
            _ => None,
        },
        PlayingPosition::Goalkeeper
        | PlayingPosition::DefensiveMidfielder
        | PlayingPosition::CentreMidfielder => None,
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
pub struct TeamShape {
    /// 0 = fully defensive stance, 1 = fully offensive (score/time driven).
    pub offensiveness_bias: f32,
    pub team_possession_amount: f32,
    pub fading_team_possession_amount: f32,
}

impl Default for TeamShape {
    fn default() -> Self {
        Self {
            offensiveness_bias: 0.5,
            team_possession_amount: 1.0,
            fading_team_possession_amount: 1.0,
        }
    }
}

#[derive(Resource, Debug, Clone, Default)]
pub struct TeamTactics {
    pub team: ByTeam<TeamShape>,
    /// One sample per 10 ms tick of `(fading0 - 0.5) * side0 + (fading1 - 0.5) * side1`
    /// (port of `Match::possessionSideHistory`, 6 s deep).
    pub possession_side_history: VecDeque<f32>,
}

impl TeamTactics {
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

/// What the decision systems read about one player this tick. No longer the
/// truth: `perception::Beliefs` fills these from what each player has seen, so
/// two of them can disagree about where a third one is.
#[derive(Debug, Clone, Copy)]
pub struct PlayerReading {
    pub id: PlayerId,
    pub playing_position: PlayingPosition,
    pub role: TacticalRole,
    pub pos: Vec2,
    pub vel: Vec2,
    pub formation_slot: Vec2,
    /// Cuántos metros puede estar equivocado `pos`, según quien lo cree. Cero es
    /// «lo estoy viendo» —y es lo que vale para quien lee la verdad del mundo en
    /// vez de una creencia—, y crece con lo que hace que uno mire.
    pub doubt: f32,
}

/// Publica una responsabilidad por jugador desde lo que él cree haber visto.
/// No arbitra un reparto perfecto con el mapa real: si dos defensores perdieron
/// al mismo rival pueden cubrirlo los dos, que es justo el fenómeno que la
/// percepción parcial tiene que poder producir.
pub fn assign_perceived_responsibilities(
    match_state: Res<MatchState>,
    plans: Res<TacticalPlans>,
    beliefs: Res<Beliefs>,
    mut players: Query<(
        &Player,
        &PositionFamiliarity,
        &RoleFamiliarity,
        &mut TacticalResponsibility,
    )>,
) {
    for (player, position_familiarity, role_familiarity, mut responsibility) in players.iter_mut() {
        let base = plans.for_team(player.id.team);
        let familiarity = (position_familiarity.0 * role_familiarity.0).clamp(0.0, 1.0);
        let plan = football_domain::TacticalPlan {
            defensive_line_depth: base.defensive_line_depth,
            press_distance: base.press_distance * familiarity,
            cover_distance: base.cover_distance * familiarity,
            support_distance: base.support_distance * role_familiarity.0.clamp(0.0, 1.0),
        };
        *responsibility = responsibility_from_readings(
            player,
            beliefs.of(player.id),
            beliefs.ball_of(player.id),
            match_state.sides,
            plan,
        );
    }
}

fn responsibility_from_readings(
    player: &Player,
    readings: &[PlayerReading],
    ball: Option<Vec2>,
    sides: PitchSides,
    plan: football_domain::TacticalPlan,
) -> TacticalResponsibility {
    let Some(me) = readings.iter().find(|reading| reading.id == player.id) else {
        return TacticalResponsibility::default();
    };
    if player.position == PlayingPosition::Goalkeeper {
        return TacticalResponsibility::default();
    }

    let side = sides.defending_x(player.id.team);
    let goal = Vec2::new(PITCH_HALF_W * side, 0.0);
    let opponent = readings
        .iter()
        .filter(|reading| reading.team() != player.id.team)
        .min_by(|left, right| {
            let left_threat = left.pos.distance(goal) + left.pos.distance(me.pos) * 0.4;
            let right_threat = right.pos.distance(goal) + right.pos.distance(me.pos) * 0.4;
            left_threat.total_cmp(&right_threat)
        });

    if let Some(opponent) = opponent
        && opponent.pos.distance(me.pos) <= plan.cover_distance
    {
        return TacticalResponsibility {
            kind: if opponent.pos.distance(me.pos) <= plan.press_distance {
                ResponsibilityKind::Press
            } else {
                ResponsibilityKind::Cover
            },
            target: Some(opponent.id),
        };
    }

    let support = ball.and_then(|ball| {
        readings
            .iter()
            .filter(|reading| reading.team() == player.id.team && reading.id != player.id)
            .min_by(|left, right| left.pos.distance(ball).total_cmp(&right.pos.distance(ball)))
    });
    if let Some(teammate) = support
        && teammate.pos.distance(me.pos) <= plan.support_distance
    {
        return TacticalResponsibility {
            kind: ResponsibilityKind::Support,
            target: Some(teammate.id),
        };
    }
    TacticalResponsibility::default()
}

impl PlayerReading {
    pub fn team(&self) -> TeamId {
        self.id.team
    }
}

/// Closest player of `team` to `position`, excluding `exclude` (port of
/// `AI_GetClosestPlayer`). Set `skip_keeper` to ignore the GK.
pub fn closest_player(
    snaps: &[PlayerReading],
    team: TeamId,
    position: Vec2,
    exclude: Option<PlayerId>,
    skip_keeper: bool,
) -> Option<PlayerId> {
    let mut best: Option<(PlayerId, f32)> = None;
    for s in snaps {
        if s.team() != team || Some(s.id) == exclude {
            continue;
        }
        if skip_keeper && s.playing_position == PlayingPosition::Goalkeeper {
            continue;
        }
        let d = s.pos.distance_squared(position);
        if best.is_none_or(|(_, bd)| d < bd) {
            best = Some((s.id, d));
        }
    }
    best.map(|(id, _)| id)
}

/// The `count` closest players of `team` to `position` (port of
/// `AI_GetClosestPlayers`).
pub fn closest_players(
    snaps: &[PlayerReading],
    team: TeamId,
    position: Vec2,
    exclude: Option<PlayerId>,
    count: usize,
) -> Vec<PlayerReading> {
    let mut list: Vec<(f32, PlayerReading)> = snaps
        .iter()
        .filter(|s| s.team() == team && Some(s.id) != exclude)
        .map(|s| (s.pos.distance_squared(position), *s))
        .collect();
    list.sort_by(|a, b| a.0.total_cmp(&b.0));
    list.truncate(count);
    list.into_iter().map(|(_, s)| s).collect()
}

// ---------------------------------------------------------------------------
// The team AI process system (port of TeamAIController::Process)
// ---------------------------------------------------------------------------

pub fn update_team_tactics(
    match_state: Res<MatchState>,
    designation: Res<PossessionDesignation>,
    mut tactics: ResMut<TeamTactics>,
) {
    let in_play = match_state.set_piece == SetPiece::None;
    let sides = match_state.sides;

    // -- possession amounts (port of Team::Process) --
    for t in TeamId::BOTH {
        let my_time = designation.time_to_ball_ms[t].min(60000.0);
        let opp_time = designation.time_to_ball_ms[t.opponent()].min(60000.0);
        let tp = (opp_time + 1500.0) / (my_time + 1500.0);
        let ai = &mut tactics.team[t];
        ai.team_possession_amount = tp;
        if in_play {
            let tmp = ai.fading_team_possession_amount * 0.995 + tp.clamp(0.5, 1.5) * 0.005;
            let delta = (tmp - ai.fading_team_possession_amount).clamp(-0.005, 0.005);
            ai.fading_team_possession_amount += delta;
        } else {
            // during a dead ball, possession belongs to the restarting team
            let amount = if match_state.set_piece_team == Some(t) {
                1.5
            } else {
                0.5
            };
            ai.team_possession_amount = amount;
            ai.fading_team_possession_amount = amount;
        }
    }

    // -- offensiveness bias (simplified UpdateTactics: goals + match time) --
    for t in TeamId::BOTH {
        let (goals, opp_goals) = if t == TeamId::Home {
            (match_state.home_score as f32, match_state.away_score as f32)
        } else {
            (match_state.away_score as f32, match_state.home_score as f32)
        };
        let goal_factor = (0.5 + (opp_goals - goals) * 0.25).clamp(0.0, 1.0);
        let time_factor =
            0.5 + 0.5 * (match_state.period_elapsed.as_secs_f32() / 6_300.0).clamp(0.0, 1.0);
        let offense_bias = (0.5 + (goal_factor - 0.5) * time_factor).clamp(0.0, 1.0);
        let recent_possession_bias =
            normalized_clamp(tactics.team[t].fading_team_possession_amount, 0.5, 1.5);
        tactics.team[t].offensiveness_bias = offense_bias * 0.5 + recent_possession_bias * 0.5;
    }

    // -- possession side history (port of Match::Process) --
    if in_play {
        let sample = (tactics.team[TeamId::Home].fading_team_possession_amount - 0.5)
            * sides.defending_x(TeamId::Home)
            + (tactics.team[TeamId::Away].fading_team_possession_amount - 0.5)
                * sides.defending_x(TeamId::Away);
        tactics.possession_side_history.push_back(sample);
        if tactics.possession_side_history.len() > 600 {
            tactics.possession_side_history.pop_front();
        }
    }
}

/// One-but-deepest player of `team` on their own defensive side (used for the
/// "slacking teammate" clamp of the trap line).
fn own_defensive_line_x(snaps: &[PlayerReading], team: TeamId, sides: PitchSides) -> f32 {
    let side = sides.defending_x(team);
    let mut deepest: Option<usize> = None;
    let list: Vec<&PlayerReading> = snaps.iter().filter(|s| s.team() == team).collect();
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

// ---------------------------------------------------------------------------
// Adapted formation position (GetAdaptedFormationPosition +
// AI_GetAdaptedFormationPosition)
// ---------------------------------------------------------------------------

/// Who the block is being adapted for: where he is now and what he was asked to
/// be. The four travel together because none of them means anything alone.
#[derive(Debug, Clone, Copy)]
pub struct AdaptedFor {
    pub player_pos: Vec2,
    pub formation_pos: Vec2,
    pub playing_position: PlayingPosition,
    pub role: TacticalRole,
}

/// Port of `TeamAIController::GetAdaptedFormationPosition(player)`. `focal_point`
/// is the designated possession player's position (either team).
pub fn get_adapted_formation_position(
    tactics: &TeamTactics,
    team: TeamId,
    sides: PitchSides,
    offside_trap_x: f32,
    player: AdaptedFor,
    focal_point: Vec2,
    ball: &Ball,
) -> Vec2 {
    let AdaptedFor {
        player_pos,
        formation_pos,
        playing_position,
        role,
    } = player;
    let side = sides.defending_x(team);
    let ai = &tactics.team[team];

    let urgency_bias = 1.0 - normalized_clamp((focal_point - player_pos).length(), 2.0, 30.0);
    let ball_x = ball.average_position(3500.0 * (1.0 - urgency_bias * 0.7)).x;
    let ball_y = ball.average_position(4000.0 * (1.0 - urgency_bias * 0.5)).y;

    let offense_depth = mixup(
        OFFENSE_DEPTH_FACTOR,
        "position_offense_depth_factor",
        playing_position,
    );
    let defense_depth = mixup(
        DEFENSE_DEPTH_FACTOR,
        "position_defense_depth_factor",
        playing_position,
    );
    let offense_width = mixup(
        OFFENSE_WIDTH_FACTOR,
        "position_offense_width_factor",
        playing_position,
    );
    let defense_width = mixup(
        DEFENSE_WIDTH_FACTOR,
        "position_defense_width_factor",
        playing_position,
    );
    let offense_ownhalf = mixup(
        OFFENSE_OWNHALF_FACTOR,
        "position_offense_ownhalf_factor",
        playing_position,
    );
    let defense_ownhalf = mixup(
        DEFENSE_OWNHALF_FACTOR,
        "position_defense_ownhalf_factor",
        playing_position,
    );
    let offense_midfocus = mixup(
        OFFENSE_MIDFIELDFOCUS,
        "position_offense_midfieldfocus",
        playing_position,
    );
    let defense_midfocus = mixup(
        DEFENSE_MIDFIELDFOCUS,
        "position_defense_midfieldfocus",
        playing_position,
    );
    let offense_midfocus_str = mixup(
        OFFENSE_MIDFIELDFOCUS_STRENGTH,
        "position_offense_midfieldfocus_strength",
        playing_position,
    );
    let defense_midfocus_str = mixup(
        DEFENSE_MIDFIELDFOCUS_STRENGTH,
        "position_defense_midfieldfocus_strength",
        playing_position,
    );
    let mut offense_sidefocus_str = mixup(
        OFFENSE_SIDEFOCUS_STRENGTH,
        "position_offense_sidefocus_strength",
        playing_position,
    );
    let mut defense_sidefocus_str = mixup(
        DEFENSE_SIDEFOCUS_STRENGTH,
        "position_defense_sidefocus_strength",
        playing_position,
    );
    let offense_microfocus_str = mixup(
        OFFENSE_MICROFOCUS_STRENGTH,
        "position_offense_microfocus_strength",
        playing_position,
    );
    let defense_microfocus_str = mixup(
        DEFENSE_MICROFOCUS_STRENGTH,
        "position_defense_microfocus_strength",
        playing_position,
    );

    let mind_set = role.attacking_bias();
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

    let avg_possession_side = tactics.average_possession_side(6000.0);
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

    if back_x_bound * side > offside_trap_x * side {
        back_x_bound = offside_trap_x;
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

/// Same smoothing law, but the line is supplied by the deciding player's
/// tactical frame rather than necessarily by the shared historical resource.
pub fn apply_offside_trap_at(
    offside_trap_x: f32,
    team: TeamId,
    sides: PitchSides,
    position: &mut Vec2,
) {
    let side = sides.defending_x(team);

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

/// The defensive line as *one player* can reconstruct it from the teammates
/// they remember.  It deliberately accepts no world query or `TeamTactics`:
/// an unseen centre-back cannot silently hold the line for the observer.
pub fn perceived_offside_trap_x(
    readings: &[PlayerReading],
    observer: PlayerId,
    believed_ball_x: f32,
    sides: PitchSides,
) -> f32 {
    let team = observer.team;
    let side = sides.defending_x(team);
    let own_line = own_defensive_line_x(readings, team, sides);
    // A line never leaves the ball behind from the observer's perspective.
    if believed_ball_x * side > own_line * side {
        believed_ball_x
    } else {
        own_line
    }
}

/// The line a team agrees before play starts. It is deliberately independent
/// of bodies and ball: until communication provides enough sightings, it is
/// the lawful fallback for a defender's local tactical frame.
pub fn planned_defensive_line_x(depth: f32, team: TeamId, sides: PitchSides) -> f32 {
    let side = sides.defending_x(team);
    // 0 = 42 m from halfway towards the own goal; 1 = 8 m into the other half.
    (-42.0 + depth.clamp(0.0, 1.0) * 50.0) * -side
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(id: PlayerId, pos: Vec2) -> PlayerReading {
        PlayerReading {
            id,
            playing_position: PlayingPosition::CentreMidfielder,
            role: TacticalRole::Linking,
            pos,
            vel: Vec2::ZERO,
            formation_slot: Vec2::ZERO,
            doubt: 0.0,
        }
    }

    #[test]
    fn a_perceived_line_uses_only_the_readings_the_observer_has() {
        let seen = [
            reading(PlayerId::home(3), Vec2::new(-35.0, 0.0)),
            reading(PlayerId::home(4), Vec2::new(-30.0, 4.0)),
        ];
        let line = perceived_offside_trap_x(&seen, PlayerId::home(3), -20.0, PitchSides::opening());
        assert!((line + 30.0).abs() < f32::EPSILON);
    }

    #[test]
    fn a_responsibility_only_covers_an_opponent_the_player_has_seen() {
        let me = Player::new(
            PlayerId::home(6),
            PlayingPosition::CentreMidfielder,
            Vec2::ZERO,
        );
        let unseen = PlayerId::away(9);
        let seen = PlayerId::away(10);
        let readings = [
            reading(me.id, Vec2::ZERO),
            reading(seen, Vec2::new(-12.0, 0.0)),
        ];

        let responsibility = responsibility_from_readings(
            &me,
            &readings,
            None,
            PitchSides::opening(),
            football_domain::TacticalPlan::default(),
        );

        assert_eq!(responsibility.target, Some(seen));
        assert_ne!(responsibility.target, Some(unseen));
        assert_eq!(responsibility.kind, ResponsibilityKind::Cover);
    }

    #[test]
    fn the_team_plan_switches_between_press_and_occupation() {
        let me = Player::new(
            PlayerId::home(6),
            PlayingPosition::CentreMidfielder,
            Vec2::ZERO,
        );
        let readings = [
            reading(me.id, Vec2::ZERO),
            reading(PlayerId::away(10), Vec2::new(-12.0, 0.0)),
        ];
        let press = football_domain::TacticalPlan {
            press_distance: 15.0,
            ..Default::default()
        };
        let occupy = football_domain::TacticalPlan {
            cover_distance: 10.0,
            ..Default::default()
        };

        assert_eq!(
            responsibility_from_readings(&me, &readings, None, PitchSides::opening(), press).kind,
            ResponsibilityKind::Press
        );
        assert_eq!(
            responsibility_from_readings(&me, &readings, None, PitchSides::opening(), occupy).kind,
            ResponsibilityKind::Occupy
        );
    }
}
