//! Integer scoring and attack-fraction arithmetic.

use crate::{
    resolution::ChainLinkFacts,
    rules::{BoardMode, ChainPowerProfile},
};

/// Score state keeps display-only soft-drop points separate from attack score.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScoreState {
    chain_score: u64,
    soft_drop_score: u64,
}

impl ScoreState {
    /// Adds points that may be converted into nuisance.
    pub fn add_chain_score(&mut self, score: u64) {
        self.chain_score += score;
    }

    /// Adds display-only soft-drop points.
    pub fn add_soft_drop_score(&mut self, score: u64) {
        self.soft_drop_score += score;
    }

    /// Total displayed score.
    #[must_use]
    pub const fn displayed(self) -> u64 {
        self.chain_score + self.soft_drop_score
    }

    /// Score eligible for attack conversion.
    #[must_use]
    pub const fn attack_score(self) -> u64 {
        self.chain_score
    }
}

/// Configurable scoring bonuses.  Tables use a zero-based count index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoringRules {
    color_bonus: Vec<u16>,
    group_bonus: Vec<u16>,
}

impl ScoringRules {
    /// Builds rules from integer lookup tables.
    #[must_use]
    pub fn new(color_bonus: Vec<u16>, group_bonus: Vec<u16>) -> Self {
        Self {
            color_bonus,
            group_bonus,
        }
    }

    /// Scores one committed link using only frozen integer tables.
    #[must_use]
    pub fn score_link(
        &self,
        link: &ChainLinkFacts,
        powers: &ChainPowerProfile,
        mode: BoardMode,
    ) -> u64 {
        let color = self.lookup(&self.color_bonus, link.color_count);
        // Accumulate in the wider type: a link can clear many groups, and
        // profile validation only bounds the sum in `u32`, not in `u16`.
        let groups: u32 = link
            .group_sizes
            .iter()
            .map(|size| u32::from(self.lookup(&self.group_bonus, *size)))
            .sum();
        let multiplier =
            (u32::from(powers.power(mode, link.chain_index)) + u32::from(color) + groups).clamp(
                u32::from(crate::rules::CHAIN_POWER_MIN),
                u32::from(crate::rules::CHAIN_POWER_MAX),
            );
        10 * u64::from(link.cleared_colored) * u64::from(multiplier)
    }

    fn lookup(&self, table: &[u16], count: u8) -> u16 {
        table
            .get(usize::from(count))
            .copied()
            .unwrap_or_else(|| *table.last().unwrap_or(&0))
    }
}

/// Attack remainder represented as exact score units below one nuisance.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AttackFraction {
    remainder: u64,
}

impl AttackFraction {
    /// Converts a committed chain score at target score `target_points`.
    ///
    /// Returns the whole nuisance amount while retaining the exact remainder.
    pub fn convert(&mut self, chain_score: u64, target_points: u64) -> u64 {
        assert!(target_points > 0, "target points must be positive");
        let total = self.remainder + chain_score;
        let nuisance = total / target_points;
        self.remainder = total % target_points;
        nuisance
    }

    /// Remaining score units that carry into the next committed link.
    #[must_use]
    pub const fn remainder(self) -> u64 {
        self.remainder
    }
}

/// Margin state stores an index, never a calculated copy of target points.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MarginState {
    table_index: usize,
}

impl MarginState {
    /// Recomputes the decay step from the round tick.
    ///
    /// The state holds only the index: the target score is always a table
    /// lookup, so no converted copy of it can go stale.
    pub fn advance_to(&mut self, rules: &crate::match_spec::MarginRules, round_tick: u64) {
        self.table_index = rules.step_at(round_tick);
    }

    /// Advances to the next value, saturating at the table tail.
    pub fn advance(&mut self, table: &[u64]) {
        if !table.is_empty() {
            self.table_index = (self.table_index + 1).min(table.len() - 1);
        }
    }

    /// Looks up the current target score in the frozen margin table.
    #[must_use]
    pub fn target_points(self, table: &[u64]) -> Option<u64> {
        table.get(self.table_index).copied()
    }

    /// Current frozen-table index.
    #[must_use]
    pub const fn table_index(self) -> usize {
        self.table_index
    }
}
