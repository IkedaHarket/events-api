use super::time_range::TimeRange;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Lifecycle status of an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventStatus {
    /// Active, confirmed event reservation.
    #[default]
    Confirmed,
    /// Provisional or pending event.
    Tentative,
    /// Explicitly cancelled event.
    Cancelled,
}

/// Event entity representing a scheduled single or recurring calendar item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Event {
    /// Unique event ID.
    pub id: Uuid,
    /// Tenant that owns this event.
    pub tenant_id: Uuid,
    /// Calendar to which this event belongs.
    pub calendar_id: Uuid,
    /// Event title.
    pub title: String,
    /// Optional description or notes.
    pub description: Option<String>,
    /// Scheduled start and end time window in UTC.
    pub time_range: TimeRange,
    /// Primary IANA timezone of the event.
    pub timezone: String,
    /// Optional RFC 5545 recurrence rule (e.g. "FREQ=WEEKLY;BYDAY=MO,WE,FR").
    pub rrule: Option<String>,
    /// Event confirmation status.
    pub status: EventStatus,
    /// Identifier linking this event to an external caller's resource.
    pub external_resource_id: Option<String>,
    /// Flexible structured metadata (JSONB).
    pub metadata: serde_json::Value,
    /// Creation timestamp in UTC.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp in UTC.
    pub updated_at: DateTime<Utc>,
    /// Soft deletion timestamp.
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Event {
    /// Constructs a new Event instance.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tenant_id: Uuid,
        calendar_id: Uuid,
        title: impl Into<String>,
        description: Option<String>,
        time_range: TimeRange,
        timezone: impl Into<String>,
        rrule: Option<String>,
        status: EventStatus,
        external_resource_id: Option<String>,
        metadata: serde_json::Value,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            tenant_id,
            calendar_id,
            title: title.into(),
            description,
            time_range,
            timezone: timezone.into(),
            rrule,
            status,
            external_resource_id,
            metadata,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        }
    }

    /// Whether this event defines a recurring schedule.
    #[must_use]
    pub fn is_recurring(&self) -> bool {
        self.rrule.as_deref().is_some_and(|r| !r.trim().is_empty())
    }

    /// Whether this event has been soft-deleted.
    #[must_use]
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    /// Marks the event as soft-deleted.
    pub fn mark_deleted(&mut self) {
        let now = Utc::now();
        self.deleted_at = Some(now);
        self.updated_at = now;
    }
}
