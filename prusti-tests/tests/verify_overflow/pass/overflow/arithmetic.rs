use prusti_contracts::*;

/// From time::duration[0]::div_rem_64[0]
#[requires(other != 0 && other != -1)]
fn div_rem_64(this: i64, other: i64) -> (i64, i64) {
    (this / other, this % other)
}

/// Source: num_traits::ops[0]::mul_add[0]::{{impl}}[*]::mul_add_assign[0]
#[requires(-1000 < *s && *s < 1000)]
//#[requires(-1000 < a && a < 1000)]   // Non-linear arithmetic makes the test unstable.
#[requires(a == 1000)]
#[requires(-1000 < b && b < 1000)]
fn mul_add_assign(s: &mut i32, a: i32, b: i32) {
    *s = (*s * a) + b
}

/// From time::display[0]::iso_week_days[0]
#[requires(0 <= yday && yday <= 365 * 5)]
#[requires(0 <= wday && wday <= 6)]
fn iso_week_days(yday: i32, wday: i32) -> i32 {
    /* The number of days from the first day of the first ISO week of this
    * year to the year day YDAY with week day WDAY.
    * ISO weeks start on Monday. The first ISO week has the year's first
    * Thursday.
    * YDAY may be as small as yday_minimum.
    */
    let iso_week_start_wday: i32 = 1;                     /* Monday */
    let iso_week1_wday: i32 = 4;                          /* Thursday */
    let yday_minimum: i32 = 366;
    /* Add enough to the first operand of % to make it nonnegative. */
    let big_enough_multiple_of_7: i32 = (yday_minimum / 7 + 2) * 7;

    yday - (yday - wday + iso_week1_wday + big_enough_multiple_of_7) % 7
        + iso_week1_wday - iso_week_start_wday
}
