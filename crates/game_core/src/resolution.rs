//! Tick-driven, deterministic chain resolution.

use std::collections::{BTreeSet, VecDeque};

use crate::board::{Board, Cell, Coord};

/// Immutable facts for one committed link in a chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainLinkFacts {
    /// One-based link number.
    pub chain_index: u8,
    /// Coordinates of colored balls removed by this link.
    pub cleared_colored_coords: Vec<Coord>,
    /// Coordinates of nuisance balls removed adjacent to cleared colors.
    pub cleared_nuisance_coords: Vec<Coord>,
    /// Number of distinct colors among the cleared groups.
    pub color_count: u8,
    /// Deterministically ordered cleared group sizes.
    pub group_sizes: Vec<u8>,
    /// Number of colored balls removed by this link.
    pub cleared_colored: u16,
}

/// Final facts from resolving one locked group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainReport {
    /// All committed links, in order.
    pub links: Vec<ChainLinkFacts>,
    /// Total colored balls cleared by all links.
    pub total_cleared_colored: u32,
    /// Stable final-board facts.
    pub field: FieldFacts,
}

/// Read-only facts about a stable field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldFacts {
    /// Whether the visible region is empty. Hidden rows are intentionally ignored.
    pub all_clear: bool,
}

/// Public resolution phase. Reading it never advances rules time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionPhase {
    /// No resolution is running. The player controls a falling group, and a
    /// lock is what leaves this state.
    Idle,
    /// A clear is previewed without changing the board.
    ClearPreview {
        /// Pending fact to present.
        facts: ChainLinkFacts,
        /// Ticks elapsed in this phase.
        elapsed_ticks: u16,
        /// Frozen phase duration.
        duration_ticks: u16,
    },
    /// Zero-tick boundary: the previewed balls are gone and this link's facts
    /// are effective; gravity has not been planned yet.
    ClearCommit {
        /// Facts that just took effect.
        facts: ChainLinkFacts,
    },
    /// Gravity animation is pending; board remains pre-gravity until commit.
    Gravity {
        /// Deterministic source/target movement pairs.
        moves: Vec<GravityMove>,
        /// Board committed atomically when the timed phase completes.
        target_board: Board,
        /// Ticks elapsed in this phase.
        elapsed_ticks: u16,
        /// Frozen phase duration.
        duration_ticks: u16,
    },
    /// Zero-tick boundary: a stable board is committed and the next link has
    /// not been scanned for yet.
    ScanNext {
        /// One-based number the next link would carry.
        next_chain_index: u8,
    },
    /// Terminal report; no further mutation occurs.
    Settlement(ChainReport),
}

/// One cell's gravity move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GravityMove {
    /// Source coordinate before gravity commits.
    pub from: Coord,
    /// Destination coordinate after gravity commits.
    pub to: Coord,
}

/// Frozen timings needed by the resolver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionRules {
    /// Preview duration before a clear commits.
    pub clear_preview_ticks: u16,
    /// Gravity duration indexed by maximum fall distance; index zero is valid.
    pub gravity_ticks_by_distance: Vec<u16>,
    /// Minimum group size required to clear.
    pub clear_threshold: u8,
}

impl Default for ResolutionRules {
    fn default() -> Self {
        Self {
            clear_preview_ticks: 12,
            gravity_ticks_by_distance: vec![0, 10, 15, 19, 22, 25, 28, 31, 33, 35, 37, 39, 41, 43],
            clear_threshold: 4,
        }
    }
}

/// An in-progress resolution. It owns all uncommitted state, so a snapshot is
/// an ordinary clone and presentation cannot mutate it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionState {
    board: Board,
    rules: ResolutionRules,
    phase: ResolutionPhase,
    committed_links: Vec<ChainLinkFacts>,
}

impl ResolutionState {
    /// Creates a resting resolution over a stable board. Nothing advances until
    /// [`ResolutionState::lock`] reports that a group locked.
    #[must_use]
    pub fn idle(board: Board, rules: ResolutionRules) -> Self {
        Self {
            board,
            rules,
            phase: ResolutionPhase::Idle,
            committed_links: Vec::new(),
        }
    }

    /// Starts resolution by scanning the supplied stable board, i.e. an idle
    /// resolution that a lock has already triggered.
    #[must_use]
    pub fn new(board: Board, rules: ResolutionRules) -> Self {
        let mut state = Self::idle(board, rules);
        state.lock();
        state
    }

    /// Applies the `GroupLocked` trigger: leaves [`ResolutionPhase::Idle`] and
    /// scans the board for the first link.
    ///
    /// A lock cannot arrive while a resolution is already running, so this is a
    /// no-op in every other phase.
    pub fn lock(&mut self) {
        if matches!(self.phase, ResolutionPhase::Idle) {
            self.committed_links.clear();
            self.scan_next();
        }
    }

    /// Current board; it changes only at clear and gravity commit boundaries.
    #[must_use]
    pub const fn board(&self) -> &Board {
        &self.board
    }

    /// Current phase, suitable for presentation.
    #[must_use]
    pub const fn phase(&self) -> &ResolutionPhase {
        &self.phase
    }

    /// Advances exactly one rule tick.
    ///
    /// A tick first leaves whatever zero-tick boundary it starts on, then
    /// spends its tick on the timed phase that boundary opened. Entering a
    /// boundary at the end of a tick therefore leaves it observable and
    /// snapshottable, without costing the chain step an extra tick.
    pub fn tick(&mut self) {
        self.leave_boundaries();
        match &mut self.phase {
            ResolutionPhase::ClearPreview {
                elapsed_ticks,
                duration_ticks,
                ..
            } => {
                *elapsed_ticks += 1;
                if *elapsed_ticks >= *duration_ticks {
                    self.enter_clear_commit();
                }
            }
            ResolutionPhase::Gravity {
                elapsed_ticks,
                duration_ticks,
                ..
            } => {
                *elapsed_ticks += 1;
                if *elapsed_ticks >= *duration_ticks {
                    self.enter_scan_next();
                }
            }
            ResolutionPhase::Idle
            | ResolutionPhase::ClearCommit { .. }
            | ResolutionPhase::ScanNext { .. }
            | ResolutionPhase::Settlement(_) => {}
        }
    }

    /// Runs the boundary actions until the phase is timed or terminal. A
    /// boundary reached from another boundary cascades inside the same tick,
    /// so a zero-distance gravity does not stall the chain.
    fn leave_boundaries(&mut self) {
        loop {
            match &self.phase {
                ResolutionPhase::ClearCommit { .. } => self.plan_gravity(),
                ResolutionPhase::ScanNext { .. } => self.scan_next(),
                _ => return,
            }
        }
    }

    /// Runs until the immutable settlement report exists.
    ///
    /// # Panics
    ///
    /// Panics when the resolution is still idle: no tick can leave that phase,
    /// so waiting for a report would never return.
    pub fn settle(&mut self) -> &ChainReport {
        assert!(
            !matches!(self.phase, ResolutionPhase::Idle),
            "settle needs a lock to have triggered the resolution"
        );
        while !matches!(self.phase, ResolutionPhase::Settlement(_)) {
            self.tick();
        }
        match &self.phase {
            ResolutionPhase::Settlement(report) => report,
            _ => unreachable!(),
        }
    }

    /// Returns the report after settlement without advancing time.
    #[must_use]
    pub fn report(&self) -> Option<&ChainReport> {
        match &self.phase {
            ResolutionPhase::Settlement(report) => Some(report),
            _ => None,
        }
    }

    fn scan_next(&mut self) {
        let chain_index = self.committed_links.len() as u8 + 1;
        match scan_link(&self.board, self.rules.clear_threshold, chain_index) {
            Some(facts) => {
                self.phase = ResolutionPhase::ClearPreview {
                    facts,
                    elapsed_ticks: 0,
                    duration_ticks: self.rules.clear_preview_ticks,
                }
            }
            None => self.finish(),
        }
    }

    /// `ClearPreview` end action: remove the previewed balls atomically and make
    /// this link's facts effective.
    fn enter_clear_commit(&mut self) {
        let facts = match &self.phase {
            ResolutionPhase::ClearPreview { facts, .. } => facts.clone(),
            _ => unreachable!(),
        };
        for coord in facts
            .cleared_colored_coords
            .iter()
            .chain(&facts.cleared_nuisance_coords)
        {
            self.board.set(*coord, Cell::Empty);
        }
        self.committed_links.push(facts.clone());
        self.phase = ResolutionPhase::ClearCommit { facts };
    }

    /// `ClearCommit` end action: plan the gravity moves and their duration.
    fn plan_gravity(&mut self) {
        let (moves, target_board, max_distance) = gravity_plan(&self.board);
        let duration_ticks = gravity_duration(&self.rules.gravity_ticks_by_distance, max_distance);
        // Keep the target private to the presentation-facing phase. Recompute
        // from the unchanged board at the atomic commit boundary.
        self.phase = ResolutionPhase::Gravity {
            moves,
            target_board,
            elapsed_ticks: 0,
            duration_ticks,
        };
        if duration_ticks == 0 {
            self.enter_scan_next();
        }
    }

    /// `Gravity` end action: commit the target board atomically.
    fn enter_scan_next(&mut self) {
        let target = match &self.phase {
            ResolutionPhase::Gravity { target_board, .. } => target_board.clone(),
            _ => unreachable!("gravity completion requires a planned target board"),
        };
        self.board = target;
        self.phase = ResolutionPhase::ScanNext {
            next_chain_index: self.committed_links.len() as u8 + 1,
        };
    }

    fn finish(&mut self) {
        let total_cleared_colored = self
            .committed_links
            .iter()
            .map(|link| u32::from(link.cleared_colored))
            .sum();
        self.phase = ResolutionPhase::Settlement(ChainReport {
            links: self.committed_links.clone(),
            total_cleared_colored,
            field: FieldFacts {
                all_clear: self.board.visible_is_empty(),
            },
        });
    }
}

fn scan_link(board: &Board, threshold: u8, chain_index: u8) -> Option<ChainLinkFacts> {
    let mut visited = BTreeSet::new();
    let mut groups: Vec<(u8, Vec<Coord>)> = Vec::new();
    for origin in board.visible_coords() {
        if visited.contains(&origin) {
            continue;
        }
        let Cell::Color(color) = board.get(origin) else {
            continue;
        };
        let mut group = Vec::new();
        let mut queue = VecDeque::from([origin]);
        visited.insert(origin);
        while let Some(coord) = queue.pop_front() {
            group.push(coord);
            for neighbor in visible_neighbors(board, coord) {
                if !visited.contains(&neighbor) && board.get(neighbor) == Cell::Color(color) {
                    visited.insert(neighbor);
                    queue.push_back(neighbor);
                }
            }
        }
        if group.len() >= usize::from(threshold) {
            groups.push((color, group));
        }
    }
    if groups.is_empty() {
        return None;
    }
    let mut colored = BTreeSet::new();
    let mut nuisance = BTreeSet::new();
    let mut colors = BTreeSet::new();
    let mut group_sizes = Vec::with_capacity(groups.len());
    for (color, group) in groups {
        colors.insert(color);
        group_sizes.push(group.len() as u8);
        for coord in group {
            colored.insert(coord);
            for neighbor in visible_neighbors(board, coord) {
                if board.get(neighbor) == Cell::Nuisance {
                    nuisance.insert(neighbor);
                }
            }
        }
    }
    let cleared_colored_coords: Vec<_> = colored.into_iter().collect();
    Some(ChainLinkFacts {
        chain_index,
        cleared_colored: cleared_colored_coords.len() as u16,
        cleared_colored_coords,
        cleared_nuisance_coords: nuisance.into_iter().collect(),
        color_count: colors.len() as u8,
        group_sizes,
    })
}

fn visible_neighbors(board: &Board, coord: Coord) -> impl Iterator<Item = Coord> + '_ {
    const OFFSETS: [(i8, i8); 4] = [(0, -1), (-1, 0), (1, 0), (0, 1)];
    OFFSETS.into_iter().filter_map(move |(dx, dy)| {
        let x = i16::from(coord.x) + i16::from(dx);
        let y = i16::from(coord.y) + i16::from(dy);
        let geometry = board.geometry();
        if x >= 0
            && x < i16::from(geometry.width())
            && y >= i16::from(geometry.hidden_rows())
            && y < i16::from(geometry.height())
        {
            Some(Coord {
                x: x as u8,
                y: y as u8,
            })
        } else {
            None
        }
    })
}

/// Gravity duration for a fall distance, saturating at the table tail.
pub(crate) fn gravity_duration(table: &[u16], distance: u8) -> u16 {
    table
        .get(usize::from(distance))
        .or_else(|| table.last())
        .copied()
        .unwrap_or(0)
}

/// Compacts every column and reports the moves, the target and the longest fall.
pub(crate) fn gravity_plan(board: &Board) -> (Vec<GravityMove>, Board, u8) {
    // Every column compacts over its full height. Hidden rows are excluded from
    // scanning, adjacent-nuisance clearing and `all_clear`, but not from
    // gravity: a ball parked in a hidden row falls into the visible region and
    // takes part in later links from there.
    let geometry = board.geometry();
    let mut target = Board::with_geometry(geometry);
    let mut moves = Vec::new();
    let mut max_distance = 0;
    for x in 0..geometry.width() {
        let mut destination = geometry.height() - 1;
        for y in (0..geometry.height()).rev() {
            let from = Coord { x, y };
            let cell = board.get(from);
            if !cell.is_occupied() {
                continue;
            }
            let to = Coord { x, y: destination };
            target.set(to, cell);
            if from != to {
                max_distance = max_distance.max(to.y - from.y);
                moves.push(GravityMove { from, to });
            }
            destination = destination.saturating_sub(1);
        }
    }
    (moves, target, max_distance)
}

impl crate::digest::Digestible for ChainLinkFacts {
    fn digest_into(&self, writer: &mut crate::digest::DigestWriter) {
        writer.u8(self.chain_index);
        writer.seq(&self.cleared_colored_coords);
        writer.seq(&self.cleared_nuisance_coords);
        writer.u8(self.color_count);
        writer.len(self.group_sizes.len());
        for size in &self.group_sizes {
            writer.u8(*size);
        }
        writer.u16(self.cleared_colored);
    }
}

impl crate::digest::Digestible for FieldFacts {
    fn digest_into(&self, writer: &mut crate::digest::DigestWriter) {
        writer.bool(self.all_clear);
    }
}

impl crate::digest::Digestible for ChainReport {
    fn digest_into(&self, writer: &mut crate::digest::DigestWriter) {
        writer.seq(&self.links);
        writer.u32(self.total_cleared_colored);
        self.field.digest_into(writer);
    }
}

impl crate::digest::Digestible for GravityMove {
    fn digest_into(&self, writer: &mut crate::digest::DigestWriter) {
        self.from.digest_into(writer);
        self.to.digest_into(writer);
    }
}

impl crate::digest::Digestible for ResolutionPhase {
    fn digest_into(&self, writer: &mut crate::digest::DigestWriter) {
        match self {
            Self::Idle => writer.u8(0),
            Self::ClearPreview {
                facts,
                elapsed_ticks,
                duration_ticks,
            } => {
                writer.u8(1);
                facts.digest_into(writer);
                writer.u16(*elapsed_ticks);
                writer.u16(*duration_ticks);
            }
            Self::ClearCommit { facts } => {
                writer.u8(2);
                facts.digest_into(writer);
            }
            Self::Gravity {
                moves,
                target_board,
                elapsed_ticks,
                duration_ticks,
            } => {
                writer.u8(3);
                writer.seq(moves);
                // The uncommitted target board is persistent state even though
                // it is never exposed, so it has to enter the checksum.
                target_board.digest_into(writer);
                writer.u16(*elapsed_ticks);
                writer.u16(*duration_ticks);
            }
            Self::ScanNext { next_chain_index } => {
                writer.u8(4);
                writer.u8(*next_chain_index);
            }
            Self::Settlement(report) => {
                writer.u8(5);
                report.digest_into(writer);
            }
        }
    }
}

impl crate::digest::Digestible for ResolutionState {
    fn digest_into(&self, writer: &mut crate::digest::DigestWriter) {
        self.board.digest_into(writer);
        self.phase.digest_into(writer);
        writer.seq(&self.committed_links);
    }
}
