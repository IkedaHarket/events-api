use crate::domain::models::{Event, EventStatus, TimeRange};
use crate::domain::repositories::{CalendarRepository, EventRepository};
use crate::domain::services::{detect_overlaps, expand_recurrence};
use chrono::{DateTime, Utc};
use ikeda_core::error::AppError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

/// Command parameters to create a new event.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateEventCommand {
    /// Target calendar ID.
    pub calendar_id: Uuid,
    /// Event title.
    pub title: String,
    /// Optional description.
    pub description: Option<String>,
    /// Start instant in UTC.
    pub starts_at: DateTime<Utc>,
    /// End instant in UTC.
    pub ends_at: DateTime<Utc>,
    /// IANA timezone (defaults to calendar's timezone if omitted).
    pub timezone: Option<String>,
    /// Optional RFC 5545 recurrence rule.
    pub rrule: Option<String>,
    /// Confirmation status (defaults to Confirmed).
    pub status: Option<EventStatus>,
    /// Optional external caller reference identifier.
    pub external_resource_id: Option<String>,
    /// Structured metadata payload.
    pub metadata: Option<serde_json::Value>,
}

/// Use case for scheduling a new event with optional strict overlap prevention.
pub struct CreateEventUseCase {
    calendar_repo: Arc<dyn CalendarRepository>,
    event_repo: Arc<dyn EventRepository>,
}

impl CreateEventUseCase {
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

    /// Executes event validation, conflict checking, and persistence.
    pub async fn execute(
        &self,
        tenant_id: Uuid,
        cmd: CreateEventCommand,
    ) -> Result<Event, AppError> {
        let title = cmd.title.trim();
        if title.is_empty() {
            return Err(AppError::BadRequest(
                "Event title cannot be empty".to_string(),
            ));
        }

        let time_range = TimeRange::new(cmd.starts_at, cmd.ends_at)?;

        // 1. Verify target calendar exists under the tenant
        let calendar = self
            .calendar_repo
            .find_by_id(tenant_id, cmd.calendar_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("Calendar '{}' not found", cmd.calendar_id))
            })?;

        // 2. If calendar enforces strict overlap prevention, verify no conflicts exist
        if !calendar.allow_overlaps {
            let existing_events = self
                .event_repo
                .find_in_range(tenant_id, calendar.id, &time_range)
                .await?;

            let mut all_instances = Vec::new();
            for existing in &existing_events {
                let exceptions = if existing.is_recurring() {
                    self.event_repo
                        .find_exceptions(tenant_id, existing.id)
                        .await?
                } else {
                    Vec::new()
                };

                let instances = expand_recurrence(existing, &time_range, &exceptions)?;
                all_instances.extend(instances);
            }

            if detect_overlaps(&all_instances, &time_range) {
                return Err(AppError::Conflict(
                    "Event time window conflicts with an existing reservation on this calendar"
                        .to_string(),
                ));
            }
        }

        let timezone = cmd.timezone.unwrap_or_else(|| calendar.timezone.clone());
        let status = cmd.status.unwrap_or(EventStatus::Confirmed);
        let metadata = cmd.metadata.unwrap_or_else(|| serde_json::json!({}));

        let event = Event::new(
            tenant_id,
            calendar.id,
            title,
            cmd.description,
            time_range,
            timezone,
            cmd.rrule,
            status,
            cmd.external_resource_id,
            metadata,
        );

        self.event_repo.create(tenant_id, &event).await
    }
}
