pub mod calendar_handlers;
pub mod event_handlers;
pub mod routes;
pub mod state;

#[cfg(test)]
mod tests;

pub use routes::{create_router, ApiDoc, HealthStatus};
pub use state::AppState;
