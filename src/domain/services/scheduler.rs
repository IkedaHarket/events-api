use async_trait::async_trait;
use chrono::{DateTime, Utc};
use ikeda_core::error::AppError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Payload dispatched to subscriber webhooks when an event trigger or reminder fires.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WebhookEventPayload {
    /// ID of the scheduled event.
    pub event_id: Uuid,
    /// Tenant identifier.
    pub tenant_id: Uuid,
    /// Calendar containing the event.
    pub calendar_id: Uuid,
    /// Notification event type (e.g. "event.reminder", "event.created").
    pub event_type: String,
    /// Event scheduled start in UTC.
    pub starts_at: DateTime<Utc>,
    /// Event scheduled end in UTC.
    pub ends_at: DateTime<Utc>,
    /// Event title.
    pub title: String,
    /// Custom metadata payload.
    pub metadata: serde_json::Value,
}

/// Port defining message scheduling capabilities to deliver delayed webhooks.
#[async_trait]
pub trait NotificationScheduler: Send + Sync {
    /// Schedules a delayed HTTP webhook invocation to `destination_url` at `scheduled_for`.
    async fn schedule_reminder(
        &self,
        tenant_id: Uuid,
        destination_url: &str,
        scheduled_for: DateTime<Utc>,
        payload: WebhookEventPayload,
    ) -> Result<String, AppError>;
}
