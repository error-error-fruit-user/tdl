//! Picking several rows at once, to copy them together. `Shift+V` starts a range at the cursor that
//! stretches as the cursor moves, and `M` marks single rows, for rows that aren't next to each
//! other. Both can be used at once. The rows always come out in list order, whatever order they
//! were picked in, so a wad's place in the load order is never scrambled.

use std::collections::BTreeSet;

#[derive(Debug, Default)]
pub struct Selection {
    /// Where the range was started, while one is being stretched.
    anchor: Option<usize>,
    marked: BTreeSet<usize>,
}

impl Selection {
    pub const LEGEND: [&str; 2] = ["Shift+V/M", "Select Range/Mark Row"];

    /// Starts a range at `cursor`, or ends the one being stretched, keeping its rows marked.
    pub fn toggle_range(&mut self, cursor: usize, count: usize) {
        match self.anchor.take() {
            Some(anchor) => self.marked.extend(range(anchor, cursor, count)),
            None if cursor < count => self.anchor = Some(cursor),
            None => {}
        }
    }

    /// Marks or unmarks the row at `row`. Rows past the last (an add row) can't be marked.
    pub fn toggle_mark(&mut self, row: usize, count: usize) {
        if row < count && !self.marked.remove(&row) {
            self.marked.insert(row);
        }
    }

    /// Whether anything is picked, so `Esc` clears it instead of leaving the screen.
    pub fn is_active(&self) -> bool {
        self.anchor.is_some() || !self.marked.is_empty()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Whether `row` is picked, by a mark or the range from its anchor to `cursor`.
    pub fn contains(&self, row: usize, cursor: usize, count: usize) -> bool {
        self.marked.contains(&row)
            || self
                .anchor
                .is_some_and(|anchor| range(anchor, cursor, count).contains(&row))
    }

    /// The rows picked, in list order. With nothing picked, the row under the cursor, unless that
    /// is past the last row.
    pub fn rows(&self, cursor: usize, count: usize) -> Vec<usize> {
        let mut rows = self.marked.clone();
        if let Some(anchor) = self.anchor {
            rows.extend(range(anchor, cursor, count));
        }
        if rows.is_empty() && cursor < count {
            rows.insert(cursor);
        }
        rows.into_iter().filter(|&row| row < count).collect()
    }

    /// How many rows are picked, for the note under the window. 0 when nothing is.
    pub fn picked(&self, cursor: usize, count: usize) -> usize {
        if self.is_active() {
            self.rows(cursor, count).len()
        } else {
            0
        }
    }
}

impl Selection {
    /// Picks exactly `rows`, as marks, for after they have moved.
    pub fn select_rows(&mut self, rows: impl IntoIterator<Item = usize>) {
        self.anchor = None;
        self.marked = rows.into_iter().collect();
    }
}

/// Moves the items at `rows` (in list order) one place in `list`, down or up, keeping their order
/// and stepping over whatever is between them. Nothing moves if one is already at the end it is
/// heading for. Returns where they are now.
pub fn shift<T>(list: &mut [T], rows: &[usize], down: bool) -> Option<Vec<usize>> {
    let (&first, &last) = (rows.first()?, rows.last()?);
    if down {
        if last + 1 >= list.len() {
            return None;
        }
        for &row in rows.iter().rev() {
            list.swap(row, row + 1);
        }
        Some(rows.iter().map(|row| row + 1).collect())
    } else {
        if first == 0 {
            return None;
        }
        for &row in rows {
            list.swap(row, row - 1);
        }
        Some(rows.iter().map(|row| row - 1).collect())
    }
}

/// The rows from `anchor` to `cursor`, whichever comes first, leaving out anything past the last.
fn range(anchor: usize, cursor: usize, count: usize) -> std::ops::RangeInclusive<usize> {
    let (start, end) = (anchor.min(cursor), anchor.max(cursor));
    start..=end.min(count.saturating_sub(1))
}

/// What the note under the window says about the rows picked, if any are.
pub fn note(picked: usize) -> Option<String> {
    (picked > 0).then(|| format!("{picked} selected"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_nothing_picked_the_cursor_row_is_the_one() {
        let selection = Selection::default();
        assert!(!selection.is_active());
        assert_eq!(selection.rows(2, 5), [2]);
        // the add row is nothing to copy
        assert!(selection.rows(5, 5).is_empty());
        assert_eq!(selection.picked(2, 5), 0);
    }

    #[test]
    fn a_range_stretches_with_the_cursor_either_way() {
        let mut selection = Selection::default();
        selection.toggle_range(2, 6);
        assert!(selection.is_active());
        assert_eq!(selection.rows(4, 6), [2, 3, 4]);
        assert_eq!(selection.rows(0, 6), [0, 1, 2]);
        assert!(selection.contains(1, 0, 6));
        assert!(!selection.contains(3, 0, 6));
        // onto the add row, it stops at the last real row
        assert_eq!(selection.rows(6, 6), [2, 3, 4, 5]);
    }

    #[test]
    fn ending_a_range_keeps_its_rows_and_marks_add_to_them_in_list_order() {
        let mut selection = Selection::default();
        selection.toggle_mark(5, 8);
        selection.toggle_range(1, 8);
        selection.toggle_range(2, 8);
        selection.toggle_mark(0, 8);
        assert_eq!(selection.rows(7, 8), [0, 1, 2, 5]);
        assert_eq!(selection.picked(7, 8), 4);
        // marking again unmarks
        selection.toggle_mark(5, 8);
        assert_eq!(selection.rows(7, 8), [0, 1, 2]);
        selection.clear();
        assert!(!selection.is_active());
    }

    #[test]
    fn the_add_row_cannot_be_picked() {
        let mut selection = Selection::default();
        selection.toggle_mark(3, 3);
        selection.toggle_range(3, 3);
        assert!(!selection.is_active());
    }

    #[test]
    fn shifting_moves_the_picked_rows_together_and_stops_at_the_ends() {
        let mut list = ['a', 'b', 'c', 'd', 'e'];
        assert_eq!(shift(&mut list, &[1, 2], true), Some(vec![2, 3]));
        assert_eq!(list, ['a', 'd', 'b', 'c', 'e']);
        // rows apart move apart, each over its neighbour
        assert_eq!(
            shift(&mut list, &[0, 3], false),
            None,
            "the first can't go up"
        );
        assert_eq!(shift(&mut list, &[1, 3], false), Some(vec![0, 2]));
        assert_eq!(list, ['d', 'a', 'c', 'b', 'e']);
        assert_eq!(shift(&mut list, &[4], true), None);
        assert_eq!(list, ['d', 'a', 'c', 'b', 'e'], "nothing moved");
        assert_eq!(shift(&mut list, &[], true), None);
    }

    #[test]
    fn moved_rows_stay_picked() {
        let mut selection = Selection::default();
        selection.toggle_range(1, 5);
        selection.select_rows([2, 3]);
        assert_eq!(selection.rows(0, 5), [2, 3]);
        assert!(selection.is_active());
    }

    #[test]
    fn the_note_only_speaks_up_when_something_is_picked() {
        assert_eq!(note(0), None);
        assert_eq!(note(3).as_deref(), Some("3 selected"));
    }
}
