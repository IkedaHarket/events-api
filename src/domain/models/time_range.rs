use chrono::{DateTime, Duration, Utc};
use ikeda_core::error::AppError;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Represents a validated, continuous time window with a UTC start and end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TimeRange {
    /// UTC start timestamp (inclusive).
    pub starts_at: DateTime<Utc>,
    /// UTC end timestamp (exclusive).
    pub ends_at: DateTime<Utc>,
}

impl TimeRange {
    /// Constructs a validated `TimeRange`, ensuring `ends_at >= starts_at`.
    pub fn new(starts_at: DateTime<Utc>, ends_at: DateTime<Utc>) -> Result<Self, AppError> {
        if ends_at < starts_at {
            return Err(AppError::BadRequest(
                "TimeRange end timestamp cannot precede start timestamp".to_string(),
            ));
        }

        Ok(Self { starts_at, ends_at })
    }

    /// Determines whether this time range overlaps with another.
    ///
    /// Ranges [A, B) and [C, D) overlap if and only if A < D and B > C.
    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        self.starts_at < other.ends_at && self.ends_at > other.starts_at
    }

    /// Determines whether a specific instant falls within this range [starts_at, ends_at).
    #[must_use]
    pub fn contains_instant(&self, instant: &DateTime<Utc>) -> bool {
        *instant >= self.starts_at && *instant < self.ends_at
    }

    /// Returns the duration of the time range.
    #[must_use]
    pub fn duration(&self) -> Duration {
        self.ends_at - self.starts_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_time_range() {
        let t1 = Utc::now();
        let t2 = t1 + Duration::hours(1);
        let range = TimeRange::new(t1, t2);
        assert!(range.is_ok());
    }

    #[test]
    fn test_invalid_time_range_fails() {
        let t1 = Utc::now();
        let t2 = t1 - Duration::hours(1);
        let range = TimeRange::new(t1, t2);
        assert!(range.is_err());
    }

    #[test]
    fn test_time_range_overlaps() {
        let now = Utc::now();
        let r1 = TimeRange::new(now, now + Duration::hours(2)).expect("valid range");
        let r2 = TimeRange::new(now + Duration::hours(1), now + Duration::hours(3))
            .expect("valid range");
        let r3 = TimeRange::new(now + Duration::hours(2), now + Duration::hours(4))
            .expect("valid range"); // Adjacent, not overlapping

        assert!(r1.overlaps(&r2));
        assert!(r2.overlaps(&r1));
        assert!(!r1.overlaps(&r3)); // Adjacent boundaries do not overlap
    }
}
