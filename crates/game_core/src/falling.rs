//! Deterministic falling-group movement over a frozen rules board.

use crate::{
    board::{Board, Cell, Coord},
    input::{GameAction, PlayerActions},
};

/// One colored ball relative to a group's pivot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupBall {
    pub dx: i8,
    pub dy: i8,
    pub color: u8,
}

/// A not-yet-committed group of one or more colored balls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FallingGroup {
    pivot: Coord,
    balls: Vec<GroupBall>,
    turn_id: u32,
}

/// Failed group construction or atomic placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FallingError {
    #[error("a falling group must contain at least one ball")]
    Empty,
    #[error("falling group contains duplicate relative cells")]
    DuplicateCell,
    #[error("falling group does not fit the board at its requested position")]
    InvalidPlacement,
}

impl FallingGroup {
    /// Creates a group after rejecting duplicate local coordinates.
    pub fn new(pivot: Coord, balls: Vec<GroupBall>, turn_id: u32) -> Result<Self, FallingError> {
        if balls.is_empty() {
            return Err(FallingError::Empty);
        }
        let mut cells = balls
            .iter()
            .map(|ball| (ball.dx, ball.dy))
            .collect::<Vec<_>>();
        cells.sort_unstable();
        if cells.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(FallingError::DuplicateCell);
        }
        Ok(Self {
            pivot,
            balls,
            turn_id,
        })
    }

    /// Turn for which this group was generated.
    #[must_use]
    pub const fn turn_id(&self) -> u32 {
        self.turn_id
    }

    /// Current pivot coordinate.
    #[must_use]
    pub const fn pivot(&self) -> Coord {
        self.pivot
    }

    /// Current occupied cells, if they fit this board.
    #[must_use]
    pub fn cells(&self, board: &Board) -> Option<Vec<(Coord, u8)>> {
        self.cells_at(board, self.pivot, &self.balls)
    }

    /// Whether the group can be introduced without a spawn collision.
    #[must_use]
    pub fn can_place(&self, board: &Board) -> bool {
        self.cells(board).is_some_and(|cells| {
            cells
                .into_iter()
                .all(|(coord, _)| !board.get(coord).is_occupied())
        })
    }

    /// Applies one normalized control tick in horizontal, rotation, then drop order.
    ///
    /// A blocked operation is a no-op. Hard drop locks atomically and returns
    /// the committed coordinates; other operations retain the active group.
    pub fn apply_actions(
        &mut self,
        board: &mut Board,
        actions: PlayerActions,
    ) -> Result<Option<Vec<Coord>>, FallingError> {
        let actions = actions.normalized();
        if actions.contains(GameAction::Left) {
            self.try_translate(board, -1, 0);
        } else if actions.contains(GameAction::Right) {
            self.try_translate(board, 1, 0);
        }
        if actions.contains(GameAction::RotateClockwise) {
            self.try_rotate(board, true);
        } else if actions.contains(GameAction::RotateCounterClockwise) {
            self.try_rotate(board, false);
        }
        if actions.contains(GameAction::HardDrop) {
            while self.try_translate(board, 0, 1) {}
            return self.lock(board).map(Some);
        }
        if actions.contains(GameAction::SoftDrop) {
            self.try_translate(board, 0, 1);
        }
        Ok(None)
    }

    /// Attempts a collision-free translation.
    pub fn try_translate(&mut self, board: &Board, dx: i8, dy: i8) -> bool {
        let Some(pivot) = shifted(board, self.pivot, dx, dy) else {
            return false;
        };
        if self
            .cells_at(board, pivot, &self.balls)
            .is_some_and(|cells| {
                cells
                    .into_iter()
                    .all(|(coord, _)| !board.get(coord).is_occupied())
            })
        {
            self.pivot = pivot;
            true
        } else {
            false
        }
    }

    /// Attempts a rotation around the pivot with no direct board mutation.
    pub fn try_rotate(&mut self, board: &Board, clockwise: bool) -> bool {
        let rotated: Vec<_> = self
            .balls
            .iter()
            .map(|ball| GroupBall {
                dx: if clockwise { -ball.dy } else { ball.dy },
                dy: if clockwise { ball.dx } else { -ball.dx },
                color: ball.color,
            })
            .collect();
        if self
            .cells_at(board, self.pivot, &rotated)
            .is_some_and(|cells| {
                cells
                    .into_iter()
                    .all(|(coord, _)| !board.get(coord).is_occupied())
            })
        {
            self.balls = rotated;
            true
        } else {
            false
        }
    }

    /// Atomically commits every ball, rejecting an invalid or blocked group.
    pub fn lock(&self, board: &mut Board) -> Result<Vec<Coord>, FallingError> {
        let cells = self.cells(board).ok_or(FallingError::InvalidPlacement)?;
        if cells
            .iter()
            .any(|(coord, _)| board.get(*coord).is_occupied())
        {
            return Err(FallingError::InvalidPlacement);
        }
        let coords = cells.iter().map(|(coord, _)| *coord).collect();
        for (coord, color) in cells {
            board.set(coord, Cell::Color(color));
        }
        Ok(coords)
    }

    fn cells_at(
        &self,
        board: &Board,
        pivot: Coord,
        balls: &[GroupBall],
    ) -> Option<Vec<(Coord, u8)>> {
        balls
            .iter()
            .map(|ball| {
                let x = i16::from(pivot.x) + i16::from(ball.dx);
                let y = i16::from(pivot.y) + i16::from(ball.dy);
                (x >= 0 && y >= 0)
                    .then(|| board.coord(x as u8, y as u8))
                    .flatten()
                    .map(|coord| (coord, ball.color))
            })
            .collect()
    }
}

fn shifted(board: &Board, coord: Coord, dx: i8, dy: i8) -> Option<Coord> {
    let x = i16::from(coord.x) + i16::from(dx);
    let y = i16::from(coord.y) + i16::from(dy);
    (x >= 0 && y >= 0)
        .then(|| board.coord(x as u8, y as u8))
        .flatten()
}
