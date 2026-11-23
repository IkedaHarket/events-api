use super::state::AppState;
use crate::application::use_cases::{CreateCalendarCommand, CreateCalendarUseCase};
use crate::domain::models::Calendar;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use ikeda_core::error::AppError;
use ikeda_core::response::ApiResponse;
use ikeda_core::tenant::TenantContext;
use uuid::Uuid;

/// Creates a new calendar for the authenticated tenant.
#[utoipa::path(
    post,
    path = "/v1/calendars",
    tag = "Calendars",
    request_body = CreateCalendarCommand,
    responses(
        (status = 201, description = "Calendar created successfully", body = ApiResponse<Calendar>),
        (status = 400, description = "Invalid request payload", body = ApiResponse<serde_json::Value>),
        (status = 401, description = "Unauthorized tenant context", body = ApiResponse<serde_json::Value>)
    )
)]
pub async fn create_calendar(
    State(state): State<AppState>,
    tenant: TenantContext,
    Json(cmd): Json<CreateCalendarCommand>,
) -> Result<impl IntoResponse, AppError> {
    let use_case = CreateCalendarUseCase::new(state.calendar_repo.clone());
    let calendar = use_case.execute(tenant.tenant_id, cmd).await?;

    let response = ApiResponse::created(calendar, "Calendar created successfully");
    Ok((StatusCode::CREATED, Json(response)))
}

/// Lists all active calendars owned by the authenticated tenant.
#[utoipa::path(
    get,
    path = "/v1/calendars",
    tag = "Calendars",
    responses(
        (status = 200, description = "List of calendars", body = ApiResponse<Vec<Calendar>>),
        (status = 401, description = "Unauthorized tenant context", body = ApiResponse<serde_json::Value>)
    )
)]
pub async fn list_calendars(
    State(state): State<AppState>,
    tenant: TenantContext,
) -> Result<impl IntoResponse, AppError> {
    let calendars = state.calendar_repo.find_by_tenant(tenant.tenant_id).await?;

    Ok(Json(ApiResponse::ok(calendars)))
}

/// Retrieves a specific calendar by ID.
#[utoipa::path(
    get,
    path = "/v1/calendars/{id}",
    tag = "Calendars",
    params(
        ("id" = Uuid, Path, description = "Calendar unique identifier")
    ),
    responses(
        (status = 200, description = "Calendar found", body = ApiResponse<Calendar>),
        (status = 404, description = "Calendar not found", body = ApiResponse<serde_json::Value>),
        (status = 401, description = "Unauthorized tenant context", body = ApiResponse<serde_json::Value>)
    )
)]
pub async fn get_calendar(
    State(state): State<AppState>,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let calendar = state
        .calendar_repo
        .find_by_id(tenant.tenant_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Calendar with ID '{id}' not found")))?;

    Ok(Json(ApiResponse::ok(calendar)))
}
