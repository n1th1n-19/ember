// SPDX-License-Identifier: GPL-3.0-or-later
//! Local date/time without a date crate: libc localtime + a month grid.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Now {
    pub year: i32,
    /// 1..=12
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    /// 0 = Sunday
    pub weekday: u32,
}

pub fn now() -> Now {
    // SAFETY: localtime_r writes into our zeroed struct; time(NULL) is always valid
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        Now { year: tm.tm_year + 1900, month: tm.tm_mon as u32 + 1, day: tm.tm_mday as u32, hour: tm.tm_hour as u32, minute: tm.tm_min as u32, weekday: tm.tm_wday as u32 }
    }
}

pub fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

pub fn days_in(y: i32, m: u32) -> u32 {
    match m {
        2 if is_leap(y) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Day of week, 0 = Sunday (Sakamoto's method).
pub fn weekday(y: i32, m: u32, d: u32) -> u32 {
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if m < 3 { y - 1 } else { y };
    ((y + y / 4 - y / 100 + y / 400 + T[m as usize - 1] + d as i32).rem_euclid(7)) as u32
}

/// Six Sunday-first weeks; days outside the month are None.
pub fn month_grid(y: i32, m: u32) -> [[Option<u32>; 7]; 6] {
    let mut g = [[None; 7]; 6];
    let start = weekday(y, m, 1) as usize;
    for d in 1..=days_in(y, m) {
        let i = start + d as usize - 1;
        g[i / 7][i % 7] = Some(d);
    }
    g
}

pub fn shift_month(y: i32, m: u32, by: i32) -> (i32, u32) {
    let idx = y * 12 + m as i32 - 1 + by;
    (idx.div_euclid(12), idx.rem_euclid(12) as u32 + 1)
}

pub const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
pub const WEEKDAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_math() {
        assert_eq!(weekday(2026, 11, 11), 3); // Wednesday
        assert_eq!(weekday(2000, 1, 1), 6);
        assert_eq!(days_in(2024, 2), 29);
        assert_eq!(days_in(2100, 2), 28);
        let g = month_grid(2026, 11);
        assert_eq!(g[0][0], Some(1)); // Nov 2026 starts on Sunday
        assert_eq!(g[1][3], Some(11));
        assert_eq!(g[4][1], Some(30));
        assert_eq!(shift_month(2026, 12, 1), (2027, 1));
        assert_eq!(shift_month(2026, 1, -1), (2025, 12));
        let n = now();
        assert!((1..=12).contains(&n.month) && n.weekday < 7);
    }
}
