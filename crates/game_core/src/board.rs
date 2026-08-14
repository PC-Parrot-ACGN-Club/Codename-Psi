//! Fixed-size, pure in-memory rules board.

/// Default board width retained for fixtures and the Fever profile.
pub const BOARD_WIDTH: usize = 6;
/// Default board height including the two hidden rows.
pub const BOARD_HEIGHT: usize = 14;
/// Default rows at the top of the board excluded from resolution.
pub const HIDDEN_ROWS: usize = 2;

/// Geometry frozen into a match specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoardGeometry {
    width: u8,
    height: u8,
    hidden_rows: u8,
    spawn_column: u8,
}

impl BoardGeometry {
    /// Creates valid board geometry.
    #[must_use]
    pub fn new(width: u8, height: u8, hidden_rows: u8, spawn_column: u8) -> Option<Self> {
        (width > 0 && height > hidden_rows && spawn_column < width).then_some(Self {
            width,
            height,
            hidden_rows,
            spawn_column,
        })
    }

    /// Column count.
    #[must_use]
    pub const fn width(self) -> u8 {
        self.width
    }
    /// Total row count, including hidden rows.
    #[must_use]
    pub const fn height(self) -> u8 {
        self.height
    }
    /// Number of hidden top rows.
    #[must_use]
    pub const fn hidden_rows(self) -> u8 {
        self.hidden_rows
    }
    /// Column used to spawn a falling group.
    #[must_use]
    pub const fn spawn_column(self) -> u8 {
        self.spawn_column
    }
    /// Whether `coord` belongs to this board.
    #[must_use]
    pub const fn contains(self, coord: Coord) -> bool {
        coord.x < self.width && coord.y < self.height
    }
}

impl Default for BoardGeometry {
    fn default() -> Self {
        Self {
            width: BOARD_WIDTH as u8,
            height: BOARD_HEIGHT as u8,
            hidden_rows: HIDDEN_ROWS as u8,
            spawn_column: 2,
        }
    }
}

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
    geometry: BoardGeometry,
    cells: Vec<Cell>,
}

impl Default for Board {
    fn default() -> Self {
        Self::empty()
    }
}

impl Board {
    /// Creates an empty board.
    #[must_use]
    pub fn empty() -> Self {
        Self::with_geometry(BoardGeometry::default())
    }

    /// Creates an empty board for frozen match geometry.
    #[must_use]
    pub fn with_geometry(geometry: BoardGeometry) -> Self {
        Self {
            geometry,
            cells: vec![Cell::Empty; usize::from(geometry.width) * usize::from(geometry.height)],
        }
    }

    /// Frozen geometry for this board.
    #[must_use]
    pub const fn geometry(&self) -> BoardGeometry {
        self.geometry
    }

    /// Makes an in-range coordinate for this board.
    #[must_use]
    pub const fn coord(&self, x: u8, y: u8) -> Option<Coord> {
        if x < self.geometry.width && y < self.geometry.height {
            Some(Coord { x, y })
        } else {
            None
        }
    }

    /// Reads a coordinate. Every [`Coord`] is in range by construction, so this
    /// cannot fail.
    #[must_use]
    pub fn get(&self, coord: Coord) -> Cell {
        debug_assert!(self.geometry.contains(coord));
        self.cells[usize::from(coord.y) * usize::from(self.geometry.width) + usize::from(coord.x)]
    }

    /// Writes a valid board coordinate.
    pub fn set(&mut self, coord: Coord, cell: Cell) {
        debug_assert!(self.geometry.contains(coord));
        let index = usize::from(coord.y) * usize::from(self.geometry.width) + usize::from(coord.x);
        self.cells[index] = cell;
    }

    /// Iterates visible coordinates in the deterministic scan order.
    pub fn visible_coords(&self) -> impl Iterator<Item = Coord> + '_ {
        (self.geometry.hidden_rows..self.geometry.height)
            .flat_map(move |y| (0..self.geometry.width).map(move |x| Coord { x, y }))
    }

    /// Reports whether visible rows are clear; hidden cells do not participate.
    #[must_use]
    pub fn visible_is_empty(&self) -> bool {
        self.visible_coords()
            .all(|coord| !self.get(coord).is_occupied())
    }
}
