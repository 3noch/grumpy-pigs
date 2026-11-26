//! Seasonal Easter eggs. On US Thanksgiving the game dresses up: the pigs
//! become pilgrims, the wolves turkeys and the fairies cherubs, and the game
//! is briefly called Peckish Pilgrims.

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Holiday {
    None = 0,
    Thanksgiving = 1,
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn current() -> Holiday {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Holiday::Thanksgiving,
        _ => Holiday::None,
    }
}

pub fn thanksgiving() -> bool {
    current() == Holiday::Thanksgiving
}

pub fn set(h: Holiday) {
    CURRENT.store(h as u8, Ordering::Relaxed);
}

/// Day of the week, 0 = Sunday (Sakamoto's method).
fn weekday(year: i32, month: u32, day: u32) -> u32 {
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if month < 3 { year - 1 } else { year };
    let m = usize::try_from(month.clamp(1, 12) - 1).unwrap_or(0);
    let d = i32::try_from(day).unwrap_or(1);
    (y + y / 4 - y / 100 + y / 400 + T[m] + d)
        .rem_euclid(7)
        .unsigned_abs()
}

/// US Thanksgiving: the fourth Thursday of November.
pub fn is_thanksgiving(year: i32, month: u32, day: u32) -> bool {
    month == 11 && weekday(year, month, day) == 4 && (22..=28).contains(&day)
}

/// The holiday (if any) for a date.
pub fn for_date(year: i32, month: u32, day: u32) -> Holiday {
    if is_thanksgiving(year, month, day) {
        Holiday::Thanksgiving
    } else {
        Holiday::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_thanksgiving() {
        for (y, d) in [(2024, 28), (2025, 27), (2026, 26), (2027, 25), (2030, 28)] {
            assert!(is_thanksgiving(y, 11, d), "{y}-11-{d}");
            assert!(!is_thanksgiving(y, 11, d - 7), "{y}-11-{}", d - 7);
            assert!(!is_thanksgiving(y, 11, d + 1), "{y}-11-{}", d + 1);
        }
        assert!(!is_thanksgiving(2026, 10, 22));
    }
}
