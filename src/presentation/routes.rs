use super::calendar_handlers::{self, *};
use super::event_handlers::{self, *};
use super::state::AppState;
use crate::application::use_cases::{CreateCalendarCommand, CreateEventCommand};
use crate::domain::models::{
    AvailabilityResult, Calendar, Event, EventInstance, EventStatus, TimeRange,
};
use axum::{
    routing::{get, post},
    Router,
};
use ikeda_core::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

/// Health status response payload.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HealthStatus {
    /// Service running state.
    pub status: String,
    /// Service identifier.
    pub service: String,
    /// Semantic version of the service.
    pub version: String,
}

/// OpenAPI documentation schema definition for Events API.
#[derive(OpenApi)]
#[openapi(
    paths(
        health_check,
        calendar_handlers::create_calendar,
        calendar_handlers::list_calendars,
        calendar_handlers::get_calendar,
        event_handlers::create_event,
        event_handlers::list_events_by_range,
        event_handlers::search_events,
        event_handlers::get_event,
        event_handlers::patch_event,
        event_handlers::delete_event,
        event_handlers::get_calendar_availability
    ),
    components(
        schemas(
            HealthStatus,
            ApiResponse<HealthStatus>,
            ApiResponse<Calendar>,
            ApiResponse<Vec<Calendar>>,
            ApiResponse<Event>,
            ApiResponse<Vec<EventInstance>>,
            ApiResponse<AvailabilityResult>,
            Calendar,
            CreateCalendarCommand,
            Event,
            CreateEventCommand,
            PatchEventPayload,
            EventInstance,
            EventStatus,
            TimeRange,
            AvailabilityResult,
            EventQueryParams,
            EventSearchRequest,
            AvailabilityQueryParams,
            Criteria,
            FilterCondition,
            FilterGroup,
            FilterRule,
            FilterOperator,
            OrderBy,
            OrderDirection,
            Pagination,
            PaginationMeta
        )
    ),
    tags(
        (name = "Health", description = "Service lifecycle and readiness endpoints"),
        (name = "Calendars", description = "Tenant calendar management and availability calculation"),
        (name = "Events", description = "Multi-tenant event scheduling, recurrence expansion and criteria search")
    ),
    info(
        title = "IkedaDev Events API",
        version = "0.1.0",
        description = "Multi-tenant Calendar and Event management service with RFC 5545 recurrence and dynamic criteria filtering."
    )
)]
pub struct ApiDoc;

/// Health check endpoint.
#[utoipa::path(
    get,
    path = "/health",
    tag = "Health",
    responses(
        (status = 200, description = "Service is operational", body = ApiResponse<HealthStatus>)
    )
)]
pub async fn health_check() -> ApiResponse<HealthStatus> {
    ApiResponse::ok(HealthStatus {
        status: "healthy".to_string(),
        service: "events-api".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// Builds and configures the presentation router with injected application state.
pub fn create_router(state: AppState) -> Router {
    Router::new()
        // Health
        .route("/health", get(health_check))
        // Calendars
        .route("/v1/calendars", post(create_calendar).get(list_calendars))
        .route("/v1/calendars/{id}", get(get_calendar))
        .route(
            "/v1/calendars/{id}/availability",
            get(get_calendar_availability),
        )
        // Events
        .route("/v1/events", post(create_event).get(list_events_by_range))
        .route("/v1/events/search", post(search_events))
        .route(
            "/v1/events/{id}",
            get(get_event).patch(patch_event).delete(delete_event),
        )
        // Swagger UI
        .merge(SwaggerUi::new("/swagger-ui").url("/openapi.json", ApiDoc::openapi()))
        .with_state(state)
}
