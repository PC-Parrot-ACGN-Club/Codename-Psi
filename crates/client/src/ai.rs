//! Single-difficulty AI participant.
//!
//! The AI is an input producer, at the same level as local device input: it
//! reads the same [`PlayerView`] a person sees and emits the same normalized
//! action bitset. It has no entry point that sets coordinates, locks a group or
//! clears a board directly, and its own state never enters a rules snapshot.

use game_core::{
    board::{Board, Cell},
    falling::{DoubleRotation, FallingGroup, TRANSFORM_COUNT},
    input::{GameAction, PlayerActions},
    match_spec::LockedMatchSpec,
    resolution::ResolutionState,
    view::PlayerView,
};

/// Ticks between finishing a plan and emitting its first action.
///
/// A calibration value, not a derived one; see
/// `docs/development/decision/ai-baseline.md`.
pub const THINK_DELAY_TICKS: u16 = 6;

/// Ticks between two emitted actions.
pub const KEY_INTERVAL_TICKS: u16 = 2;

/// One reachable resting pose and the actions that get there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementCandidate {
    /// Orientation at rest.
    pub transform: u8,
    /// Pivot column at rest.
    pub column: u8,
    /// A legal action path that reaches it.
    pub actions: Vec<GameAction>,
    /// What the rules sandbox says this placement does.
    pub score: CandidateScore,
    /// Position in the canonical enumeration order, which breaks exact ties.
    pub order: usize,
}

/// Integer features of one candidate, compared in a fixed order.
///
/// There is no weighted total: the layers are compared lexicographically, so a
/// lower layer can never outvote survival.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CandidateScore {
    /// Whether the next group can still spawn after this placement.
    pub survives: bool,
    /// Nuisance this placement's chain would cancel.
    pub offsets: u32,
    /// Whether it takes a Fever opportunity that is on offer.
    pub takes_fever: bool,
    /// Links the chain reaches.
    pub links: u8,
    /// Colored balls cleared.
    pub cleared: u32,
    /// Whether the visible board ends empty.
    pub all_clear: bool,
    /// Height of the tallest column afterwards.
    pub max_height: u8,
    /// Empty cells with something above them.
    pub holes: u16,
}

impl CandidateScore {
    /// The lexicographic comparison key, most important layer first.
    #[must_use]
    pub fn key(&self) -> [i64; 7] {
        [
            i64::from(self.survives),
            i64::from(self.offsets),
            i64::from(self.takes_fever),
            i64::from(self.links),
            i64::from(self.all_clear),
            -i64::from(self.holes),
            -i64::from(self.max_height),
        ]
    }
}

/// The placement the AI committed to for one turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementPlan {
    /// Turn this plan belongs to.
    pub turn_id: u32,
    /// Actions to emit, in order, ending in a hard drop.
    pub actions: Vec<GameAction>,
    /// Orientation the plan aims for.
    pub transform: u8,
    /// Column the plan aims for.
    pub column: u8,
}

/// Execution state for one AI-driven participant.
///
/// Deliberately not part of `MatchState`: AI plans and their timing are not
/// rules state and must never enter a snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AiControllerState {
    plan: Option<PlacementPlan>,
    cursor: usize,
    wait: u16,
    observed_turn: Option<u32>,
    observed_board: Option<Board>,
    plans_made: u32,
}

impl AiControllerState {
    /// A controller that has not planned yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The plan currently being executed.
    #[must_use]
    pub const fn plan(&self) -> Option<&PlacementPlan> {
        self.plan.as_ref()
    }

    /// How many plans this controller has produced.
    #[must_use]
    pub const fn plans_made(&self) -> u32 {
        self.plans_made
    }

    /// Produces this tick's actions.
    ///
    /// Replanning happens when the turn changes or when the observation the
    /// plan was made against no longer holds — a nuisance drop or a Fever
    /// switch both change the board under the plan.
    pub fn step(&mut self, view: &PlayerView, spec: &LockedMatchSpec) -> PlayerActions {
        let Some(group) = view.active_group.as_ref() else {
            self.plan = None;
            self.observed_turn = None;
            return PlayerActions::EMPTY;
        };

        let stale = self.observed_turn != Some(view.turn_id)
            || self.observed_board.as_ref() != Some(&view.board);
        if stale {
            let plan = plan_placement(view, spec, group);
            self.plan = Some(plan);
            self.cursor = 0;
            self.wait = THINK_DELAY_TICKS;
            self.observed_turn = Some(view.turn_id);
            self.observed_board = Some(view.board.clone());
            self.plans_made += 1;
        }

        if self.wait > 0 {
            self.wait -= 1;
            return PlayerActions::EMPTY;
        }
        let Some(plan) = &self.plan else {
            return PlayerActions::EMPTY;
        };
        let Some(action) = plan.actions.get(self.cursor).copied() else {
            // Every plan ends in a hard drop, so running out means the lock
            // already happened and the next view will carry a new turn.
            return PlayerActions::EMPTY;
        };
        self.cursor += 1;
        self.wait = KEY_INTERVAL_TICKS.saturating_sub(1);
        PlayerActions::from(action).normalized()
    }
}

/// Enumerates candidates and picks one, deterministically.
#[must_use]
pub fn plan_placement(
    view: &PlayerView,
    spec: &LockedMatchSpec,
    group: &FallingGroup,
) -> PlacementPlan {
    let candidates = generate_candidates(view, spec, group);
    // A candidate set is never empty: placements that lose are kept and simply
    // sort last, so the AI always hands over a placement.
    let best = candidates
        .iter()
        .max_by(|left, right| {
            left.score
                .key()
                .cmp(&right.score.key())
                // Ties fall back to the canonical enumeration order rather
                // than to whatever order the container happened to yield.
                .then(right.order.cmp(&left.order))
        })
        .expect("hard dropping in place is always a candidate");

    PlacementPlan {
        turn_id: view.turn_id,
        actions: best.actions.clone(),
        transform: best.transform,
        column: best.column,
    }
}

/// Every resting pose reachable by rotating, sliding and hard dropping.
#[must_use]
pub fn generate_candidates(
    view: &PlayerView,
    spec: &LockedMatchSpec,
    group: &FallingGroup,
) -> Vec<PlacementCandidate> {
    let width = spec.board_geometry.width();
    let mut candidates = Vec::new();
    let mut order = 0;

    for turns in 0..TRANSFORM_COUNT {
        for column in 0..width {
            let Some((posed, actions)) = reach(view, group, turns, column) else {
                continue;
            };
            let score = evaluate(view, spec, &posed);
            candidates.push(PlacementCandidate {
                transform: posed.transform_id(),
                column: posed.pivot().x(),
                actions,
                score,
                order,
            });
            order += 1;
        }
    }
    candidates
}

/// Walks a candidate path with the real geometry, or reports it unreachable.
fn reach(
    view: &PlayerView,
    group: &FallingGroup,
    turns: u8,
    column: u8,
) -> Option<(FallingGroup, Vec<GameAction>)> {
    let board = &view.board;
    let mut posed = *group;
    let mut actions = Vec::new();
    let mut counter = DoubleRotation::new();

    for _ in 0..turns {
        let before = posed.transform_id();
        posed.rotate(board, true, &mut counter, 2, 4);
        if posed.transform_id() == before {
            // A rotation the board refuses makes the whole pose unreachable.
            return None;
        }
        actions.push(GameAction::RotateClockwise);
    }

    while posed.pivot().x() != column {
        let step = if posed.pivot().x() > column { -1 } else { 1 };
        if !posed.try_translate(board, step, 0) {
            return None;
        }
        actions.push(if step < 0 {
            GameAction::Left
        } else {
            GameAction::Right
        });
    }

    while posed.try_translate(board, 0, 1) {}
    actions.push(GameAction::HardDrop);
    Some((posed, actions))
}

/// Runs one placement through the real rules and reads the outcome.
///
/// The sandbox reuses the production lock and resolution functions rather than
/// keeping a second copy of the clearing or scoring rules.
fn evaluate(view: &PlayerView, spec: &LockedMatchSpec, posed: &FallingGroup) -> CandidateScore {
    let mut board = view.board.clone();
    if posed.lock(&mut board).is_err() {
        return CandidateScore {
            survives: false,
            offsets: 0,
            takes_fever: false,
            links: 0,
            cleared: 0,
            all_clear: false,
            max_height: spec.board_geometry.height(),
            holes: u16::MAX,
        };
    }

    let mut resolution = ResolutionState::new(board, spec.resolution.clone());
    let report = resolution.settle().clone();
    let settled = resolution.board().clone();

    let incoming = view.pending[view.active_channel];
    let cleared = report.total_cleared_colored;
    // A chain of any size stops this turn's drop and cancels part of the queue.
    let offsets = if report.links.is_empty() {
        0
    } else {
        incoming.min(cleared)
    };
    let takes_fever = !report.links.is_empty()
        && view.fever_gauge + 1 >= view.fever_capacity
        && incoming > 0
        && !view.in_fever;

    CandidateScore {
        survives: spawn_is_free(&settled, spec),
        offsets,
        takes_fever,
        links: report.links.len() as u8,
        cleared,
        all_clear: report.field.all_clear,
        max_height: max_height(&settled),
        holes: holes(&settled),
    }
}

/// Whether the next group's spawn pose would still be free.
fn spawn_is_free(board: &Board, spec: &LockedMatchSpec) -> bool {
    let column = spec.board_geometry.spawn_column();
    let rows = spec.board_geometry.hidden_rows();
    (0..rows).all(|y| {
        board
            .coord(column, y)
            .is_some_and(|coord| !board.get(coord).is_occupied())
    })
}

fn max_height(board: &Board) -> u8 {
    let geometry = board.geometry();
    let mut tallest = 0;
    for x in 0..geometry.width() {
        for y in 0..geometry.height() {
            if board
                .coord(x, y)
                .is_some_and(|coord| board.get(coord).is_occupied())
            {
                tallest = tallest.max(geometry.height() - y);
                break;
            }
        }
    }
    tallest
}

fn holes(board: &Board) -> u16 {
    let geometry = board.geometry();
    let mut count = 0;
    for x in 0..geometry.width() {
        let mut covered = false;
        for y in 0..geometry.height() {
            let Some(coord) = board.coord(x, y) else {
                continue;
            };
            if board.get(coord) == Cell::Empty {
                if covered {
                    count += 1;
                }
            } else {
                covered = true;
            }
        }
    }
    count
}
