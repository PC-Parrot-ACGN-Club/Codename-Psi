//! Immutable, integer-only rule values shared by the component rule modules.

/// Number of samples in a chain-power table.
pub const CHAIN_POWER_TABLE_LEN: usize = 24;

/// A frozen chain-power curve.  The table, rather than its generation
/// parameters, is the runtime authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainPowerProfile {
    normal: [u16; CHAIN_POWER_TABLE_LEN],
    fever: [u16; CHAIN_POWER_TABLE_LEN],
}

impl ChainPowerProfile {
    /// Creates a profile after checking the runtime table invariant.
    pub fn new(
        normal: [u16; CHAIN_POWER_TABLE_LEN],
        fever: [u16; CHAIN_POWER_TABLE_LEN],
    ) -> Result<Self, ChainPowerError> {
        for (mode, table) in [("normal", &normal), ("fever", &fever)] {
            if let Some((index, value)) = table
                .iter()
                .enumerate()
                .find(|(_, value)| !(1..=999).contains(*value))
            {
                return Err(ChainPowerError::OutOfRange {
                    mode,
                    index: index + 1,
                    value: *value,
                });
            }
        }
        Ok(Self { normal, fever })
    }

    /// Returns the power for a one-based chain index, saturating at the tail.
    #[must_use]
    pub fn power(&self, mode: BoardMode, chain_index: u8) -> u16 {
        let table = match mode {
            BoardMode::Normal => &self.normal,
            BoardMode::Fever => &self.fever,
        };
        table[usize::from(chain_index.saturating_sub(1)).min(CHAIN_POWER_TABLE_LEN - 1)]
    }

    /// Returns the runtime-authoritative normal table.
    #[must_use]
    pub const fn normal(&self) -> &[u16; CHAIN_POWER_TABLE_LEN] {
        &self.normal
    }

    /// Returns the runtime-authoritative Fever table.
    #[must_use]
    pub const fn fever(&self) -> &[u16; CHAIN_POWER_TABLE_LEN] {
        &self.fever
    }
}

/// Board channel that selects a chain-power curve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardMode {
    /// The ordinary board.
    Normal,
    /// The Fever board.
    Fever,
}

/// A runtime-invalid chain-power value.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChainPowerError {
    /// A stored table entry is outside its declared domain.
    #[error("{mode}[{index}] must be in 1..=999, got {value}")]
    OutOfRange {
        /// Table name.
        mode: &'static str,
        /// One-based sample position.
        index: usize,
        /// Rejected value.
        value: u16,
    },
}

/// Source parameters for the offline chain-power table generator.
///
/// These values are metadata in a profile. Runtime rules use
/// [`ChainPowerProfile`] and never call this generator during a match.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChainPowerParameters {
    /// Normal-board ten-chain anchor.
    pub normal_anchor: f64,
    /// Normal-board low-chain tilt.
    pub normal_tilt: f64,
    /// Normal-board post-ten growth.
    pub normal_growth: f64,
    /// Fever-board tail base.
    pub fever_anchor: f64,
    /// Fever-board low-chain tilt.
    pub fever_tilt: f64,
}

/// Regenerates a profile for CI/content validation, not for match execution.
#[must_use]
pub fn generate_chain_power_profile(parameters: ChainPowerParameters) -> ChainPowerProfile {
    let normal = std::array::from_fn(|index| {
        let chain = (index + 1) as f64;
        let shape = match index + 1 {
            1 => 0.01,
            2 => 0.03,
            3 => 0.06,
            4 => 0.08,
            5 => 0.12,
            6 => 0.24,
            7 => 0.40,
            8 => 0.60,
            9 => 0.80,
            10 => 1.00,
            _ => 1.00 + parameters.normal_growth * (chain - 10.0),
        };
        sample(
            parameters.normal_anchor
                * shape
                * parameters.normal_tilt.powf((10.0 - chain.min(10.0)) / 9.0),
        )
    });
    let fever = std::array::from_fn(|index| {
        let chain = (index + 1) as f64;
        let shape = match index + 1 {
            1 => 0.10,
            2 => 0.25,
            3 => 0.45,
            4 => 0.55,
            5 => 0.75,
            6 => 1.20,
            7 => 2.00,
            8 => 3.00,
            9 => 4.00,
            10 => 6.00,
            11 => 7.00,
            12 => 7.20,
            13 => 8.55,
            _ => chain - 4.0,
        };
        sample(
            parameters.fever_anchor
                * shape
                * parameters.fever_tilt.powf((14.0 - chain.min(14.0)) / 13.0),
        )
    });
    // The formula is clamped into the invariant required by `new`.
    ChainPowerProfile::new(normal, fever).expect("generated profile must be in domain")
}

/// Reports the first generated-table mismatch in a content file.
pub fn verify_chain_power_profile(
    stored: &ChainPowerProfile,
    parameters: ChainPowerParameters,
) -> Result<(), ChainPowerMismatch> {
    let generated = generate_chain_power_profile(parameters);
    for (mode, expected, actual) in [
        (BoardMode::Normal, generated.normal(), stored.normal()),
        (BoardMode::Fever, generated.fever(), stored.fever()),
    ] {
        if let Some((index, (&expected, &actual))) = expected
            .iter()
            .zip(actual)
            .enumerate()
            .find(|(_, (expected, actual))| expected != actual)
        {
            return Err(ChainPowerMismatch {
                mode,
                index: index + 1,
                expected,
                actual,
            });
        }
    }
    Ok(())
}

/// A content-table sample differs from its generated source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("{mode:?} chain-power sample {index}: expected {expected}, found {actual}")]
pub struct ChainPowerMismatch {
    /// Normal or Fever table.
    pub mode: BoardMode,
    /// One-based table sample.
    pub index: usize,
    /// Value regenerated in CI.
    pub expected: u16,
    /// Value stored in content.
    pub actual: u16,
}

fn sample(value: f64) -> u16 {
    value.round().clamp(1.0, 999.0) as u16
}
