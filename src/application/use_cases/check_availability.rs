use crate::domain::models::{AvailabilityResult, EventStatus, TimeRange};
use crate::domain::repositories::{CalendarRepository, EventRepository};
use crate::domain::services::{calculate_free_slots, expand_recurrence, merge_busy_intervals};
use chrono::Duration;
use ikeda_core::error::AppError;
use std::sync::Arc;
use uuid::Uuid;

/// Query parameters for availability checks.
#[derive(Debug, Clone)]
pub struct CheckAvailabilityQuery {
    /// Calendar ID to evaluate.
    pub calendar_id: Uuid,
    /// Time window to check.
    pub range: TimeRange,
    /// Minimum slot length in minutes (defaults to 30 minutes).
    pub slot_duration_minutes: Option<i64>,
}

/// Use case for computing free/busy windows on a calendar.
pub struct CheckAvailabilityUseCase {
    calendar_repo: Arc<dyn CalendarRepository>,
    event_repo: Arc<dyn EventRepository>,
}

impl CheckAvailabilityUseCase {
    /// Injects repository dependencies.
    pub fn new(
        calendar_repo: Arc<dyn CalendarRepository>,
        event_repo: Arc<dyn EventRepository>,
    ) -> Self {
        Self {
            calendar_repo,
            event_repo,
        }
    }

    /// Computes free and busy blocks.
    pub async fn execute(
        &self,
        tenant_id: Uuid,
        query: CheckAvailabilityQuery,
    ) -> Result<AvailabilityResult, AppError> {
        // Verify calendar existence
        let _calendar = self
            .calendar_repo
            .find_by_id(tenant_id, query.calendar_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("Calendar '{}' not found", query.calendar_id))
            })?;

        let events = self
            .event_repo
            .find_in_range(tenant_id, query.calendar_id, &query.range)
            .await?;

        let mut busy_intervals = Vec::new();

        for event in &events {
            let exceptions = if event.is_recurring() {
                self.event_repo.find_exceptions(tenant_id, event.id).await?
            } else {
                Vec::new()
            };

            let instances = expand_recurrence(event, &query.range, &exceptions)?;
            for inst in instances {
                if inst.status != EventStatus::Cancelled {
                    busy_intervals.push(inst.time_range);
                }
            }
        }

        let merged_busy = merge_busy_intervals(&busy_intervals);

        let duration_minutes = query.slot_duration_minutes.unwrap_or(30).max(5);
        let slot_duration = Duration::minutes(duration_minutes);

        let available_slots = calculate_free_slots(&query.range, &merged_busy, slot_duration)?;

        Ok(AvailabilityResult::new(
            query.calendar_id,
            query.range,
            available_slots,
            merged_busy,
        ))
    }
}
