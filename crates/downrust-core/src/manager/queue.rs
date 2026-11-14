// SPDX-License-Identifier: GPL-3.0-or-later
//! Queue order: computes the new queue positions for one move.
use crate::model::{DownloadId, QueueMove};

/// New queue positions for moving one queued download. `queued` lists the (id, position) pairs of
/// every queued download in queue order. The result holds only the entries whose position changes.
pub(super) fn queue_moves(
    queued: &[(DownloadId, i64)],
    id: DownloadId,
    to: QueueMove,
) -> Vec<(DownloadId, i64)> {
    let Some(index) = queued.iter().position(|(entry, _)| *entry == id) else {
        return Vec::new();
    };
    let last = queued.len() - 1;
    match to {
        QueueMove::Up if index > 0 => swap_positions(queued, index, index - 1),
        QueueMove::Down if index < last => swap_positions(queued, index, index + 1),
        QueueMove::Top if index > 0 => {
            let lowest = queued.iter().map(|(_, pos)| *pos).min().unwrap_or(0);
            vec![(id, lowest.saturating_sub(1))]
        }
        QueueMove::Bottom if index < last => {
            let highest = queued.iter().map(|(_, pos)| *pos).max().unwrap_or(0);
            vec![(id, highest.saturating_add(1))]
        }
        _ => Vec::new(),
    }
}

/// Exchanges the positions of two neighbours.
fn swap_positions(queued: &[(DownloadId, i64)], a: usize, b: usize) -> Vec<(DownloadId, i64)> {
    let (id_a, pos_a) = queued[a];
    let (id_b, pos_b) = queued[b];
    vec![(id_a, pos_b), (id_b, pos_a)]
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUEUE: [(DownloadId, i64); 3] = [(1, 1), (2, 2), (3, 3)];

    #[test]
    fn up_and_down_swap_with_the_neighbour() {
        assert_eq!(queue_moves(&QUEUE, 2, QueueMove::Up), vec![(2, 1), (1, 2)]);
        assert_eq!(
            queue_moves(&QUEUE, 2, QueueMove::Down),
            vec![(2, 3), (3, 2)]
        );
    }

    #[test]
    fn moves_past_the_ends_change_nothing() {
        assert!(queue_moves(&QUEUE, 1, QueueMove::Up).is_empty());
        assert!(queue_moves(&QUEUE, 1, QueueMove::Top).is_empty());
        assert!(queue_moves(&QUEUE, 3, QueueMove::Down).is_empty());
        assert!(queue_moves(&QUEUE, 3, QueueMove::Bottom).is_empty());
    }

    #[test]
    fn top_and_bottom_go_beyond_the_current_range() {
        assert_eq!(queue_moves(&QUEUE, 3, QueueMove::Top), vec![(3, 0)]);
        assert_eq!(queue_moves(&QUEUE, 1, QueueMove::Bottom), vec![(1, 4)]);
    }

    #[test]
    fn unknown_id_changes_nothing() {
        assert!(queue_moves(&QUEUE, 9, QueueMove::Top).is_empty());
        assert!(queue_moves(&[], 1, QueueMove::Up).is_empty());
    }
}
