//! Deterministic half-open schedules. This crate validates positions and
//! identities, never storage layouts, runtime qualification or carryover.
use std::fmt;

/// Lexicographic execution position; no arithmetic or ordinal sentinel is used.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub block: u64,
    pub ordinal: u64,
}
impl Position {
    pub const fn new(block: u64, ordinal: u64) -> Self {
        Self { block, ordinal }
    }
}

/// A schedule entry owns `start <= position < end`; `None` has no upper bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Interval {
    pub start: Position,
    pub end: Option<Position>,
    /// A later entry for the same market; consumers must initialize it afresh.
    pub successor: bool,
}
impl Interval {
    pub const fn unbounded(start: Position) -> Self {
        Self {
            start,
            end: None,
            successor: false,
        }
    }
    pub fn contains(self, position: Position) -> bool {
        self.start <= position && self.end.is_none_or(|end| position < end)
    }
    /// Whether any position in this block belongs to the interval.
    pub fn intersects_block(self, block: u64) -> bool {
        self.start.block <= block && self.end.is_none_or(|end| end.block > block || (end.block == block && end.ordinal > 0))
    }
    /// A prefix in this block cannot represent END_OF_BLOCK state.
    pub fn ends_in_block(self, block: u64) -> bool {
        self.end.is_some_and(|end| end.block == block)
    }
    pub fn overlaps(self, other: Self) -> bool {
        self.end.is_none_or(|end| other.start < end) && other.end.is_none_or(|end| self.start < end)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScheduleError {
    ZeroEpoch,
    DuplicateStart,
    NonIncreasingEpoch,
}
impl fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ZeroEpoch => "epoch must be positive",
            Self::DuplicateStart => "duplicate epoch activation position within a market",
            Self::NonIncreasingEpoch => "duplicate or decreasing epoch IDs in activation order",
        })
    }
}
impl std::error::Error for ScheduleError {}

/// Normalize each market by activation position, require strictly increasing
/// positive IDs, and return intervals aligned with the original input order.
/// Different markets may use the same IDs and positions. IDs need not be consecutive.
pub fn schedule<K: Ord>(entries: impl IntoIterator<Item = (K, u32, Position)>) -> Result<Vec<Interval>, ScheduleError> {
    let mut entries: Vec<_> = entries.into_iter().enumerate().collect();
    entries.sort_by(|(_, a), (_, b)| (&a.0, a.2).cmp(&(&b.0, b.2)));
    let mut result = vec![Interval::unbounded(Position::default()); entries.len()];
    for (i, (original, (market, epoch, start))) in entries.iter().enumerate() {
        if *epoch == 0 {
            return Err(ScheduleError::ZeroEpoch);
        }
        let previous = i.checked_sub(1).map(|j| &entries[j].1).filter(|entry| entry.0 == *market);
        if let Some((_, previous_epoch, previous_start)) = previous {
            if previous_start == start {
                return Err(ScheduleError::DuplicateStart);
            }
            if previous_epoch >= epoch {
                return Err(ScheduleError::NonIncreasingEpoch);
            }
        }
        let end = entries.get(i + 1).filter(|(_, entry)| entry.0 == *market).map(|(_, entry)| entry.2);
        result[*original] = Interval {
            start: *start,
            end,
            successor: previous.is_some(),
        };
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unordered_nonconsecutive_schedules_preserve_input_alignment() {
        let rows = schedule([
            ("a", 9, Position::new(3, 40)),
            ("b", 1, Position::new(2, 30)),
            ("a", 1, Position::new(2, 30)),
            ("a", 3, Position::new(3, 10)),
        ])
        .unwrap();
        assert_eq!(rows[2].end, Some(Position::new(3, 10)));
        assert_eq!(rows[3].end, Some(Position::new(3, 40)));
        assert!(rows[0].successor && rows[3].successor && !rows[1].successor && !rows[2].successor);
        assert!(rows[2].contains(Position::new(2, 30)));
        assert!(!rows[2].contains(Position::new(3, 10)));
        assert!(rows[3].contains(Position::new(3, 10)));
        assert!(!rows[3].contains(Position::new(3, 40)));
        assert!(!rows[2].overlaps(rows[3]));
        assert!(rows[1].overlaps(rows[0]));
    }

    #[test]
    fn invalid_ids_and_boundaries_are_refused() {
        let p = Position::new(1, 0);
        let q = Position::new(2, 0);
        assert_eq!(schedule([(0, 0, p)]), Err(ScheduleError::ZeroEpoch));
        assert_eq!(schedule([(0, 1, p), (0, 2, p)]), Err(ScheduleError::DuplicateStart));
        for ids in [(1, 1), (3, 1)] {
            assert_eq!(schedule([(0, ids.0, p), (0, ids.1, q)]), Err(ScheduleError::NonIncreasingEpoch));
        }
        assert!(schedule::<u8>([]).unwrap().is_empty());
    }

    #[test]
    fn block_zero_max_positions_and_block_boundaries_need_no_sentinel() {
        let rows = schedule([
            (0, 1, Position::new(0, 0)),
            (0, 3, Position::new(1, 0)),
            (0, 4, Position::new(u64::MAX, u64::MAX)),
        ])
        .unwrap();
        assert!(rows[0].contains(Position::new(0, u64::MAX)));
        assert!(!rows[0].intersects_block(1));
        assert!(rows[0].ends_in_block(1));
        assert!(rows[1].contains(Position::new(u64::MAX, u64::MAX - 1)));
        assert!(rows[2].contains(Position::new(u64::MAX, u64::MAX)));
        assert!(rows[1].intersects_block(u64::MAX) && rows[2].intersects_block(u64::MAX));
        assert!(!rows[2].intersects_block(u64::MAX - 1));
    }
}
