pub const ROWS: usize = 5;

pub fn visible_range(count: usize, selection: usize) -> core::ops::Range<usize> {
    let start = selection.min(count.saturating_sub(1)) / ROWS * ROWS;
    start..count.min(start.saturating_add(ROWS))
}

pub fn navigate(count: usize, selection: usize, delta: isize) -> usize {
    selection
        .saturating_add_signed(delta)
        .min(count.saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_and_navigation_stay_bounded() {
        assert_eq!(visible_range(0, 0), 0..0);
        assert_eq!(visible_range(25, 4), 0..5);
        assert_eq!(visible_range(25, 5), 5..10);
        assert_eq!(visible_range(23, 22), 20..23);
        assert_eq!(navigate(0, 0, 1), 0);
        assert_eq!(navigate(25, 0, -5), 0);
        assert_eq!(navigate(25, 23, 5), 24);
        assert_eq!(navigate(25, 5, -1), 4);
    }
}
