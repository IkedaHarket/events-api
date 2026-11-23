use super::time_range::TimeRange;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Availability query result detailing free and busy time intervals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AvailabilityResult {
    /// Target calendar ID.
    pub calendar_id: Uuid,
    /// The searched time window.
    pub query_range: TimeRange,
    /// Unoccupied, bookable time slots.
    pub available_slots: Vec<TimeRange>,
    /// Intervals occupied by existing events.
    pub busy_slots: Vec<TimeRange>,
}

impl AvailabilityResult {
    /// Creates a new availability result.
    pub fn new(
        calendar_id: Uuid,
        query_range: TimeRange,
        available_slots: Vec<TimeRange>,
        busy_slots: Vec<TimeRange>,
    ) -> Self {
        Self {
            calendar_id,
            query_range,
            available_slots,
            busy_slots,
        }
    }
}
