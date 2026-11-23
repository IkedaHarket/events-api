use super::event::EventStatus;
use super::time_range::TimeRange;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Concrete occurrence of an event within a query time window.
///
/// For non-recurring events, this corresponds directly to the stored event.
/// For recurring events, each expanded repetition produces an `EventInstance`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct EventInstance {
    /// ID of the parent event.
    pub event_id: Uuid,
    /// Calendar containing this event.
    pub calendar_id: Uuid,
    /// Event title.
    pub title: String,
    /// Specific time range of this occurrence in UTC.
    pub time_range: TimeRange,
    /// Confirmation status.
    pub status: EventStatus,
    /// Whether this occurrence originates from a recurrence rule.
    pub is_recurring: bool,
    /// Whether this occurrence has been modified from the rule via an exception.
    pub is_exception: bool,
}

impl EventInstance {
    /// Creates a new event instance.
    pub fn new(
        event_id: Uuid,
        calendar_id: Uuid,
        title: impl Into<String>,
        time_range: TimeRange,
        status: EventStatus,
        is_recurring: bool,
        is_exception: bool,
    ) -> Self {
        Self {
            event_id,
            calendar_id,
            title: title.into(),
            time_range,
            status,
            is_recurring,
            is_exception,
        }
    }
}
