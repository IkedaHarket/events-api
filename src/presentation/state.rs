use crate::domain::repositories::{CalendarRepository, EventRepository};
use std::sync::Arc;

/// Shared application state containing injectable repository ports.
#[derive(Clone)]
pub struct AppState {
    /// Injected calendar repository port.
    pub calendar_repo: Arc<dyn CalendarRepository>,
    /// Injected event repository port.
    pub event_repo: Arc<dyn EventRepository>,
}

impl AppState {
    /// Creates a new AppState instance with injected dependencies.
    pub fn new(
        calendar_repo: Arc<dyn CalendarRepository>,
        event_repo: Arc<dyn EventRepository>,
    ) -> Self {
        Self {
            calendar_repo,
            event_repo,
        }
    }
}
