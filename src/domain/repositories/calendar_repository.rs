use crate::domain::models::Calendar;
use async_trait::async_trait;
use ikeda_core::error::AppError;
use uuid::Uuid;

/// Port defining persistence operations for `Calendar` entities.
#[async_trait]
pub trait CalendarRepository: Send + Sync {
    /// Persists a new calendar.
    async fn create(&self, tenant_id: Uuid, calendar: &Calendar) -> Result<Calendar, AppError>;

    /// Finds a calendar by its unique identifier within the tenant.
    async fn find_by_id(&self, tenant_id: Uuid, id: Uuid) -> Result<Option<Calendar>, AppError>;

    /// Lists all active calendars belonging to the tenant.
    async fn find_by_tenant(&self, tenant_id: Uuid) -> Result<Vec<Calendar>, AppError>;

    /// Updates calendar details.
    async fn update(&self, tenant_id: Uuid, calendar: &Calendar) -> Result<Calendar, AppError>;

    /// Soft-deletes a calendar by ID.
    async fn soft_delete(&self, tenant_id: Uuid, id: Uuid) -> Result<bool, AppError>;
}
