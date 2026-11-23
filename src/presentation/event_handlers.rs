use super::state::AppState;
use crate::application::use_cases::{
    CheckAvailabilityQuery, CheckAvailabilityUseCase, CreateEventCommand, CreateEventUseCase,
    FindEventsByRangeQuery, FindEventsByRangeUseCase,
};
use crate::domain::models::{AvailabilityResult, Event, EventInstance, EventStatus, TimeRange};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use ikeda_core::criteria::Criteria;
use ikeda_core::error::AppError;
use ikeda_core::response::ApiResponse;
use ikeda_core::tenant::TenantContext;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// Header name constant for request idempotency.
pub const HEADER_IDEMPOTENCY_KEY: &str = "idempotency-key";

/// Query parameters for fetching events within a date range.
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct EventQueryParams {
    /// ID of the calendar.
    pub calendar_id: Uuid,
    /// UTC search window start.
    pub starts_at: DateTime<Utc>,
    /// UTC search window end.
    pub ends_at: DateTime<Utc>,
}

/// Request payload for dynamic criteria searching on events.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EventSearchRequest {
    /// ID of the calendar.
    pub calendar_id: Uuid,
    /// UTC search window start.
    pub starts_at: DateTime<Utc>,
    /// UTC search window end.
    pub ends_at: DateTime<Utc>,
    /// Optional dynamic recursive criteria.
    pub criteria: Option<Criteria>,
}

/// Query parameters for calendar availability slot calculation.
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct AvailabilityQueryParams {
    /// Range start timestamp in UTC.
    pub starts_at: DateTime<Utc>,
    /// Range end timestamp in UTC.
    pub ends_at: DateTime<Utc>,
    /// Length of individual bookable slots in minutes (default: 30).
    pub slot_duration_minutes: Option<i64>,
}

/// Request payload for patching an existing event.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PatchEventPayload {
    /// Optional updated title.
    pub title: Option<String>,
    /// Optional updated description.
    pub description: Option<String>,
    /// Optional updated start timestamp.
    pub starts_at: Option<DateTime<Utc>>,
    /// Optional updated end timestamp.
    pub ends_at: Option<DateTime<Utc>>,
    /// Optional updated confirmation status.
    pub status: Option<EventStatus>,
    /// Optional updated metadata object.
    pub metadata: Option<serde_json::Value>,
}

/// Creates a new event with idempotency support and optional overlap prevention.
#[utoipa::path(
    post,
    path = "/v1/events",
    tag = "Events",
    request_body = CreateEventCommand,
    params(
        ("Idempotency-Key" = Option<String>, Header, description = "Unique client request key for idempotent retries")
    ),
    responses(
        (status = 201, description = "Event created successfully", body = ApiResponse<Event>),
        (status = 409, description = "Conflict: event overlaps on strict calendar", body = ApiResponse<serde_json::Value>),
        (status = 400, description = "Invalid event payload", body = ApiResponse<serde_json::Value>),
        (status = 401, description = "Unauthorized tenant context", body = ApiResponse<serde_json::Value>)
    )
)]
pub async fn create_event(
    State(state): State<AppState>,
    tenant: TenantContext,
    headers: HeaderMap,
    Json(mut cmd): Json<CreateEventCommand>,
) -> Result<impl IntoResponse, AppError> {
    // Idempotency: inspect Idempotency-Key header and bind to external_resource_id if unassigned
    if cmd.external_resource_id.is_none() {
        if let Some(idemp_val) = headers.get(HEADER_IDEMPOTENCY_KEY) {
            if let Ok(key_str) = idemp_val.to_str() {
                let trimmed = key_str.trim();
                if !trimmed.is_empty() {
                    cmd.external_resource_id = Some(trimmed.to_string());
                }
            }
        }
    }

    let use_case = CreateEventUseCase::new(state.calendar_repo.clone(), state.event_repo.clone());
    let event = use_case.execute(tenant.tenant_id, cmd).await?;

    let response = ApiResponse::created(event, "Event created successfully");
    Ok((StatusCode::CREATED, Json(response)))
}

/// Retrieves events expanded across a date range.
#[utoipa::path(
    get,
    path = "/v1/events",
    tag = "Events",
    params(
        EventQueryParams
    ),
    responses(
        (status = 200, description = "List of expanded event occurrences", body = ApiResponse<Vec<EventInstance>>),
        (status = 400, description = "Invalid time range", body = ApiResponse<serde_json::Value>),
        (status = 401, description = "Unauthorized tenant context", body = ApiResponse<serde_json::Value>)
    )
)]
pub async fn list_events_by_range(
    State(state): State<AppState>,
    tenant: TenantContext,
    Query(params): Query<EventQueryParams>,
) -> Result<impl IntoResponse, AppError> {
    let range = TimeRange::new(params.starts_at, params.ends_at)?;
    let query = FindEventsByRangeQuery {
        calendar_id: params.calendar_id,
        range,
        criteria: None,
    };

    let use_case = FindEventsByRangeUseCase::new(state.event_repo.clone());
    let instances = use_case.execute(tenant.tenant_id, query).await?;

    Ok(Json(ApiResponse::ok(instances)))
}

/// Searches and filters events using dynamic criteria expressions.
#[utoipa::path(
    post,
    path = "/v1/events/search",
    tag = "Events",
    request_body = EventSearchRequest,
    responses(
        (status = 200, description = "Filtered event occurrences", body = ApiResponse<Vec<EventInstance>>),
        (status = 400, description = "Invalid search criteria", body = ApiResponse<serde_json::Value>),
        (status = 401, description = "Unauthorized tenant context", body = ApiResponse<serde_json::Value>)
    )
)]
pub async fn search_events(
    State(state): State<AppState>,
    tenant: TenantContext,
    Json(req): Json<EventSearchRequest>,
) -> Result<impl IntoResponse, AppError> {
    let range = TimeRange::new(req.starts_at, req.ends_at)?;
    let query = FindEventsByRangeQuery {
        calendar_id: req.calendar_id,
        range,
        criteria: req.criteria,
    };

    let use_case = FindEventsByRangeUseCase::new(state.event_repo.clone());
    let instances = use_case.execute(tenant.tenant_id, query).await?;

    Ok(Json(ApiResponse::ok(instances)))
}

/// Retrieves a specific event by ID.
#[utoipa::path(
    get,
    path = "/v1/events/{id}",
    tag = "Events",
    params(
        ("id" = Uuid, Path, description = "Event unique identifier")
    ),
    responses(
        (status = 200, description = "Event found", body = ApiResponse<Event>),
        (status = 404, description = "Event not found", body = ApiResponse<serde_json::Value>),
        (status = 401, description = "Unauthorized tenant context", body = ApiResponse<serde_json::Value>)
    )
)]
pub async fn get_event(
    State(state): State<AppState>,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let event = state
        .event_repo
        .find_by_id(tenant.tenant_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Event with ID '{id}' not found")))?;

    Ok(Json(ApiResponse::ok(event)))
}

/// Updates details of an existing event.
#[utoipa::path(
    patch,
    path = "/v1/events/{id}",
    tag = "Events",
    params(
        ("id" = Uuid, Path, description = "Event unique identifier")
    ),
    request_body = PatchEventPayload,
    responses(
        (status = 200, description = "Event updated successfully", body = ApiResponse<Event>),
        (status = 404, description = "Event not found", body = ApiResponse<serde_json::Value>),
        (status = 400, description = "Invalid update payload", body = ApiResponse<serde_json::Value>),
        (status = 401, description = "Unauthorized tenant context", body = ApiResponse<serde_json::Value>)
    )
)]
pub async fn patch_event(
    State(state): State<AppState>,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<PatchEventPayload>,
) -> Result<impl IntoResponse, AppError> {
    let mut event = state
        .event_repo
        .find_by_id(tenant.tenant_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Event with ID '{id}' not found")))?;

    if let Some(title) = payload.title {
        let t = title.trim();
        if t.is_empty() {
            return Err(AppError::BadRequest(
                "Event title cannot be empty".to_string(),
            ));
        }
        event.title = t.to_string();
    }

    if let Some(desc) = payload.description {
        event.description = Some(desc);
    }

    let starts_at = payload.starts_at.unwrap_or(event.time_range.starts_at);
    let ends_at = payload.ends_at.unwrap_or(event.time_range.ends_at);
    event.time_range = TimeRange::new(starts_at, ends_at)?;

    if let Some(status) = payload.status {
        event.status = status;
    }

    if let Some(meta) = payload.metadata {
        event.metadata = meta;
    }

    let updated = state.event_repo.update(tenant.tenant_id, &event).await?;
    Ok(Json(ApiResponse::ok(updated)))
}

/// Soft-deletes an event by ID.
#[utoipa::path(
    delete,
    path = "/v1/events/{id}",
    tag = "Events",
    params(
        ("id" = Uuid, Path, description = "Event unique identifier")
    ),
    responses(
        (status = 200, description = "Event deleted successfully", body = ApiResponse<serde_json::Value>),
        (status = 404, description = "Event not found", body = ApiResponse<serde_json::Value>),
        (status = 401, description = "Unauthorized tenant context", body = ApiResponse<serde_json::Value>)
    )
)]
pub async fn delete_event(
    State(state): State<AppState>,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let deleted = state.event_repo.soft_delete(tenant.tenant_id, id).await?;
    if !deleted {
        return Err(AppError::NotFound(format!(
            "Event with ID '{id}' not found"
        )));
    }

    Ok(Json(ApiResponse::message("Event deleted successfully")))
}

/// Computes available booking slots for a calendar.
#[utoipa::path(
    get,
    path = "/v1/calendars/{id}/availability",
    tag = "Calendars",
    params(
        ("id" = Uuid, Path, description = "Calendar unique identifier"),
        AvailabilityQueryParams
    ),
    responses(
        (status = 200, description = "Calendar availability blocks", body = ApiResponse<AvailabilityResult>),
        (status = 404, description = "Calendar not found", body = ApiResponse<serde_json::Value>),
        (status = 400, description = "Invalid time range", body = ApiResponse<serde_json::Value>),
        (status = 401, description = "Unauthorized tenant context", body = ApiResponse<serde_json::Value>)
    )
)]
pub async fn get_calendar_availability(
    State(state): State<AppState>,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
    Query(params): Query<AvailabilityQueryParams>,
) -> Result<impl IntoResponse, AppError> {
    let range = TimeRange::new(params.starts_at, params.ends_at)?;
    let query = CheckAvailabilityQuery {
        calendar_id: id,
        range,
        slot_duration_minutes: params.slot_duration_minutes,
    };

    let use_case =
        CheckAvailabilityUseCase::new(state.calendar_repo.clone(), state.event_repo.clone());
    let availability = use_case.execute(tenant.tenant_id, query).await?;

    Ok(Json(ApiResponse::ok(availability)))
}
