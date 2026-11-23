use crate::domain::models::TimeRange;
use chrono::Duration;
use ikeda_core::error::AppError;

/// Merges and consolidates overlapping or adjacent busy intervals.
#[must_use]
pub fn merge_busy_intervals(intervals: &[TimeRange]) -> Vec<TimeRange> {
    if intervals.is_empty() {
        return Vec::new();
    }

    let mut sorted = intervals.to_vec();
    sorted.sort_by_key(|r| r.starts_at);

    let mut merged: Vec<TimeRange> = Vec::with_capacity(sorted.len());

    for range in sorted {
        if let Some(last) = merged.last_mut() {
            // If ranges overlap or touch, extend the end
            if range.starts_at <= last.ends_at {
                last.ends_at = last.ends_at.max(range.ends_at);
                continue;
            }
        }
        merged.push(range);
    }

    merged
}

/// Computes discrete available (free) time slots of length `slot_duration`
/// within `query_range`, subtracting any busy intervals.
pub fn calculate_free_slots(
    query_range: &TimeRange,
    busy_intervals: &[TimeRange],
    slot_duration: Duration,
) -> Result<Vec<TimeRange>, AppError> {
    if slot_duration <= Duration::zero() {
        return Err(AppError::BadRequest(
            "Slot duration must be strictly positive".to_string(),
        ));
    }

    let merged_busy = merge_busy_intervals(busy_intervals);
    let mut free_slots = Vec::new();
    let mut current_cursor = query_range.starts_at;

    for busy in &merged_busy {
        // Clamp busy interval to query range
        let busy_start = busy.starts_at.max(query_range.starts_at);
        let busy_end = busy.ends_at.min(query_range.ends_at);

        if busy_start > current_cursor {
            // Free window between cursor and busy_start
            let mut window_start = current_cursor;
            while window_start + slot_duration <= busy_start {
                let slot = TimeRange::new(window_start, window_start + slot_duration)?;
                free_slots.push(slot);
                window_start += slot_duration;
            }
        }

        current_cursor = current_cursor.max(busy_end);
    }

    // Remaining window after the last busy block up to query_range.ends_at
    if current_cursor < query_range.ends_at {
        let mut window_start = current_cursor;
        while window_start + slot_duration <= query_range.ends_at {
            let slot = TimeRange::new(window_start, window_start + slot_duration)?;
            free_slots.push(slot);
            window_start += slot_duration;
        }
    }

    Ok(free_slots)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    #[test]
    fn test_merge_busy_intervals() {
        let now = Utc::now();
        let r1 = TimeRange::new(now, now + Duration::hours(2)).unwrap();
        let r2 = TimeRange::new(now + Duration::hours(1), now + Duration::hours(3)).unwrap();
        let r3 = TimeRange::new(now + Duration::hours(5), now + Duration::hours(6)).unwrap();

        let merged = merge_busy_intervals(&[r1, r2, r3]);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].starts_at, now);
        assert_eq!(merged[0].ends_at, now + Duration::hours(3));
        assert_eq!(merged[1].starts_at, now + Duration::hours(5));
        assert_eq!(merged[1].ends_at, now + Duration::hours(6));
    }

    #[test]
    fn test_calculate_free_slots() {
        let now = Utc::now();
        let query_range = TimeRange::new(now, now + Duration::hours(4)).unwrap();
        // Busy from hour 1 to hour 2
        let busy = TimeRange::new(now + Duration::hours(1), now + Duration::hours(2)).unwrap();

        let slots = calculate_free_slots(&query_range, &[busy], Duration::hours(1))
            .expect("calculation failed");

        // Should have hour 0..1, hour 2..3, hour 3..4 (3 slots of 1 hour)
        assert_eq!(slots.len(), 3);
        assert_eq!(slots[0].starts_at, now);
        assert_eq!(slots[0].ends_at, now + Duration::hours(1));
        assert_eq!(slots[1].starts_at, now + Duration::hours(2));
        assert_eq!(slots[2].starts_at, now + Duration::hours(3));
    }
}
