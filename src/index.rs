//! Wrap-around index math for selectable lists.

/// The index after `index`, wrapping to the start.
pub fn next(index: usize, count: usize) -> usize {
    if index == count - 1 { 0 } else { index + 1 }
}

/// The index before `index`, wrapping to the end.
pub fn previous(index: usize, count: usize) -> usize {
    if index == 0 { count - 1 } else { index - 1 }
}

/// Returns whether the selection changed (and so needs a redraw).
pub fn select_next(index: &mut Option<usize>, count: usize) -> bool {
    let Some(index) = index else {
        return false;
    };
    if count == 1 {
        return false;
    }
    *index = next(*index, count);
    true
}

/// Like [`select_next`], but backwards.
pub fn select_previous(index: &mut Option<usize>, count: usize) -> bool {
    let Some(index) = index else {
        return false;
    };
    if count == 1 {
        return false;
    }
    *index = previous(*index, count);
    true
}
