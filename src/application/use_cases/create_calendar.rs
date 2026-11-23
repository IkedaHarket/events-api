use crate::domain::models::Calendar;
use crate::domain::repositories::CalendarRepository;
use ikeda_core::error::AppError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

/// Command parameters to create a new calendar.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateCalendarCommand {
    /// Human-readable calendar name.
    pub name: String,
    /// IANA timezone identifier (e.g. "America/New_York", defaults to "UTC").
    pub timezone: Option<String>,
    /// Whether concurrent events are allowed (defaults to true).
    pub allow_overlaps: Option<bool>,
    /// Optional working hours configuration (JSON).
    pub working_hours: Option<serde_json::Value>,
}

/// Use case for creating and registering a new calendar under a tenant.
pub struct CreateCalendarUseCase {
    calendar_repo: Arc<dyn CalendarRepository>,
}

impl CreateCalendarUseCase {
    /// Injects required repository dependencies.
    pub fn new(calendar_repo: Arc<dyn CalendarRepository>) -> Self {
        Self { calendar_repo }
    }

    /// Executes calendar creation.
    pub async fn execute(
        &self,
        tenant_id: Uuid,
        cmd: CreateCalendarCommand,
    ) -> Result<Calendar, AppError> {
        let name = cmd.name.trim();
        if name.is_empty() {
            return Err(AppError::BadRequest(
                "Calendar name cannot be empty".to_string(),
            ));
        }

        let timezone = cmd.timezone.as_deref().unwrap_or("UTC");
        // Validate timezone format
        if timezone.parse::<chrono_tz::Tz>().is_err() && timezone != "UTC" {
            return Err(AppError::BadRequest(format!(
                "Invalid IANA timezone identifier: '{timezone}'"
            )));
        }

        let allow_overlaps = cmd.allow_overlaps.unwrap_or(true);

        let calendar = Calendar::new(tenant_id, name, timezone, allow_overlaps, cmd.working_hours);

        self.calendar_repo.create(tenant_id, &calendar).await
    }
}
