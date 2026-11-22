use std::ops::Add;
use std::time::Duration;

/// A reading of a monotonic clock. Readings compare only with readings of the
/// same clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Time(Duration);

impl Time {
    /// Returns the reading taken `elapsed` after the clock's origin.
    pub const fn from_elapsed(elapsed: Duration) -> Self {
        Self(elapsed)
    }

    /// Returns how long after `earlier` this reading is, or zero when `earlier`
    /// is not before it.
    pub fn saturating_since(self, earlier: Self) -> Duration {
        self.0.saturating_sub(earlier.0)
    }
}

/// Returns the earlier of `a` and `b`, ignoring a None. Returns None when both
/// are None.
pub(crate) fn earliest<T: Ord>(a: Option<T>, b: Option<T>) -> Option<T> {
    a.into_iter().chain(b).min()
}

impl Add<Duration> for Time {
    type Output = Self;

    fn add(self, span: Duration) -> Self {
        Self(self.0 + span)
    }
}

/// Times for the crate's tests.
#[cfg(test)]
pub(crate) mod testing {
    use std::time::Duration;

    use super::Time;

    /// The click window that the tests' times assume.
    pub(crate) const WINDOW: Duration = Duration::from_millis(300);

    /// Returns the reading taken `n` milliseconds after the clock's origin.
    pub(crate) const fn ms(n: u64) -> Time {
        Time::from_elapsed(Duration::from_millis(n))
    }
}

#[cfg(test)]
mod tests {
    use super::testing::ms;
    use super::*;

    #[test]
    fn spans_between_readings() {
        let start = ms(100);
        let later = start + Duration::from_millis(50);
        assert!(start < later);
        assert_eq!(later.saturating_since(start), Duration::from_millis(50));
        assert_eq!(start.saturating_since(later), Duration::ZERO);
    }
}
