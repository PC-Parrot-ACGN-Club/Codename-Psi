//! Player-level Fever gauge and time lifecycle.

/// Fever state that survives normal/Fever board channel switches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeverState {
    gauge: u8,
    capacity: u8,
    time_ticks: u32,
    min_time_ticks: u32,
    max_time_ticks: u32,
    active: bool,
    exit_pending: bool,
}

impl FeverState {
    #[must_use]
    pub const fn new(
        capacity: u8,
        initial_time_ticks: u32,
        min_time_ticks: u32,
        max_time_ticks: u32,
    ) -> Self {
        Self {
            gauge: 0,
            capacity,
            time_ticks: initial_time_ticks,
            min_time_ticks,
            max_time_ticks,
            active: false,
            exit_pending: false,
        }
    }
    #[must_use]
    pub const fn active(self) -> bool {
        self.active
    }
    #[must_use]
    pub const fn time_ticks(self) -> u32 {
        self.time_ticks
    }
    /// Records a qualifying offset; one safety point can add at most one gauge cell.
    pub fn record_offset(&mut self, qualifying: bool) {
        if qualifying {
            self.gauge = (self.gauge + 1).min(self.capacity);
        }
    }
    /// Enters only at the caller's settlement boundary.
    pub fn enter_if_full(&mut self) -> bool {
        if self.gauge == self.capacity {
            self.gauge = 0;
            self.active = true;
            true
        } else {
            false
        }
    }
    /// Adds a player-level reward even outside Fever.
    pub fn reward_time(&mut self, ticks: u32) {
        self.time_ticks = self
            .time_ticks
            .saturating_add(ticks)
            .min(self.max_time_ticks);
    }
    /// Whether the clock has run out and the exit is waiting for a boundary.
    ///
    /// The clock does not pause for settlement animation, so it can reach zero
    /// mid-chain. The exit is recorded here and applied at the safety point,
    /// which is what keeps the chain's tick sequence unaffected.
    #[must_use]
    pub const fn exit_pending(self) -> bool {
        self.exit_pending
    }

    /// Advances one rules tick; expiry asks the caller to exit at a safe boundary.
    pub fn tick(&mut self) -> bool {
        if self.active {
            self.time_ticks = self.time_ticks.saturating_sub(1).max(self.min_time_ticks);
            if self.time_ticks == self.min_time_ticks {
                self.exit_pending = true;
            }
        }
        self.active && self.exit_pending
    }

    /// Applies a pending exit. Callers only reach here at a safety point.
    pub fn exit(&mut self) {
        self.active = false;
        self.exit_pending = false;
    }
}
