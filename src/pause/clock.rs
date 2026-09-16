use std::time::Duration;

use crate::pause::model::Moment;

pub trait Clock: Send + Sync + 'static {
    fn wall(&self) -> Moment;
    fn uptime(&self) -> Duration;
}

pub struct SystemClock {
    started: std::time::Instant,
}

impl Default for SystemClock {
    fn default() -> Self {
        Self {
            started: std::time::Instant::now(),
        }
    }
}

impl Clock for SystemClock {
    fn wall(&self) -> Moment {
        let seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| {
                i64::try_from(since.as_secs()).unwrap_or(i64::MAX)
            });

        Moment::from_epoch_seconds(seconds)
    }

    fn uptime(&self) -> Duration {
        self.started.elapsed()
    }
}

#[derive(Debug, Default)]
pub struct GapDetector {
    last: Option<(Moment, Duration)>,
}

impl GapDetector {
    pub fn observe(&mut self, wall: Moment, uptime: Duration) -> Duration {
        let gap = match self.last {
            None => Duration::ZERO,
            Some((last_wall, last_uptime)) => {
                let by_wall = wall.saturating_duration_since(last_wall);
                let by_uptime = uptime.saturating_sub(last_uptime);
                by_wall.saturating_sub(by_uptime)
            }
        };

        self.last = Some((wall, uptime));
        gap
    }
}

pub mod manual {
    use std::sync::Mutex;

    use super::{Clock, Duration, Moment};

    #[derive(Debug)]
    pub struct TestClock {
        state: Mutex<(Moment, Duration)>,
    }

    impl TestClock {
        pub fn new(wall: Moment) -> Self {
            Self {
                state: Mutex::new((wall, Duration::ZERO)),
            }
        }

        pub fn tick(&self, span: Duration) {
            let mut state = self.state.lock().expect("the test clock is not poisoned");
            state.0 = state.0.saturating_add(span);
            state.1 += span;
        }

        pub fn sleep(&self, span: Duration) {
            let mut state = self.state.lock().expect("the test clock is not poisoned");
            state.0 = state.0.saturating_add(span);
        }
    }

    impl Clock for TestClock {
        fn wall(&self) -> Moment {
            self.state.lock().expect("the test clock is not poisoned").0
        }

        fn uptime(&self) -> Duration {
            self.state.lock().expect("the test clock is not poisoned").1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::manual::TestClock;
    use super::*;

    fn base() -> Moment {
        Moment::from_epoch_seconds(1_700_000_000)
    }

    #[test]
    fn the_first_observation_can_never_look_like_a_gap() {
        let mut detector = GapDetector::default();

        assert_eq!(detector.observe(base(), Duration::ZERO), Duration::ZERO);
    }

    #[test]
    fn a_steady_tick_reports_no_gap_at_all() {
        let clock = TestClock::new(base());
        let mut detector = GapDetector::default();
        detector.observe(clock.wall(), clock.uptime());

        for _ in 0..10 {
            clock.tick(Duration::from_secs(20));
            assert_eq!(
                detector.observe(clock.wall(), clock.uptime()),
                Duration::ZERO
            );
        }
    }

    #[test]
    fn time_that_passed_while_the_monotonic_clock_stood_still_is_reported_as_the_gap() {
        let clock = TestClock::new(base());
        let mut detector = GapDetector::default();
        detector.observe(clock.wall(), clock.uptime());

        clock.sleep(Duration::from_hours(2));
        clock.tick(Duration::from_secs(20));

        assert_eq!(
            detector.observe(clock.wall(), clock.uptime()),
            Duration::from_hours(2)
        );
    }

    #[test]
    fn a_wall_clock_that_steps_backwards_is_not_mistaken_for_a_gap() {
        let mut detector = GapDetector::default();
        detector.observe(base(), Duration::ZERO);

        let earlier = Moment::from_epoch_seconds(base().epoch_seconds() - 600);
        assert_eq!(
            detector.observe(earlier, Duration::from_secs(20)),
            Duration::ZERO
        );
    }
}
