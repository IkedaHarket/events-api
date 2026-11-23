use crate::domain::models::{EventInstance, EventStatus, TimeRange};

/// Checks if a candidate time range overlaps with any active (non-cancelled) event instance.
#[must_use]
pub fn detect_overlaps(instances: &[EventInstance], candidate: &TimeRange) -> bool {
    instances
        .iter()
        .any(|inst| inst.status != EventStatus::Cancelled && inst.time_range.overlaps(candidate))
}

/// Returns all active event instances that overlap with a candidate time range.
#[must_use]
pub fn find_overlapping(instances: &[EventInstance], candidate: &TimeRange) -> Vec<EventInstance> {
    instances
        .iter()
        .filter(|inst| inst.status != EventStatus::Cancelled && inst.time_range.overlaps(candidate))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use uuid::Uuid;

    #[test]
    fn test_detect_overlaps_finds_conflict() {
        let now = Utc::now();
        let r1 = TimeRange::new(now, now + Duration::hours(2)).unwrap();
        let r_candidate =
            TimeRange::new(now + Duration::hours(1), now + Duration::hours(3)).unwrap();

        let inst = EventInstance::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "Meeting",
            r1,
            EventStatus::Confirmed,
            false,
            false,
        );

        assert!(detect_overlaps(std::slice::from_ref(&inst), &r_candidate));
        assert_eq!(find_overlapping(&[inst], &r_candidate).len(), 1);
    }

    #[test]
    fn test_detect_overlaps_ignores_cancelled() {
        let now = Utc::now();
        let r1 = TimeRange::new(now, now + Duration::hours(2)).unwrap();
        let r_candidate =
            TimeRange::new(now + Duration::hours(1), now + Duration::hours(3)).unwrap();

        let inst = EventInstance::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "Cancelled Meeting",
            r1,
            EventStatus::Cancelled,
            false,
            false,
        );

        assert!(!detect_overlaps(&[inst], &r_candidate));
    }
}
