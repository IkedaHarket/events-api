use crate::domain::models::{EventInstance, TimeRange};
use crate::domain::repositories::EventRepository;
use crate::domain::services::expand_recurrence;
use ikeda_core::criteria::Criteria;
use ikeda_core::error::AppError;
use std::sync::Arc;
use uuid::Uuid;

/// Query parameters for fetching and expanding events in a date window.
#[derive(Debug, Clone)]
pub struct FindEventsByRangeQuery {
    /// Target calendar ID.
    pub calendar_id: Uuid,
    /// Time window to query and expand occurrences in.
    pub range: TimeRange,
    /// Optional dynamic criteria filter.
    pub criteria: Option<Criteria>,
}

/// Use case that expands both single and recurring events into discrete instances.
pub struct FindEventsByRangeUseCase {
    event_repo: Arc<dyn EventRepository>,
}

impl FindEventsByRangeUseCase {
    /// Injects repository dependencies.
    pub fn new(event_repo: Arc<dyn EventRepository>) -> Self {
        Self { event_repo }
    }

    /// Executes query and recurrence expansion.
    pub async fn execute(
        &self,
        tenant_id: Uuid,
        query: FindEventsByRangeQuery,
    ) -> Result<Vec<EventInstance>, AppError> {
        let events = if let Some(criteria) = &query.criteria {
            self.event_repo
                .find_by_criteria(tenant_id, query.calendar_id, criteria)
                .await?
        } else {
            self.event_repo
                .find_in_range(tenant_id, query.calendar_id, &query.range)
                .await?
        };

        let mut instances = Vec::new();

        for event in &events {
            let exceptions = if event.is_recurring() {
                self.event_repo.find_exceptions(tenant_id, event.id).await?
            } else {
                Vec::new()
            };

            let expanded = expand_recurrence(event, &query.range, &exceptions)?;
            instances.extend(expanded);
        }

        // Sort instances by start time ascending
        instances.sort_by_key(|inst| inst.time_range.starts_at);

        Ok(instances)
    }
}
