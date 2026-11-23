use crate::domain::models::{Event, EventException, EventInstance, EventStatus, TimeRange};
use chrono::{DateTime, Utc};
use ikeda_core::error::AppError;
use rrule::RRuleSet;

/// Expands an event (either single or recurring RFC 5545) into concrete `EventInstance`s
/// within the specified `query_range`, applying any registered exceptions.
pub fn expand_recurrence(
    event: &Event,
    query_range: &TimeRange,
    exceptions: &[EventException],
) -> Result<Vec<EventInstance>, AppError> {
    // 1. If not recurring, check if base event overlaps with query range
    if !event.is_recurring() {
        if event.time_range.overlaps(query_range) && event.status != EventStatus::Cancelled {
            return Ok(vec![EventInstance::new(
                event.id,
                event.calendar_id,
                &event.title,
                event.time_range,
                event.status,
                false,
                false,
            )]);
        }
        return Ok(Vec::new());
    }

    // 2. Parse RFC 5545 RRULE
    let rrule_str = event
        .rrule
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("Event has no recurrence rule defined".to_string()))?;

    let event_duration = event.time_range.duration();

    // Format full RFC 5545 string with DTSTART if not already present
    let formatted_dtstart = event
        .time_range
        .starts_at
        .format("%Y%m%dT%H%M%SZ")
        .to_string();

    let rrule_input = if rrule_str.to_uppercase().contains("DTSTART") {
        rrule_str.to_string()
    } else {
        let clean_rule = if let Some(stripped) = rrule_str.strip_prefix("RRULE:") {
            stripped
        } else {
            rrule_str
        };
        format!("DTSTART:{formatted_dtstart}\nRRULE:{clean_rule}")
    };

    let rrule_set: RRuleSet = rrule_input.parse().map_err(|e| {
        AppError::BadRequest(format!("Failed to parse RFC 5545 recurrence rule: {e}"))
    })?;

    // Build map of exceptions by original_starts_at for quick lookup
    let mut exception_map: std::collections::HashMap<DateTime<Utc>, &EventException> =
        std::collections::HashMap::new();
    for ex in exceptions {
        exception_map.insert(ex.original_starts_at, ex);
    }

    // Expand recurrences using rrule iterator
    let occurrences = rrule_set.all(500); // Safety limit against infinite recurrence
    let mut instances = Vec::new();

    for occ in occurrences.dates {
        let occ_utc: DateTime<Utc> = occ.with_timezone(&Utc);

        // Check if there is an exception for this occurrence
        if let Some(exception) = exception_map.get(&occ_utc) {
            if exception.is_cancelled {
                continue; // Cancelled instance: omit from results
            }

            if let Some(rescheduled) = exception.rescheduled_time_range {
                if rescheduled.overlaps(query_range) {
                    instances.push(EventInstance::new(
                        event.id,
                        event.calendar_id,
                        &event.title,
                        rescheduled,
                        event.status,
                        true,
                        true,
                    ));
                }
                continue;
            }
        }

        let occ_end = occ_utc + event_duration;
        let instance_range = TimeRange::new(occ_utc, occ_end)?;

        if instance_range.overlaps(query_range) {
            instances.push(EventInstance::new(
                event.id,
                event.calendar_id,
                &event.title,
                instance_range,
                event.status,
                true,
                false,
            ));
        }
    }

    Ok(instances)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::TimeRange;
    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    #[test]
    fn test_expand_weekly_recurrence() {
        let tenant_id = Uuid::new_v4();
        let cal_id = Uuid::new_v4();

        let dt_start = Utc.with_ymd_and_hms(2026, 11, 1, 10, 0, 0).unwrap();
        let dt_end = Utc.with_ymd_and_hms(2026, 11, 1, 11, 0, 0).unwrap();
        let time_range = TimeRange::new(dt_start, dt_end).unwrap();

        let event = Event::new(
            tenant_id,
            cal_id,
            "Weekly Sync",
            None,
            time_range,
            "UTC",
            Some("FREQ=WEEKLY;INTERVAL=1;COUNT=4".into()),
            EventStatus::Confirmed,
            None,
            serde_json::json!({}),
        );

        let query_range = TimeRange::new(
            Utc.with_ymd_and_hms(2026, 11, 1, 0, 0, 0).unwrap(),
            Utc.with_ymd_and_hms(2026, 11, 30, 23, 59, 59).unwrap(),
        )
        .unwrap();

        let instances = expand_recurrence(&event, &query_range, &[]).expect("expansion failed");
        assert_eq!(instances.len(), 4);
    }

    #[test]
    fn test_expand_with_cancellation_exception() {
        let tenant_id = Uuid::new_v4();
        let cal_id = Uuid::new_v4();

        let dt_start = Utc.with_ymd_and_hms(2026, 11, 1, 10, 0, 0).unwrap();
        let dt_end = Utc.with_ymd_and_hms(2026, 11, 1, 11, 0, 0).unwrap();
        let time_range = TimeRange::new(dt_start, dt_end).unwrap();

        let event = Event::new(
            tenant_id,
            cal_id,
            "Weekly Standup",
            None,
            time_range,
            "UTC",
            Some("FREQ=WEEKLY;INTERVAL=1;COUNT=3".into()),
            EventStatus::Confirmed,
            None,
            serde_json::json!({}),
        );

        // Cancel the 2nd instance on 2026-11-08
        let cancelled_dt = Utc.with_ymd_and_hms(2026, 11, 8, 10, 0, 0).unwrap();
        let exception = EventException::cancel(tenant_id, event.id, cancelled_dt);

        let query_range = TimeRange::new(
            Utc.with_ymd_and_hms(2026, 11, 1, 0, 0, 0).unwrap(),
            Utc.with_ymd_and_hms(2026, 11, 30, 0, 0, 0).unwrap(),
        )
        .unwrap();

        let instances =
            expand_recurrence(&event, &query_range, &[exception]).expect("expansion failed");
        assert_eq!(instances.len(), 2);
    }
}
