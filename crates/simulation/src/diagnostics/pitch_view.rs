//! Seeing the shape of a match without a window.
//!
//! The cheapest possible view: one character per body on a grid. It answers the
//! one question a stream of numbers cannot — is that a team block or is
//! everybody chasing the ball? It lives here rather than inside a test because
//! any headless run should be able to ask for it.

use bevy_ecs::prelude::*;
use football_domain::{Ball, Player, Position, TeamId};

const COLUMNS: usize = 92;
const ROWS: usize = 25;
/// The drawn area extends a metre past the touchlines, so a ball that has just
/// gone out is still visible instead of clamping onto the line.
const HALF_WIDTH: f32 = 56.0;
const HALF_HEIGHT: f32 = 37.0;

/// The pitch as text: `o` home, `x` away, `@` ball.
pub fn render_pitch(world: &mut World) -> String {
    let mut grid = vec![vec![' '; COLUMNS]; ROWS];

    let cell = |x: f32, y: f32| -> (usize, usize) {
        let column = ((x + HALF_WIDTH) / (HALF_WIDTH * 2.0) * (COLUMNS as f32 - 1.0)).round();
        let row = ((y + HALF_HEIGHT) / (HALF_HEIGHT * 2.0) * (ROWS as f32 - 1.0)).round();
        (
            (column as isize).clamp(0, COLUMNS as isize - 1) as usize,
            (row as isize).clamp(0, ROWS as isize - 1) as usize,
        )
    };

    let (left, top) = cell(-55.0, -36.0);
    let (right, bottom) = cell(55.0, 36.0);
    let (halfway, _) = cell(0.0, 0.0);
    for touchline in [top, bottom] {
        for cell in &mut grid[touchline][left..=right] {
            *cell = '-';
        }
    }
    for row in grid.iter_mut().take(bottom + 1).skip(top) {
        row[left] = '|';
        row[right] = '|';
        row[halfway] = ':';
    }

    let mut players = world.query::<(&Position, &Player)>();
    for (position, player) in players.iter(world) {
        let (column, row) = cell(position.0.x, position.0.y);
        grid[row][column] = match player.id.team {
            TeamId::Home => 'o',
            TeamId::Away => 'x',
        };
    }

    let mut ball = world.query_filtered::<&Position, With<Ball>>();
    if let Ok(position) = ball.single(world) {
        let (column, row) = cell(position.0.x, position.0.y);
        grid[row][column] = '@';
    }

    grid.into_iter()
        .map(|row| row.into_iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_world_still_draws_a_pitch() {
        let mut world = World::new();
        let drawn = render_pitch(&mut world);
        let lines: Vec<&str> = drawn.lines().collect();

        assert_eq!(lines.len(), ROWS);
        assert!(lines.iter().any(|line| line.contains("---")), "no touchline");
        assert!(lines.iter().any(|line| line.contains(':')), "no halfway line");
        assert!(!drawn.contains('@'), "a ball that does not exist was drawn");
    }
}
