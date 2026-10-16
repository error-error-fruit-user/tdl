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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_and_previous_wrap_at_the_ends() {
        assert_eq!(next(0, 3), 1);
        assert_eq!(next(2, 3), 0);
        assert_eq!(previous(1, 3), 0);
        assert_eq!(previous(0, 3), 2);
    }

    #[test]
    fn selecting_walks_the_whole_list_and_comes_back_around() {
        let mut index = Some(0);
        let visited: Vec<_> = (0..4)
            .map(|_| {
                assert!(select_next(&mut index, 3));
                index.unwrap()
            })
            .collect();
        assert_eq!(visited, [1, 2, 0, 1]);
        let visited: Vec<_> = (0..4)
            .map(|_| {
                assert!(select_previous(&mut index, 3));
                index.unwrap()
            })
            .collect();
        assert_eq!(visited, [0, 2, 1, 0]);
    }

    #[test]
    fn a_single_row_or_no_selection_never_moves_and_asks_for_no_redraw() {
        let mut only = Some(0);
        assert!(!select_next(&mut only, 1));
        assert!(!select_previous(&mut only, 1));
        assert_eq!(only, Some(0));
        let mut none = None;
        assert!(!select_next(&mut none, 3));
        assert!(!select_previous(&mut none, 3));
        assert_eq!(none, None);
    }
}
