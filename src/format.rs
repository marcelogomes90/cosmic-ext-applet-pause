use std::time::Duration;

use crate::fl;
use crate::pause::model::Moment;

pub fn remaining(span: Duration) -> String {
    let total = span.as_secs();

    if total == 0 {
        return fl!("duration-now");
    }

    let minutes: u64 = total.div_ceil(60);

    if minutes < 60 {
        return fl!("duration-minutes", minutes = minutes);
    }

    let hours: u64 = minutes / 60;
    let rest: u64 = minutes % 60;

    if rest == 0 {
        fl!("duration-hours", hours = hours)
    } else {
        fl!("duration-hours-minutes", hours = hours, minutes = rest)
    }
}

pub fn interval(span: Duration) -> String {
    let minutes: u64 = span.as_secs() / 60;

    fl!("settings-every", minutes = minutes)
}

pub fn clock(moment: Moment) -> String {
    let Ok(stamp) = jiff::Timestamp::from_second(moment.epoch_seconds()) else {
        return fl!("duration-now");
    };

    stamp
        .to_zoned(jiff::tz::TimeZone::system())
        .strftime(&fl!("clock-format"))
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minutes(count: u64) -> Duration {
        Duration::from_mins(count)
    }

    fn plain(rendered: &str) -> String {
        rendered.replace(['\u{2068}', '\u{2069}'], "")
    }

    #[test]
    fn only_a_countdown_that_has_run_out_reads_as_now() {
        assert_eq!(remaining(Duration::ZERO), fl!("duration-now"));

        for seconds in [1, 30, 59] {
            assert_eq!(plain(&remaining(Duration::from_secs(seconds))), "1 min");
        }
    }

    #[test]
    fn under_an_hour_is_counted_in_whole_minutes() {
        assert_eq!(plain(&remaining(minutes(1))), "1 min");
        assert_eq!(plain(&remaining(minutes(12))), "12 min");
        assert_eq!(plain(&remaining(minutes(59))), "59 min");
    }

    #[test]
    fn a_whole_number_of_hours_does_not_trail_a_pointless_zero() {
        assert_eq!(plain(&remaining(minutes(60))), "1 hr");
        assert_eq!(plain(&remaining(minutes(120))), "2 hr");
    }

    #[test]
    fn an_hour_and_a_bit_shows_both_halves() {
        assert_eq!(plain(&remaining(minutes(72))), "1 hr 12 min");
        assert_eq!(plain(&remaining(minutes(92))), "1 hr 32 min");
    }

    #[test]
    fn a_fresh_timer_keeps_the_minute_it_started_on() {
        assert_eq!(plain(&remaining(minutes(20))), "20 min");
        assert_eq!(
            plain(&remaining(Duration::from_secs(20 * 60 - 1))),
            "20 min"
        );
        assert_eq!(plain(&remaining(minutes(19))), "19 min");
    }

    #[test]
    fn an_interval_is_spelled_out_so_it_cannot_be_read_as_a_countdown() {
        assert_eq!(plain(&interval(minutes(20))), "Every 20 min");
        assert_eq!(plain(&interval(minutes(90))), "Every 90 min");
    }

    #[test]
    fn a_moment_is_rendered_as_a_time_of_day() {
        let rendered = clock(Moment::from_epoch_seconds(1_700_000_000));

        assert!(!rendered.is_empty());
        assert!(
            rendered.contains(':'),
            "a clock reads as a time: {rendered}"
        );
        assert!(!rendered.contains('%'), "the pattern leaked: {rendered}");
    }
}
