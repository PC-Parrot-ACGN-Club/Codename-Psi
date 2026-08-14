//! Falling-group geometry, rotation judgement and locking.
//!
//! A group is a shape plus an orientation and a pivot, never a free-form set of
//! cells: every pose is derived from the frozen drop-set hand, so an illegal
//! pose cannot be represented rather than merely rejected.

use crate::{
    board::{Board, Cell, Coord},
    config::{ColorSlot, DropShape, DropTemplate},
};

/// Orientation, `0..=3`, clockwise from the spawn pose.
///
/// `0` puts the follower above the pivot, so `0` and `2` are the vertical
/// orientations and `1` and `3` the horizontal ones.
pub type TransformId = u8;

/// The four orientations.
pub const TRANSFORM_COUNT: u8 = 4;

/// One ball of a group at its current orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupBall {
    /// Column offset from the pivot.
    pub dx: i8,
    /// Row offset from the pivot; negative is upward.
    pub dy: i8,
    /// Resolved color id.
    pub color: u8,
}

/// A not-yet-committed group of balls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FallingGroup {
    template: DropTemplate,
    transform_id: TransformId,
    pivot: Coord,
    colors: [u8; 2],
    turn_id: u32,
}

/// Failed group construction or atomic placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FallingError {
    /// The group does not fit the board at its requested position.
    #[error("falling group does not fit the board at its requested position")]
    InvalidPlacement,
}

/// What a rotation input did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationOutcome {
    /// The target cells were free.
    Confirmed,
    /// The group was pushed toward the opposite side and then rotated.
    PushedBack {
        /// Column component of the push.
        dx: i8,
        /// Row component of the push; negative is upward.
        dy: i8,
    },
    /// Wedged between two columns, and this input released the flip.
    DoubleRotated,
    /// A single-color `O` cycled its color instead of turning.
    ColorCycled,
    /// Nothing changed.
    Blocked,
}

impl RotationOutcome {
    /// Whether the group ended this input in a new orientation.
    #[must_use]
    pub const fn turned(self) -> bool {
        matches!(
            self,
            Self::Confirmed | Self::PushedBack { .. } | Self::DoubleRotated
        )
    }

    /// Whether the group was lifted, which counts against the lift limit.
    #[must_use]
    pub const fn lifted(self) -> bool {
        matches!(self, Self::PushedBack { dy, .. } if dy < 0)
    }
}

/// The wedged-rotation counter.
///
/// Two blocked attempts release a 180 degree flip; confirming an ordinary
/// rotation rounds the counter back down to an even value so the parity keeps
/// its meaning across poses.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DoubleRotation {
    attempts: u8,
}

impl DoubleRotation {
    /// A counter that has not yet seen a wedged attempt.
    #[must_use]
    pub const fn new() -> Self {
        Self { attempts: 0 }
    }

    /// Current attempt count.
    #[must_use]
    pub const fn attempts(self) -> u8 {
        self.attempts
    }

    /// Records a wedged attempt; `true` when it releases the flip.
    fn record_attempt(&mut self, period: u8) -> bool {
        self.attempts = self.attempts.saturating_add(1);
        period >= 2 && self.attempts.is_multiple_of(period)
    }

    /// Rounds down to the nearest even count after a confirmed rotation.
    fn settle(&mut self) {
        self.attempts &= !1;
    }
}

impl FallingGroup {
    /// Creates a group in its spawn pose.
    #[must_use]
    pub const fn new(template: DropTemplate, colors: [u8; 2], pivot: Coord, turn_id: u32) -> Self {
        Self {
            template,
            transform_id: 0,
            pivot,
            colors,
            turn_id,
        }
    }

    /// The frozen hand this group came from.
    #[must_use]
    pub const fn template(&self) -> DropTemplate {
        self.template
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

    /// Current orientation.
    #[must_use]
    pub const fn transform_id(&self) -> TransformId {
        self.transform_id
    }

    /// The hand's drawn colors, first then second.
    #[must_use]
    pub const fn colors(&self) -> [u8; 2] {
        self.colors
    }

    /// Ball offsets and colors at the current orientation.
    #[must_use]
    pub fn balls(&self) -> Vec<GroupBall> {
        oriented(self.template, self.transform_id)
            .into_iter()
            .map(|(dx, dy, slot)| GroupBall {
                dx,
                dy,
                color: self.colors[usize::from(slot == ColorSlot::Second)],
            })
            .collect()
    }

    /// Occupied cells, or `None` when the pose leaves the board.
    #[must_use]
    pub fn cells(&self, board: &Board) -> Option<Vec<(Coord, u8)>> {
        cells_at(board, self.pivot, &self.balls())
    }

    /// Whether the pose fits the board and touches no settled ball.
    #[must_use]
    pub fn can_place(&self, board: &Board) -> bool {
        fits(board, self.pivot, &self.balls())
    }

    /// Attempts a collision-free translation.
    pub fn try_translate(&mut self, board: &Board, dx: i8, dy: i8) -> bool {
        let Some(pivot) = shifted(board, self.pivot, dx, dy) else {
            return false;
        };
        if fits(board, pivot, &self.balls()) {
            self.pivot = pivot;
            true
        } else {
            false
        }
    }

    /// Whether the group rests on the board floor or on a settled ball.
    #[must_use]
    pub fn is_grounded(&self, board: &Board) -> bool {
        let mut probe = *self;
        !probe.try_translate(board, 0, 1)
    }

    /// Cycles a single-color `O` through the color domain.
    fn cycle_color(&mut self, color_count: u8) {
        if color_count > 0 {
            self.colors[0] = (self.colors[0] + 1) % color_count;
            self.colors[1] = self.colors[0];
        }
    }

    /// Applies one rotation input using the full judgement order.
    ///
    /// The order is: free target confirms; a hidden-row vertical target is
    /// refused; a free opposite side pushes the group and confirms; otherwise
    /// the wedged counter decides whether this input releases a flip.
    pub fn rotate(
        &mut self,
        board: &Board,
        clockwise: bool,
        counter: &mut DoubleRotation,
        double_rotation_period: u8,
        color_count: u8,
    ) -> RotationOutcome {
        if matches!(self.template.shape, DropShape::OMono) {
            self.cycle_color(color_count);
            return RotationOutcome::ColorCycled;
        }

        let target = if clockwise {
            (self.transform_id + 1) % TRANSFORM_COUNT
        } else {
            (self.transform_id + TRANSFORM_COUNT - 1) % TRANSFORM_COUNT
        };
        let target_balls = self.balls_at(target);

        // 2. A free target confirms straight away.
        if fits(board, self.pivot, &target_balls) {
            self.transform_id = target;
            counter.settle();
            return RotationOutcome::Confirmed;
        }

        // 3. Inside the hidden rows a vertical target must not shove the group
        //    further up, so it is refused rather than pushed.
        if !self.pivot.is_visible() && is_vertical(target) {
            return RotationOutcome::Blocked;
        }

        // 4. A free opposite side pushes the whole group and confirms.
        let opposite = target ^ 2;
        let (dx, dy) = push_vector(opposite);
        if let Some(pushed) = shifted(board, self.pivot, dx, dy)
            && fits(board, pushed, &target_balls)
        {
            self.pivot = pushed;
            self.transform_id = target;
            counter.settle();
            return RotationOutcome::PushedBack { dx, dy };
        }

        // 5. Wedged between two columns: parity decides.
        if counter.record_attempt(double_rotation_period) {
            let flipped = (self.transform_id + 2) % TRANSFORM_COUNT;
            if fits(board, self.pivot, &self.balls_at(flipped)) {
                self.transform_id = flipped;
                return RotationOutcome::DoubleRotated;
            }
        }
        RotationOutcome::Blocked
    }

    /// Atomically commits every ball, rejecting an invalid or blocked pose.
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

    fn balls_at(&self, transform: TransformId) -> Vec<GroupBall> {
        oriented(self.template, transform)
            .into_iter()
            .map(|(dx, dy, slot)| GroupBall {
                dx,
                dy,
                color: self.colors[usize::from(slot == ColorSlot::Second)],
            })
            .collect()
    }
}

/// Whether a target orientation is one of the two vertical ones.
const fn is_vertical(transform: TransformId) -> bool {
    transform.is_multiple_of(2)
}

/// Which way to shove the group when the opposite side is free.
const fn push_vector(opposite: TransformId) -> (i8, i8) {
    match opposite % TRANSFORM_COUNT {
        0 => (0, -1),
        1 => (1, 0),
        2 => (0, 1),
        _ => (-1, 0),
    }
}

/// Ball offsets and color slots for one hand at one orientation.
///
/// `O` shapes keep their occupancy across orientations because they turn
/// around the block's center rather than around a cell: a dual-color `O`
/// rotates which side carries the first color, and a single-color `O` does not
/// turn at all.
fn oriented(template: DropTemplate, transform: TransformId) -> Vec<(i8, i8, ColorSlot)> {
    const BLOCK: [(i8, i8); 4] = [(0, 0), (1, 0), (0, -1), (1, -1)];
    match template.shape {
        DropShape::OMono => BLOCK
            .iter()
            .map(|(dx, dy)| (*dx, *dy, ColorSlot::First))
            .collect(),
        DropShape::ODual => BLOCK
            .iter()
            .map(|(dx, dy)| {
                let first = match transform % TRANSFORM_COUNT {
                    0 => *dy == 0,
                    1 => *dx == 0,
                    2 => *dy == -1,
                    _ => *dx == 1,
                };
                (
                    *dx,
                    *dy,
                    if first {
                        ColorSlot::First
                    } else {
                        ColorSlot::Second
                    },
                )
            })
            .collect(),
        DropShape::I | DropShape::L | DropShape::J => {
            let mut cells: Vec<_> = template
                .balls()
                .into_iter()
                .map(|ball| (ball.dx, ball.dy, ball.color_slot))
                .collect();
            for _ in 0..(transform % TRANSFORM_COUNT) {
                for cell in &mut cells {
                    let (dx, dy) = (cell.0, cell.1);
                    cell.0 = -dy;
                    cell.1 = dx;
                }
            }
            cells
        }
    }
}

fn cells_at(board: &Board, pivot: Coord, balls: &[GroupBall]) -> Option<Vec<(Coord, u8)>> {
    balls
        .iter()
        .map(|ball| {
            let x = i16::from(pivot.x()) + i16::from(ball.dx);
            let y = i16::from(pivot.y()) + i16::from(ball.dy);
            (x >= 0 && y >= 0)
                .then(|| board.coord(x as u8, y as u8))
                .flatten()
                .map(|coord| (coord, ball.color))
        })
        .collect()
}

fn fits(board: &Board, pivot: Coord, balls: &[GroupBall]) -> bool {
    cells_at(board, pivot, balls).is_some_and(|cells| {
        cells
            .into_iter()
            .all(|(coord, _)| !board.get(coord).is_occupied())
    })
}

fn shifted(board: &Board, coord: Coord, dx: i8, dy: i8) -> Option<Coord> {
    let x = i16::from(coord.x()) + i16::from(dx);
    let y = i16::from(coord.y()) + i16::from(dy);
    (x >= 0 && y >= 0)
        .then(|| board.coord(x as u8, y as u8))
        .flatten()
}
