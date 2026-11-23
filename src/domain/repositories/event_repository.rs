use crate::domain::models::{Event, EventException, TimeRange};
use async_trait::async_trait;
use ikeda_core::criteria::Criteria;
use ikeda_core::error::AppError;
use uuid::Uuid;

/// Port defining persistence operations for `Event` and `EventException` entities.
#[async_trait]
pub trait EventRepository: Send + Sync {
    /// Persists a new event.
    async fn create(&self, tenant_id: Uuid, event: &Event) -> Result<Event, AppError>;

    /// Finds an event by its unique identifier within the tenant.
    async fn find_by_id(&self, tenant_id: Uuid, id: Uuid) -> Result<Option<Event>, AppError>;

    /// Finds events that overlap with a time range on a specific calendar.
    async fn find_in_range(
        &self,
        tenant_id: Uuid,
        calendar_id: Uuid,
        range: &TimeRange,
    ) -> Result<Vec<Event>, AppError>;

    /// Queries events matching dynamic criteria filters.
    async fn find_by_criteria(
        &self,
        tenant_id: Uuid,
        calendar_id: Uuid,
        criteria: &Criteria,
    ) -> Result<Vec<Event>, AppError>;

    /// Retrieves all exceptions registered for a recurring event.
    async fn find_exceptions(
        &self,
        tenant_id: Uuid,
        event_id: Uuid,
    ) -> Result<Vec<EventException>, AppError>;

    /// Adds an exception (cancellation or rescheduling) to a recurring event.
    async fn add_exception(
        &self,
        tenant_id: Uuid,
        exception: &EventException,
    ) -> Result<EventException, AppError>;

    /// Updates existing event details.
    async fn update(&self, tenant_id: Uuid, event: &Event) -> Result<Event, AppError>;

    /// Soft-deletes an event by ID.
    async fn soft_delete(&self, tenant_id: Uuid, id: Uuid) -> Result<bool, AppError>;
}
