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
    pub const LEGEND: [&str; 2] = ["Shift+V⟨lb+down⟩/M⟨lb+up⟩", "Select Range/Mark Row"];

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
