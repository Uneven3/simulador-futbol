use crate::data::{
    Ball, BallTouched, MatchState, OffsideRecords, PitchConfig, Player, SetPiece, Velocity,
};
use crate::simulation::SimulationSet;
use crate::simulation::player_movement::get_base_formation_position;
use bevy::prelude::*;

pub struct RefereePlugin;

impl Plugin for RefereePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(OffsideRecords::default()).add_systems(
            FixedUpdate,
            (
                referee_offside_system,
                referee_system,
                referee_set_piece_system,
            )
                .chain()
                .in_set(SimulationSet::Referee),
        );
    }
}

/// Port of `Match::CheckForGoal(side)`: swept segment (previous → current ball
/// position) against the goal mouth plane at x = ±(pitchHalfW + lineHalfW + 0.11),
/// i.e. the whole ball must cross the outer edge of the line. `side` is -1 for
/// the left goal, 1 for the right goal.
fn check_for_goal(
    side: f32,
    prev: Vec3,
    current: Vec3,
    predict_10ms: Vec3,
    pitch: &PitchConfig,
) -> bool {
    if predict_10ms.x.abs() < pitch.half_width - 1.0 {
        return false;
    }

    let plane_x = (pitch.half_width + pitch.line_half_width + 0.11) * side;

    // segment must cross the plane going outward
    let d_prev = prev.x - plane_x;
    let d_curr = current.x - plane_x;
    if !(d_prev * side <= 0.0 && d_curr * side > 0.0) {
        return false;
    }
    let denom = current.x - prev.x;
    if denom.abs() < 1e-9 {
        return false;
    }
    let t = (plane_x - prev.x) / denom;
    let hit = prev + (current - prev) * t;
    if hit.y.abs() >= pitch.goal_half_width || hit.z <= 0.0 || hit.z >= pitch.goal_height {
        return false;
    }

    // extra check: ball could have gone 'in' via the side netting, if segment
    // begin == outside of post but already behind the line. disallow!
    if prev.y.abs() > pitch.goal_half_width
        && prev.x.abs() > pitch.half_width - pitch.line_half_width - 0.11
    {
        return false;
    }

    true
}

fn referee_system(
    mut match_state: ResMut<MatchState>,
    pitch_config: Res<PitchConfig>,
    ball_query: Single<(&Transform, &Ball)>,
) {
    // If a set piece is already pending/ticking down, don't check for new events
    if match_state.set_piece != SetPiece::None {
        return;
    }

    let (transform, ball) = *ball_query;

    let pos = transform.translation;
    let prev = ball.previous_position;
    let predict_10ms = ball.predictions[1];
    let pitch_half_w = pitch_config.half_width;
    let pitch_half_h = pitch_config.half_height;
    let line_half_w = pitch_config.line_half_width;

    // 1. Goal Detection (swept, per goal side)
    if !match_state.is_ball_in_goal {
        for side in [-1.0f32, 1.0] {
            if check_for_goal(side, prev, pos, predict_10ms, &pitch_config) {
                if side < 0.0 {
                    // Away team scores (in the left goal, defended by home)
                    match_state.away_score += 1;
                    match_state.set_piece_team = Some(0); // home team kicks off after conceding
                } else {
                    match_state.home_score += 1;
                    match_state.set_piece_team = Some(1);
                }
                match_state.is_ball_in_goal = true;
                match_state.set_piece = SetPiece::KickOff;
                // original: 6000 ms celebration + 2000 ms preparation
                match_state.set_piece_timer = 8.0;
                match_state.restart_pos = Vec3::ZERO;
                info!(
                    "GOAL! Score: {} - {}",
                    match_state.home_score, match_state.away_score
                );
                return;
            }
        }
    }

    // Out-of-play detection: the whole ball must be past the outer edge of the
    // line (original referee.cpp: fabs(pos) > pitchHalf + lineHalfW + 0.11)
    let last_touch = ball.last_touch_team.unwrap_or(0);
    // side of the pitch the last touching team defends (-1 left for home)
    let last_side: f32 = if last_touch == 0 { -1.0 } else { 1.0 };

    // 2. Over the backline: corner or goal kick
    if pos.x.abs() > pitch_half_w + line_half_w + 0.11 {
        debug!(
            "BACKLINE CROSS at y={:.2} z={:.2} (goal mouth: |y|<3.7, z<2.5)",
            pos.y, pos.z
        );
        let taking_team = 1 - last_touch;
        if pos.x * last_side > 0.0 {
            // last touch by the team defending this side -> corner for the attackers
            match_state.set_piece = SetPiece::Corner;
            match_state.set_piece_team = Some(taking_team);
            match_state.set_piece_timer = 4.0;
            match_state.restart_pos = Vec3::new(
                pitch_half_w * last_side,
                if pos.y > 0.0 {
                    pitch_half_h
                } else {
                    -pitch_half_h
                },
                0.0,
            );
            info!(
                "Ball out over the backline: corner kick for team {}",
                taking_team
            );
        } else {
            match_state.set_piece = SetPiece::GoalKick;
            match_state.set_piece_team = Some(taking_team);
            match_state.set_piece_timer = 4.0;
            match_state.restart_pos = Vec3::new(pitch_half_w * 0.92 * -last_side, 0.0, 0.0);
            info!(
                "Ball out over the backline: goal kick for team {}",
                taking_team
            );
        }
    }
    // 3. Over the sideline: throw-in
    else if pos.y.abs() > pitch_half_h + line_half_w + 0.11 {
        let throw_in_team = 1 - last_touch;
        match_state.set_piece = SetPiece::ThrowIn;
        match_state.set_piece_team = Some(throw_in_team);
        match_state.set_piece_timer = 4.0;
        match_state.restart_pos = Vec3::new(
            pos.x.clamp(-pitch_half_w + 0.6, pitch_half_w - 0.6),
            if pos.y > 0.0 {
                pitch_half_h
            } else {
                -pitch_half_h
            },
            0.0,
        );
        info!(
            "Ball out over the sideline: throw-in for team {}",
            throw_in_team
        );
    }
}

/// Port of `Referee::BallTouched()`: on every touch, first whistle if the toucher
/// was recorded offside at the previous touch, then re-record teammates standing
/// beyond the offside line (`AI_GetOffsideLine`) at this moment.
fn referee_offside_system(
    mut match_state: ResMut<MatchState>,
    mut records: ResMut<OffsideRecords>,
    pitch_config: Res<PitchConfig>,
    mut touches: MessageReader<BallTouched>,
    player_query: Query<(Entity, &Transform, &Player)>,
    ball_query: Single<&Transform, With<Ball>>,
) {
    for touch in touches.read() {
        if match_state.set_piece != SetPiece::None {
            records.players.clear();
            records.team = None;
            continue;
        }

        // offside player receiving the ball?
        if records.team == Some(touch.team) {
            if let Some((_, recorded_pos)) = records
                .players
                .iter()
                .find(|(entity, _)| *entity == touch.player)
                .copied()
            {
                match_state.set_piece = SetPiece::FreeKick;
                match_state.set_piece_team = Some(1 - touch.team);
                match_state.set_piece_timer = 4.0;
                match_state.restart_pos = Vec3::new(recorded_pos.x, recorded_pos.y, 0.0);
                records.players.clear();
                records.team = None;
                info!("OFFSIDE! Free kick for team {}", 1 - touch.team);
                continue;
            }
        }

        records.players.clear();
        records.team = Some(touch.team);

        // AI_GetOffsideLine: one-but-deepest defender, or the ball, whichever is
        // deeper; never inside the attackers' own half.
        let defending_team = 1 - touch.team;
        let def_side: f32 = if defending_team == 0 { -1.0 } else { 1.0 };

        let mut deepest: Option<(Entity, f32)> = None;
        for (entity, transform, player) in player_query.iter() {
            if player.team_index != defending_team {
                continue;
            }
            let depth = transform.translation.x * def_side;
            if deepest.is_none() || depth > deepest.unwrap().1 {
                deepest = Some((entity, depth));
            }
        }
        let mut second_deepest_x = 0.0f32;
        for (entity, transform, player) in player_query.iter() {
            if player.team_index != defending_team || Some(entity) == deepest.map(|d| d.0) {
                continue;
            }
            if transform.translation.x * def_side > second_deepest_x * def_side {
                second_deepest_x = transform.translation.x;
            }
        }

        let mut offside_line = second_deepest_x;
        let ball_x = ball_query.translation.x;
        if ball_x * def_side > offside_line * def_side {
            offside_line = ball_x;
        }
        if offside_line * def_side < 0.0 {
            offside_line = 0.01 * -def_side;
        }
        offside_line = offside_line.clamp(-pitch_config.half_width, pitch_config.half_width);

        let att_dir = -def_side;
        for (entity, transform, player) in player_query.iter() {
            if player.team_index != touch.team || entity == touch.player {
                continue;
            }
            let pos = transform.translation;
            if pos.x * att_dir > offside_line * att_dir + 0.20 {
                records.players.push((entity, pos));
            }
        }
    }
}

/// Reset System:
/// Ticks down the set piece timer. When it expires, places the ball at the
/// restart position recorded when play was stopped, teleports players to their
/// base positions and resets all state (original: `ResetSituation`).
fn referee_set_piece_system(
    mut match_state: ResMut<MatchState>,
    mut records: ResMut<OffsideRecords>,
    time: Res<Time>,
    mut ball_query: Query<(&mut Transform, &mut Ball), Without<Player>>,
    mut player_query: Query<(&mut Transform, &mut Velocity, &mut Player), Without<Ball>>,
) {
    if match_state.set_piece == SetPiece::None {
        return;
    }

    if match_state.set_piece_timer > 0.0 {
        match_state.set_piece_timer -= time.delta_secs();
        // dead ball: park it at the restart spot right away, or it keeps
        // rolling into the stands while the restart timer runs
        if let Ok((mut ball_transform, mut ball)) = ball_query.single_mut() {
            let restart_pos = match_state.restart_pos;
            if ball_transform.translation.distance(restart_pos) > 0.3 {
                ball.reset(restart_pos);
                ball_transform.translation = restart_pos + Vec3::new(0.0, 0.0, 0.11);
                ball_transform.rotation = Quat::IDENTITY;
            }
        }
        return;
    }

    // Timer expired! Execute reset.
    let Ok((mut ball_transform, mut ball)) = ball_query.single_mut() else {
        return;
    };

    let restart_pos = match_state.restart_pos;
    ball.reset(restart_pos);
    ball_transform.translation = restart_pos + Vec3::new(0.0, 0.0, 0.11);
    ball_transform.rotation = Quat::IDENTITY;

    // Teleport players to base positions
    for (mut transform, mut velocity, mut player) in player_query.iter_mut() {
        let base_pos = get_base_formation_position(player.team_index, player.role, player.id);
        let height = player.height;
        transform.translation = Vec3::new(base_pos.x, base_pos.y, height / 2.0);
        velocity.0 = Vec3::ZERO;
        player.last_touch_time_ms = 0;
    }

    // Clear set piece state
    let prev_set_piece = match_state.set_piece;
    match_state.set_piece = SetPiece::None;
    match_state.set_piece_team = None;
    match_state.is_ball_in_goal = false;
    match_state.possession_player = None;
    match_state.possession_team = None;
    match_state.previous_possessor = None;
    match_state.last_possession_change_time = 0;
    records.players.clear();
    records.team = None;

    info!("Play restarted after {:?}", prev_set_piece);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_goal_requires_whole_ball_over_the_line() {
        let pitch = PitchConfig::default();
        let far_predict = Vec3::new(56.0, 0.0, 0.11);

        // ball center barely past the line but not fully over: no goal yet
        let prev = Vec3::new(54.8, 0.0, 0.5);
        let current = Vec3::new(55.05, 0.0, 0.5);
        assert!(!check_for_goal(1.0, prev, current, far_predict, &pitch));

        // whole ball (center past 55 + 0.06 + 0.11) crossed: goal
        let current = Vec3::new(55.3, 0.0, 0.5);
        assert!(check_for_goal(1.0, prev, current, far_predict, &pitch));
    }

    #[test]
    fn test_fast_shot_does_not_tunnel() {
        let pitch = PitchConfig::default();
        let far_predict = Vec3::new(60.0, 0.0, 0.11);
        // 40 m/s shot moves 0.4 m per 10 ms tick: an instantaneous check could
        // miss the mouth, the swept segment must not
        let prev = Vec3::new(54.9, 1.0, 1.0);
        let current = Vec3::new(55.6, 1.05, 1.0);
        assert!(check_for_goal(1.0, prev, current, far_predict, &pitch));
    }

    #[test]
    fn test_no_goal_through_side_netting() {
        let pitch = PitchConfig::default();
        let far_predict = Vec3::new(56.0, 0.0, 0.11);
        // segment starting almost on the line and outside the post, crossing the
        // plane inside the mouth: the original explicitly disallows this
        let prev = Vec3::new(55.16, 3.8, 0.5);
        let current = Vec3::new(55.30, 2.0, 0.5);
        assert!(!check_for_goal(1.0, prev, current, far_predict, &pitch));
    }

    #[test]
    fn test_shot_over_the_bar_is_not_a_goal() {
        let pitch = PitchConfig::default();
        let far_predict = Vec3::new(56.0, 0.0, 3.5);
        let prev = Vec3::new(54.0, 0.0, 3.4);
        let current = Vec3::new(55.5, 0.0, 3.3);
        assert!(!check_for_goal(1.0, prev, current, far_predict, &pitch));
    }
}
