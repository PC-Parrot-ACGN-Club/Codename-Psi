//! Snapshot, restore, state checksum and the headless verification log.
//!
//! These serve development checking and the later rollback work. Nothing here
//! is a player-facing replay feature.

use crate::{
    config::{CharacterId, ValidatedRuleLibrary},
    determinism::StateChecksum,
    digest::{ContentDigest, DigestWriter, Digestible},
    input::{PlayerActions, TickInputs},
    match_spec::{
        AlgorithmVersions, LockedMatchSpec, MatchDigests, MatchRequest, PARTICIPANT_SLOTS,
    },
    match_state::MatchState,
};

/// Schema of the snapshot header.
pub const SNAPSHOT_SCHEMA_VERSION: u32 = 1;

/// Format of the verification log.
pub const VERIFICATION_LOG_FORMAT_VERSION: u32 = 1;

/// A deep copy of every field that can affect a later tick.
///
/// Presentation, audio, settings, device state, AI plans and sockets are all
/// absent by construction: none of them live in [`MatchState`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchSnapshot {
    /// Header schema version.
    pub snapshot_schema_version: u32,
    /// Digest tree the snapshot was taken under.
    pub digests: MatchDigests,
    /// Algorithm versions the snapshot was taken under.
    pub algorithms: AlgorithmVersions,
    state: MatchState,
}

/// Why a snapshot could not be restored.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SnapshotError {
    /// The snapshot header is from a schema this build does not read.
    #[error("snapshot schema {found} is not supported (supported: {supported})")]
    UnsupportedSchema {
        /// Version found in the snapshot.
        found: u32,
        /// Version this build supports.
        supported: u32,
    },
    /// The content the snapshot was taken under is not the content supplied.
    #[error("snapshot digest {found} does not match the supplied rules {expected}")]
    DigestMismatch {
        /// Root digest carried by the snapshot.
        found: ContentDigest,
        /// Root digest of the supplied rules.
        expected: ContentDigest,
    },
    /// One of the algorithm versions differs.
    #[error("snapshot {algorithm} version {found} does not match this build's {expected}")]
    AlgorithmMismatch {
        /// Which algorithm differs.
        algorithm: &'static str,
        /// Version carried by the snapshot.
        found: u32,
        /// Version this build produces.
        expected: u32,
    },
}

impl MatchState {
    /// Takes a deep copy of every field that can affect a later tick.
    #[must_use]
    pub fn snapshot(&self) -> MatchSnapshot {
        MatchSnapshot {
            snapshot_schema_version: SNAPSHOT_SCHEMA_VERSION,
            digests: self.spec().digests.clone(),
            algorithms: self.spec().algorithms,
            state: self.clone(),
        }
    }

    /// Canonical checksum over this state.
    ///
    /// The root digest is the prefix, so two states are only ever comparable
    /// when they were produced under the same rule content.
    #[must_use]
    pub fn checksum(&self) -> StateChecksum {
        let mut writer = DigestWriter::new();
        writer.u64(self.spec().digests.root.0);
        writer.u32(self.spec().algorithms.state_codec);
        self.digest_into(&mut writer);
        StateChecksum(writer.finish().0)
    }
}

impl MatchSnapshot {
    /// Rebuilds the state, refusing anything but an exact version match.
    ///
    /// There is deliberately no approximate restore and no cross-version
    /// migration: a mismatch is reported and the caller decides.
    pub fn restore(self, spec: &LockedMatchSpec) -> Result<MatchState, SnapshotError> {
        if self.snapshot_schema_version != SNAPSHOT_SCHEMA_VERSION {
            return Err(SnapshotError::UnsupportedSchema {
                found: self.snapshot_schema_version,
                supported: SNAPSHOT_SCHEMA_VERSION,
            });
        }
        if self.digests.root != spec.digests.root {
            return Err(SnapshotError::DigestMismatch {
                found: self.digests.root,
                expected: spec.digests.root,
            });
        }
        let current = spec.algorithms;
        for (algorithm, found, expected) in [
            ("digest", self.algorithms.digest, current.digest),
            ("rng", self.algorithms.rng, current.rng),
            (
                "state-codec",
                self.algorithms.state_codec,
                current.state_codec,
            ),
        ] {
            if found != expected {
                return Err(SnapshotError::AlgorithmMismatch {
                    algorithm,
                    found,
                    expected,
                });
            }
        }
        Ok(self.state)
    }

    /// The captured state, without version checking. For inspection only.
    #[must_use]
    pub const fn state(&self) -> &MatchState {
        &self.state
    }
}

/// A tick at which a run must agree with the log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerificationCheckpoint {
    /// Match tick the checksum was taken at.
    pub match_tick: u64,
    /// Expected checksum.
    pub checksum: StateChecksum,
}

/// Everything needed to reproduce a match headlessly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationLog {
    /// Log format version.
    pub format_version: u32,
    /// Digest tree the log was recorded under.
    pub digests: MatchDigests,
    /// Algorithm versions the log was recorded under.
    pub algorithms: AlgorithmVersions,
    /// Seed every named stream derives from.
    pub root_seed: u64,
    /// Characters, per participant slot.
    pub characters: [CharacterId; PARTICIPANT_SLOTS],
    /// Per-tick inputs, one entry per participant slot.
    pub inputs: Vec<[PlayerActions; PARTICIPANT_SLOTS]>,
    /// Checkpoints the run is compared against.
    pub checkpoints: Vec<VerificationCheckpoint>,
}

/// Where a run and its log disagreed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChecksumDifference {
    /// Tick the checkpoint covers.
    pub match_tick: u64,
    /// Checksum the log expected.
    pub expected: StateChecksum,
    /// Checksum the run produced.
    pub actual: StateChecksum,
}

/// Result of replaying a log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationOutcome {
    /// Checksums taken at each checkpoint of this run.
    pub checkpoints: Vec<VerificationCheckpoint>,
    /// Checkpoints that disagreed with the log, in tick order.
    pub differences: Vec<ChecksumDifference>,
}

impl VerificationOutcome {
    /// Whether every checkpoint agreed.
    #[must_use]
    pub fn is_consistent(&self) -> bool {
        self.differences.is_empty()
    }
}

/// A log that cannot be run against this build.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VerificationError {
    /// The log format is not one this build reads.
    #[error("verification log format {found} is not supported (supported: {supported})")]
    UnsupportedFormat {
        /// Version found in the log.
        found: u32,
        /// Version this build supports.
        supported: u32,
    },
    /// The log was recorded against different rule content.
    #[error("verification log digest {found} does not match the supplied rules {expected}")]
    DigestMismatch {
        /// Root digest carried by the log.
        found: ContentDigest,
        /// Root digest of the supplied rules.
        expected: ContentDigest,
    },
    /// Freezing the log's selection failed.
    #[error("the log's selection could not be frozen: {0}")]
    Freeze(String),
}

/// Replays a log with no window, no filesystem access and no wall clock.
pub fn run_verification_log(
    log: &VerificationLog,
    library: &ValidatedRuleLibrary,
    rule_profile_id: &crate::config::RuleProfileId,
) -> Result<VerificationOutcome, VerificationError> {
    if log.format_version != VERIFICATION_LOG_FORMAT_VERSION {
        return Err(VerificationError::UnsupportedFormat {
            found: log.format_version,
            supported: VERIFICATION_LOG_FORMAT_VERSION,
        });
    }
    if log.digests.root != library.root_digest() {
        return Err(VerificationError::DigestMismatch {
            found: log.digests.root,
            expected: library.root_digest(),
        });
    }

    let spec = LockedMatchSpec::freeze(
        MatchRequest {
            rule_profile_id: rule_profile_id.clone(),
            root_seed: log.root_seed,
            characters: log.characters.clone(),
        },
        library,
    )
    .map_err(|error| VerificationError::Freeze(error.to_string()))?;

    let mut state = MatchState::new(spec);
    let mut checkpoints = Vec::new();
    let mut differences = Vec::new();
    let mut expected = log.checkpoints.iter().peekable();

    for actions in &log.inputs {
        let inputs = TickInputs::new(*actions).unwrap_or(TickInputs::EMPTY);
        // A malformed tick would change the state, so refuse it instead.
        if inputs.len() != PARTICIPANT_SLOTS {
            break;
        }
        if state.step(&inputs).is_err() {
            break;
        }
        let tick = state.match_tick();
        if expected
            .peek()
            .is_some_and(|point| point.match_tick == tick)
        {
            let point = expected.next().expect("peek said there is one");
            let actual = state.checksum();
            checkpoints.push(VerificationCheckpoint {
                match_tick: tick,
                checksum: actual,
            });
            if actual != point.checksum {
                differences.push(ChecksumDifference {
                    match_tick: tick,
                    expected: point.checksum,
                    actual,
                });
            }
        }
    }

    Ok(VerificationOutcome {
        checkpoints,
        differences,
    })
}

/// Records a log by running the inputs once and checkpointing as it goes.
///
/// This is how a log is produced in the first place; verifying it is then the
/// same code path with the expected values filled in.
#[must_use]
pub fn record_verification_log(
    spec: &LockedMatchSpec,
    inputs: Vec<[PlayerActions; PARTICIPANT_SLOTS]>,
    checkpoint_every: u64,
) -> VerificationLog {
    let mut state = MatchState::new(spec.clone());
    let mut checkpoints = Vec::new();
    for actions in &inputs {
        let tick_inputs = TickInputs::new(*actions).unwrap_or(TickInputs::EMPTY);
        if state.step(&tick_inputs).is_err() {
            break;
        }
        let tick = state.match_tick();
        if checkpoint_every > 0 && tick.is_multiple_of(checkpoint_every) {
            checkpoints.push(VerificationCheckpoint {
                match_tick: tick,
                checksum: state.checksum(),
            });
        }
    }
    VerificationLog {
        format_version: VERIFICATION_LOG_FORMAT_VERSION,
        digests: spec.digests.clone(),
        algorithms: spec.algorithms,
        root_seed: spec.root_seed,
        characters: spec.characters.clone(),
        inputs,
        checkpoints,
    }
}
