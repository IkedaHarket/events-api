use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Calendar entity representing a schedule or bookable resource within a tenant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Calendar {
    /// Unique calendar ID.
    pub id: Uuid,
    /// Tenant that owns this calendar.
    pub tenant_id: Uuid,
    /// Human-readable calendar title.
    pub name: String,
    /// Primary IANA timezone (e.g. "America/New_York", "UTC").
    pub timezone: String,
    /// Whether concurrent events are allowed on this calendar.
    pub allow_overlaps: bool,
    /// Optional structured working hours specification (JSONB).
    pub working_hours: Option<serde_json::Value>,
    /// Creation timestamp in UTC.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp in UTC.
    pub updated_at: DateTime<Utc>,
    /// Soft deletion timestamp.
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Calendar {
    /// Creates a new calendar instance.
    pub fn new(
        tenant_id: Uuid,
        name: impl Into<String>,
        timezone: impl Into<String>,
        allow_overlaps: bool,
        working_hours: Option<serde_json::Value>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            tenant_id,
            name: name.into(),
            timezone: timezone.into(),
            allow_overlaps,
            working_hours,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        }
    }

    /// Whether this calendar has been soft-deleted.
    #[must_use]
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    /// Marks the calendar as soft-deleted.
    pub fn mark_deleted(&mut self) {
        let now = Utc::now();
        self.deleted_at = Some(now);
        self.updated_at = now;
    }
}
