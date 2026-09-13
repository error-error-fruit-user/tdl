pub fn next(index: usize, count: usize) -> usize {
    if index == count - 1 { 0 } else { index + 1 }
}

pub fn previous(index: usize, count: usize) -> usize {
    if index == 0 { count - 1 } else { index - 1 }
}

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
