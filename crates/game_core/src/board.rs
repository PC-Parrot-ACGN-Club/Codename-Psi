//! Fixed-size, pure in-memory rules board.

/// Board width defined by the current rule family.
pub const BOARD_WIDTH: usize = 6;
/// Board height including the two hidden rows.
pub const BOARD_HEIGHT: usize = 14;
/// Rows at the top of the board excluded from resolution.
pub const HIDDEN_ROWS: usize = 2;

/// A single occupied or empty board cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Cell {
    /// No ball occupies the coordinate.
    #[default]
    Empty,
    /// An ordinary colored ball. Color IDs are frozen rule data.
    Color(u8),
    /// A nuisance ball.
    Nuisance,
}

impl Cell {
    /// Whether the cell is occupied.
    #[must_use]
    pub const fn is_occupied(self) -> bool {
        !matches!(self, Self::Empty)
    }
}

/// A board coordinate that is in range by construction.
///
/// The fields are crate-private so [`Coord::new`] is the only way to build one
/// from outside, which is what lets [`Board`] index without a bounds check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Coord {
    pub(crate) x: u8,
    pub(crate) y: u8,
}

impl Coord {
    /// Creates a coordinate when it belongs to the fixed rules board.
    #[must_use]
    pub const fn new(x: u8, y: u8) -> Option<Self> {
        if x < BOARD_WIDTH as u8 && y < BOARD_HEIGHT as u8 {
            Some(Self { x, y })
        } else {
            None
        }
    }

    /// Horizontal position, left to right.
    #[must_use]
    pub const fn x(self) -> u8 {
        self.x
    }

    /// Vertical position, top to bottom.
    #[must_use]
    pub const fn y(self) -> u8 {
        self.y
    }

    /// Whether this coordinate is in the visible resolution region.
    #[must_use]
    pub const fn is_visible(self) -> bool {
        self.y >= HIDDEN_ROWS as u8
    }
}

/// Board storage with no I/O or presentation state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    cells: [[Cell; BOARD_WIDTH]; BOARD_HEIGHT],
}

impl Default for Board {
    fn default() -> Self {
        Self::empty()
    }
}

impl Board {
    /// Creates an empty board.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            cells: [[Cell::Empty; BOARD_WIDTH]; BOARD_HEIGHT],
        }
    }

    /// Reads a coordinate. Every [`Coord`] is in range by construction, so this
    /// cannot fail.
    #[must_use]
    pub fn get(&self, coord: Coord) -> Cell {
        self.cells[coord.y as usize][coord.x as usize]
    }

    /// Writes a valid board coordinate.
    pub fn set(&mut self, coord: Coord, cell: Cell) {
        self.cells[coord.y as usize][coord.x as usize] = cell;
    }

    /// Iterates visible coordinates in the deterministic scan order.
    pub fn visible_coords() -> impl Iterator<Item = Coord> {
        (HIDDEN_ROWS as u8..BOARD_HEIGHT as u8)
            .flat_map(|y| (0..BOARD_WIDTH as u8).map(move |x| Coord { x, y }))
    }

    /// Reports whether visible rows are clear; hidden cells do not participate.
    #[must_use]
    pub fn visible_is_empty(&self) -> bool {
        Self::visible_coords().all(|coord| !self.get(coord).is_occupied())
    }
}
