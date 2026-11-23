use super::routes::{create_router, ApiDoc};
use super::state::AppState;
use crate::application::tests::MockCalendarRepository;
use crate::application::use_cases::{CreateCalendarCommand, CreateEventCommand};
use crate::domain::models::{Calendar, Event, EventException, TimeRange};
use crate::domain::repositories::{CalendarRepository, EventRepository};
use async_trait::async_trait;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use chrono::{Duration, Utc};
use ikeda_core::criteria::Criteria;
use ikeda_core::error::AppError;
use ikeda_core::tenant::HEADER_TENANT_ID;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use utoipa::OpenApi;
use uuid::Uuid;

#[derive(Default)]
struct PresentationMockEventRepo {
    events: Mutex<Vec<Event>>,
}

#[async_trait]
impl EventRepository for PresentationMockEventRepo {
    async fn create(&self, _tenant_id: Uuid, event: &Event) -> Result<Event, AppError> {
        let mut lock = self.events.lock().unwrap();
        // Check idempotency via external_resource_id
        if let Some(ext) = &event.external_resource_id {
            if let Some(existing) = lock
                .iter()
                .find(|e| e.external_resource_id.as_deref() == Some(ext))
            {
                return Ok(existing.clone());
            }
        }
        lock.push(event.clone());
        Ok(event.clone())
    }

    async fn find_by_id(&self, tenant_id: Uuid, id: Uuid) -> Result<Option<Event>, AppError> {
        let lock = self.events.lock().unwrap();
        Ok(lock
            .iter()
            .find(|e| e.tenant_id == tenant_id && e.id == id && !e.is_deleted())
            .cloned())
    }

    async fn find_in_range(
        &self,
        tenant_id: Uuid,
        calendar_id: Uuid,
        range: &TimeRange,
    ) -> Result<Vec<Event>, AppError> {
        let lock = self.events.lock().unwrap();
        Ok(lock
            .iter()
            .filter(|e| {
                e.tenant_id == tenant_id
                    && e.calendar_id == calendar_id
                    && !e.is_deleted()
                    && (e.time_range.overlaps(range) || e.is_recurring())
            })
            .cloned()
            .collect())
    }

    async fn find_by_criteria(
        &self,
        tenant_id: Uuid,
        calendar_id: Uuid,
        _criteria: &Criteria,
    ) -> Result<Vec<Event>, AppError> {
        let lock = self.events.lock().unwrap();
        Ok(lock
            .iter()
            .filter(|e| e.tenant_id == tenant_id && e.calendar_id == calendar_id && !e.is_deleted())
            .cloned()
            .collect())
    }

    async fn find_exceptions(
        &self,
        _tenant_id: Uuid,
        _event_id: Uuid,
    ) -> Result<Vec<EventException>, AppError> {
        Ok(Vec::new())
    }

    async fn add_exception(
        &self,
        _tenant_id: Uuid,
        exception: &EventException,
    ) -> Result<EventException, AppError> {
        Ok(exception.clone())
    }

    async fn update(&self, _tenant_id: Uuid, event: &Event) -> Result<Event, AppError> {
        let mut lock = self.events.lock().unwrap();
        if let Some(pos) = lock.iter().position(|e| e.id == event.id) {
            lock[pos] = event.clone();
            Ok(event.clone())
        } else {
            Err(AppError::NotFound("Event not found".into()))
        }
    }

    async fn soft_delete(&self, tenant_id: Uuid, id: Uuid) -> Result<bool, AppError> {
        let mut lock = self.events.lock().unwrap();
        if let Some(e) = lock
            .iter_mut()
            .find(|e| e.tenant_id == tenant_id && e.id == id)
        {
            e.mark_deleted();
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

fn create_test_state() -> AppState {
    AppState::new(
        Arc::new(MockCalendarRepository::default()),
        Arc::new(PresentationMockEventRepo::default()),
    )
}

#[tokio::test]
async fn test_health_check_endpoint() {
    let app = create_router(create_test_state());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("failed to execute request");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_openapi_json_endpoint() {
    let app = create_router(create_test_state());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("failed to execute request");

    assert_eq!(response.status(), StatusCode::OK);
}

#[test]
fn test_openapi_spec_generation() {
    let openapi = ApiDoc::openapi();
    assert_eq!(openapi.info.title, "IkedaDev Events API");
    assert_eq!(openapi.info.version, "0.1.0");
    assert!(openapi.paths.paths.contains_key("/health"));
    assert!(openapi.paths.paths.contains_key("/v1/calendars"));
    assert!(openapi.paths.paths.contains_key("/v1/events"));
    assert!(openapi.paths.paths.contains_key("/v1/events/search"));
    assert!(openapi
        .paths
        .paths
        .contains_key("/v1/calendars/{id}/availability"));
}

#[tokio::test]
async fn test_create_and_get_calendar_via_http() {
    let state = create_test_state();
    let app = create_router(state);
    let tenant_id = Uuid::new_v4();

    let cmd = CreateCalendarCommand {
        name: "Meeting Room A".to_string(),
        timezone: Some("UTC".to_string()),
        allow_overlaps: Some(true),
        working_hours: None,
    };
    let payload = serde_json::to_string(&cmd).unwrap();

    let req = Request::builder()
        .method("POST")
        .uri("/v1/calendars")
        .header(header::CONTENT_TYPE, "application/json")
        .header(HEADER_TENANT_ID, tenant_id.to_string())
        .body(Body::from(payload))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_create_event_with_idempotency_key_via_http() {
    let state = create_test_state();
    let tenant_id = Uuid::new_v4();

    // Pre-create calendar
    let cal = Calendar::new(tenant_id, "Resource 1", "UTC", true, None);
    state.calendar_repo.create(tenant_id, &cal).await.unwrap();

    let app = create_router(state);

    let now = Utc::now();
    let cmd = CreateEventCommand {
        calendar_id: cal.id,
        title: "Team Retrospective".to_string(),
        description: None,
        starts_at: now + Duration::hours(1),
        ends_at: now + Duration::hours(2),
        timezone: None,
        rrule: None,
        status: None,
        external_resource_id: None,
        metadata: None,
    };
    let payload = serde_json::to_string(&cmd).unwrap();

    let req = Request::builder()
        .method("POST")
        .uri("/v1/events")
        .header(header::CONTENT_TYPE, "application/json")
        .header(HEADER_TENANT_ID, tenant_id.to_string())
        .header("idempotency-key", "req-unique-key-1234")
        .body(Body::from(payload))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
}
