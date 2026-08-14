//! One control tick: horizontal, rotation, soft drop, then gravity and lock.
//!
//! Every timer lives here rather than inside [`FallingGroup`], so the geometry
//! stays a pure function of shape, orientation and pivot while the timing stays
//! a pure function of the frozen profile plus the tick's actions.

use crate::{
    board::{Board, Cell, Coord},
    falling::{DoubleRotation, FallingGroup},
    input::{GameAction, PlayerActions},
    match_spec::DropTiming,
};

/// Everything one control tick reads from the frozen match spec.
///
/// The free-fall table is borrowed from the resolution rules rather than
/// copied: post-lock free fall and chain gravity use the same parameter set,
/// so duplicating it would let the two drift apart.
#[derive(Debug, Clone, Copy)]
pub struct ControlRules<'a> {
    /// Falling-group and rotation timing.
    pub timing: &'a DropTiming,
    /// Fall duration indexed by distance in cells.
    pub fall_ticks_by_distance: &'a [u16],
    /// Number of distinct ball colors.
    pub color_count: u8,
}

/// Result of one control tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlOutcome {
    /// The group is still controllable.
    Continue,
    /// The group locked; the board already holds every ball.
    Locked {
        /// Cells written by the lock, in the group's canonical order.
        coords: Vec<Coord>,
        /// Post-lock free fall, when any ball lost its support.
        split: Option<SplitState>,
    },
}

/// Why a group locked. Kept for diagnostics and presentation cues.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockCause {
    /// A hard drop.
    HardDrop,
    /// The grounded lock delay ran out.
    LockDelay,
    /// Soft drop was held while grounded.
    SoftDrop,
    /// Rotation push-ups reached the lift limit.
    LiftLimit,
}

/// Horizontal auto-repeat state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct HorizontalState {
    direction: Option<i8>,
    held_ticks: u16,
    cooldown: u16,
}

/// All control timers for one falling group.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ControlState {
    fall_ticks: u16,
    lock_delay_ticks: u16,
    lifts: u8,
    horizontal: HorizontalState,
    rotate_cooldown: [u16; 2],
    counter: DoubleRotation,
    soft_drop_cells: u32,
    last_lock_cause: Option<LockCause>,
}

impl ControlState {
    /// Fresh timers for a newly spawned group.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            fall_ticks: 0,
            lock_delay_ticks: 0,
            lifts: 0,
            horizontal: HorizontalState {
                direction: None,
                held_ticks: 0,
                cooldown: 0,
            },
            rotate_cooldown: [0, 0],
            counter: DoubleRotation::new(),
            soft_drop_cells: 0,
            last_lock_cause: None,
        }
    }

    /// Grounded ticks accumulated toward the lock delay.
    #[must_use]
    pub const fn lock_delay_ticks(&self) -> u16 {
        self.lock_delay_ticks
    }

    /// Rotation push-ups taken so far.
    #[must_use]
    pub const fn lifts(&self) -> u8 {
        self.lifts
    }

    /// The wedged-rotation counter.
    #[must_use]
    pub const fn double_rotation(&self) -> DoubleRotation {
        self.counter
    }

    /// Cells this group descended under soft drop.
    ///
    /// The caller turns these into display-only points; on the Fever profile
    /// they never reach the attack conversion.
    #[must_use]
    pub const fn soft_drop_cells(&self) -> u32 {
        self.soft_drop_cells
    }

    /// Why the group locked, once it has.
    #[must_use]
    pub const fn last_lock_cause(&self) -> Option<LockCause> {
        self.last_lock_cause
    }

    /// Applies one tick of already-normalized actions.
    ///
    /// A blocked operation is a deterministic no-op and never aborts the tick.
    pub fn step(
        &mut self,
        group: &mut FallingGroup,
        board: &mut Board,
        actions: PlayerActions,
        rules: ControlRules<'_>,
    ) -> ControlOutcome {
        let timing = rules.timing;
        let actions = actions.normalized();

        let direction = if actions.contains(GameAction::Left) {
            Some(-1)
        } else if actions.contains(GameAction::Right) {
            Some(1)
        } else {
            None
        };

        self.apply_horizontal(group, board, direction, timing);
        self.apply_rotation(group, board, actions, rules);

        if actions.contains(GameAction::HardDrop) {
            while group.try_translate(board, 0, 1) {}
            return self.lock(group, board, rules, LockCause::HardDrop);
        }

        // A held direction suppresses soft drop, so the two rates never stack.
        let soft_dropping = actions.contains(GameAction::SoftDrop) && direction.is_none();
        self.apply_gravity(group, board, soft_dropping, timing);

        if group.is_grounded(board) {
            if soft_dropping {
                return self.lock(group, board, rules, LockCause::SoftDrop);
            }
            self.lock_delay_ticks += 1;
            if self.lock_delay_ticks >= timing.lock_delay_ticks {
                return self.lock(group, board, rules, LockCause::LockDelay);
            }
        }
        if self.lifts >= timing.lift_limit {
            return self.lock(group, board, rules, LockCause::LiftLimit);
        }
        ControlOutcome::Continue
    }

    fn apply_horizontal(
        &mut self,
        group: &mut FallingGroup,
        board: &Board,
        direction: Option<i8>,
        timing: &DropTiming,
    ) {
        self.horizontal.cooldown = self.horizontal.cooldown.saturating_sub(1);

        let Some(dx) = direction else {
            self.horizontal.direction = None;
            self.horizontal.held_ticks = 0;
            return;
        };

        // A newly pressed direction moves on its own tick; holding it repeats
        // after the initial delay and then on the repeat interval.
        let fires = if self.horizontal.direction == Some(dx) {
            self.horizontal.held_ticks += 1;
            let held = self.horizontal.held_ticks;
            held == timing.horizontal_repeat_delay_ticks
                || (held > timing.horizontal_repeat_delay_ticks
                    && timing.horizontal_repeat_interval_ticks > 0
                    && (held - timing.horizontal_repeat_delay_ticks)
                        .is_multiple_of(timing.horizontal_repeat_interval_ticks))
        } else {
            self.horizontal.direction = Some(dx);
            self.horizontal.held_ticks = 0;
            true
        };

        if fires && self.horizontal.cooldown == 0 {
            group.try_translate(board, dx, 0);
            self.horizontal.cooldown = timing.horizontal_cooldown_ticks;
        }
    }

    fn apply_rotation(
        &mut self,
        group: &mut FallingGroup,
        board: &Board,
        actions: PlayerActions,
        rules: ControlRules<'_>,
    ) {
        let timing = rules.timing;
        for cooldown in &mut self.rotate_cooldown {
            *cooldown = cooldown.saturating_sub(1);
        }
        // Normalization has already dropped a same-tick clockwise plus
        // counter-clockwise pair, so at most one of these is set.
        let clockwise = if actions.contains(GameAction::RotateClockwise) {
            true
        } else if actions.contains(GameAction::RotateCounterClockwise) {
            false
        } else {
            return;
        };
        let slot = usize::from(!clockwise);
        if self.rotate_cooldown[slot] > 0 {
            return;
        }
        self.rotate_cooldown[slot] = timing.rotation_cooldown_ticks;

        let outcome = group.rotate(
            board,
            clockwise,
            &mut self.counter,
            timing.double_rotation_period,
            rules.color_count,
        );
        if outcome.lifted() {
            self.lifts = self.lifts.saturating_add(1);
        }
        // A rotation, including a push-back, deliberately does not reset the
        // fall or lock timers: a lifted group re-lands on the ordinary fall
        // rate and keeps accumulating its grace.
    }

    fn apply_gravity(
        &mut self,
        group: &mut FallingGroup,
        board: &Board,
        soft_dropping: bool,
        timing: &DropTiming,
    ) {
        let threshold = if soft_dropping {
            timing.soft_drop_ticks
        } else {
            timing.natural_fall_ticks
        };
        self.fall_ticks += 1;
        if threshold > 0 && self.fall_ticks >= threshold {
            self.fall_ticks = 0;
            // The lock grace is cumulative over the group's whole life: a
            // group that leaves the floor after a rotation push-up keeps what
            // it has already accumulated, so rotating cannot stall a drop.
            if group.try_translate(board, 0, 1) && soft_dropping {
                self.soft_drop_cells += 1;
            }
        }
    }

    fn lock(
        &mut self,
        group: &FallingGroup,
        board: &mut Board,
        rules: ControlRules<'_>,
        cause: LockCause,
    ) -> ControlOutcome {
        self.last_lock_cause = Some(cause);
        let pivot = group.pivot();
        match group.lock(board) {
            Ok(coords) => {
                let split = SplitState::plan(board, &coords, pivot, rules);
                ControlOutcome::Locked { coords, split }
            }
            // The pose was validated before every move, so this is unreachable
            // in practice; refusing to write a partial group is the documented
            // error semantics.
            Err(_) => ControlOutcome::Locked {
                coords: Vec::new(),
                split: None,
            },
        }
    }
}

/// One ball's post-lock free fall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BallFall {
    /// Cell the ball locked into.
    pub from: Coord,
    /// Cell it comes to rest in.
    pub to: Coord,
    /// Tick, counted from the lock, at which it starts moving.
    pub start_tick: u16,
    /// Tick, counted from the lock, at which it arrives.
    pub arrival_tick: u16,
}

/// Post-lock free fall for balls that lost their support.
///
/// The split runs before chain resolution, so a ball still in flight cannot be
/// scanned into a link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitState {
    falls: Vec<BallFall>,
    elapsed: u16,
}

impl SplitState {
    /// Plans the free fall, or `None` when every ball is supported.
    ///
    /// The pivot ball takes the shorter split delay; the balls that followed it
    /// take the longer one.
    #[must_use]
    pub fn plan(
        board: &Board,
        locked: &[Coord],
        pivot: Coord,
        rules: ControlRules<'_>,
    ) -> Option<Self> {
        // Bottom-up so a ball never plans through one that is still above it.
        let mut ordered: Vec<Coord> = locked.to_vec();
        ordered.sort_by_key(|coord| std::cmp::Reverse(coord.y()));

        let mut falls = Vec::new();
        let mut occupied = board.clone();
        for coord in ordered {
            let mut distance = 0_u8;
            let mut landing = coord;
            while let Some(below) = occupied.coord(landing.x(), landing.y() + 1) {
                if occupied.get(below).is_occupied() {
                    break;
                }
                landing = below;
                distance += 1;
            }
            if distance == 0 {
                continue;
            }
            let cell = occupied.get(coord);
            occupied.set(coord, Cell::Empty);
            occupied.set(landing, cell);

            let start = if coord == pivot {
                rules.timing.split_delay_pivot_ticks
            } else {
                rules.timing.split_delay_follower_ticks
            };
            falls.push(BallFall {
                from: coord,
                to: landing,
                start_tick: start,
                arrival_tick: start
                    .saturating_add(fall_duration(rules.fall_ticks_by_distance, distance)),
            });
        }
        (!falls.is_empty()).then_some(Self { falls, elapsed: 0 })
    }

    /// The planned falls, in bottom-up order.
    #[must_use]
    pub fn falls(&self) -> &[BallFall] {
        &self.falls
    }

    /// Ticks spent in the split so far.
    #[must_use]
    pub const fn elapsed_ticks(&self) -> u16 {
        self.elapsed
    }

    /// Whether every ball has arrived.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.falls
            .iter()
            .all(|fall| self.elapsed >= fall.arrival_tick)
    }

    /// Coordinates still in flight, which resolution must not scan.
    #[must_use]
    pub fn in_flight(&self) -> Vec<Coord> {
        self.falls
            .iter()
            .filter(|fall| self.elapsed < fall.arrival_tick)
            .map(|fall| fall.from)
            .collect()
    }

    /// Advances one tick, committing every ball whose arrival this tick is.
    pub fn tick(&mut self, board: &mut Board) {
        self.elapsed = self.elapsed.saturating_add(1);
        for fall in &self.falls {
            if fall.arrival_tick == self.elapsed {
                let cell = board.get(fall.from);
                board.set(fall.from, Cell::Empty);
                board.set(fall.to, cell);
            }
        }
    }
}

/// Free-fall duration for a distance, saturating at the table tail.
fn fall_duration(table: &[u16], distance: u8) -> u16 {
    table
        .get(usize::from(distance))
        .or_else(|| table.last())
        .copied()
        .unwrap_or(0)
}
