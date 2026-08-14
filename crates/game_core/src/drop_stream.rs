//! Supply of falling groups: the drop cursor, its L/J cycle, and the NEXT queue.
//!
//! Colors are drawn when a hand *enters* the queue, so what the player sees in
//! NEXT is exactly what spawns. Hands beyond the queue are not drawn yet and
//! therefore cannot leak through any read model.

use std::collections::VecDeque;

use crate::{
    board::{Board, BoardGeometry},
    config::{ColorDraw, DropSet, DropTemplate},
    determinism::MatchRng,
    falling::FallingGroup,
};

/// A hand that has been drawn and is waiting in NEXT.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingHand {
    /// Shape and color layout, with the L/J cycle already applied.
    pub template: DropTemplate,
    /// The hand's drawn colors, first then second.
    pub colors: [u8; 2],
    /// Turn number this hand will spawn as.
    pub turn_id: u32,
}

/// The per-player supply of hands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DropStream {
    drop_set: DropSet,
    cursor: u64,
    queue: VecDeque<PendingHand>,
    queue_len: usize,
}

impl DropStream {
    /// Fills the NEXT queue from a fresh cursor.
    #[must_use]
    pub fn new(drop_set: DropSet, queue_len: u8, color_count: u8, rng: &mut MatchRng) -> Self {
        let mut stream = Self {
            drop_set,
            cursor: 0,
            queue: VecDeque::new(),
            queue_len: usize::from(queue_len).max(1),
        };
        stream.refill(color_count, rng);
        stream
    }

    /// The hand that will spawn next.
    #[must_use]
    pub fn peek(&self) -> Option<PendingHand> {
        self.queue.front().copied()
    }

    /// The queued hands, oldest first. Nothing beyond the queue is visible.
    pub fn queued(&self) -> impl Iterator<Item = PendingHand> + '_ {
        self.queue.iter().copied()
    }

    /// Position in the drop cycle, counting every hand ever drawn.
    #[must_use]
    pub const fn cursor(&self) -> u64 {
        self.cursor
    }

    /// Whether this drop set swaps L and J on alternate cycles.
    #[must_use]
    pub fn swaps_l_and_j(&self) -> bool {
        self.drop_set.swaps_l_and_j()
    }

    /// Removes the front hand and draws a replacement.
    pub fn take(&mut self, color_count: u8, rng: &mut MatchRng) -> Option<PendingHand> {
        let hand = self.queue.pop_front()?;
        self.refill(color_count, rng);
        Some(hand)
    }

    fn refill(&mut self, color_count: u8, rng: &mut MatchRng) {
        while self.queue.len() < self.queue_len {
            let template = self.drop_set.hand(self.cursor);
            let colors = draw_colors(template, color_count, rng);
            let turn_id = self.cursor as u32;
            self.cursor += 1;
            self.queue.push_back(PendingHand {
                template,
                colors,
                turn_id,
            });
        }
    }
}

/// Draws one hand's colors from a participant's color stream.
///
/// A dual-color `O` needs two different colors, so its second draw picks from
/// the remaining colors rather than rejecting and retrying: a retry loop would
/// consume a data-dependent number of random values, which would make the
/// stream position depend on the draws themselves.
fn draw_colors(template: DropTemplate, color_count: u8, rng: &mut MatchRng) -> [u8; 2] {
    let colors = u32::from(color_count).max(1);
    let first = (rng.next_u32() % colors) as u8;
    match template.color_draw() {
        ColorDraw::Single => [first, first],
        ColorDraw::Independent => [first, (rng.next_u32() % colors) as u8],
        ColorDraw::Distinct => {
            if colors < 2 {
                return [first, first];
            }
            let offset = 1 + rng.next_u32() % (colors - 1);
            [first, ((u32::from(first) + offset) % colors) as u8]
        }
    }
}

/// No group could be supplied, which loses the round for that participant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the spawn pose is blocked")]
pub struct SpawnBlocked;

/// Supplies the next group, or reports the spawn failure that ends a round.
///
/// The spawn pose is tested *before* the hand leaves NEXT, so a failure leaves
/// the cursor and the queue exactly where they were. This is also the only
/// moment a defeat can be decided.
pub fn spawn_group(
    board: &Board,
    stream: &mut DropStream,
    geometry: BoardGeometry,
    color_count: u8,
    rng: &mut MatchRng,
) -> Result<FallingGroup, SpawnBlocked> {
    let hand = stream.peek().ok_or(SpawnBlocked)?;
    let pivot = board
        .coord(
            geometry.spawn_column(),
            geometry.hidden_rows().saturating_sub(1),
        )
        .ok_or(SpawnBlocked)?;
    let group = FallingGroup::new(hand.template, hand.colors, pivot, hand.turn_id);
    if !group.can_place(board) {
        return Err(SpawnBlocked);
    }
    stream.take(color_count, rng);
    Ok(group)
}
