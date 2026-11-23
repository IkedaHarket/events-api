use super::time_range::TimeRange;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Exception overriding or cancelling a specific instance of a recurring event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct EventException {
    /// Exception ID.
    pub id: Uuid,
    /// Tenant owner.
    pub tenant_id: Uuid,
    /// Parent recurring event ID.
    pub event_id: Uuid,
    /// The original occurrence start timestamp being modified or cancelled.
    pub original_starts_at: DateTime<Utc>,
    /// Whether this occurrence is cancelled entirely.
    pub is_cancelled: bool,
    /// New rescheduled time window if modified rather than cancelled.
    pub rescheduled_time_range: Option<TimeRange>,
    /// Creation timestamp in UTC.
    pub created_at: DateTime<Utc>,
}

impl EventException {
    /// Creates a cancellation exception for an occurrence.
    pub fn cancel(tenant_id: Uuid, event_id: Uuid, original_starts_at: DateTime<Utc>) -> Self {
        Self {
            id: Uuid::new_v4(),
            tenant_id,
            event_id,
            original_starts_at,
            is_cancelled: true,
            rescheduled_time_range: None,
            created_at: Utc::now(),
        }
    }

    /// Creates a rescheduling exception for an occurrence.
    pub fn reschedule(
        tenant_id: Uuid,
        event_id: Uuid,
        original_starts_at: DateTime<Utc>,
        rescheduled_time_range: TimeRange,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            tenant_id,
            event_id,
            original_starts_at,
            is_cancelled: false,
            rescheduled_time_range: Some(rescheduled_time_range),
            created_at: Utc::now(),
        }
    }
}
