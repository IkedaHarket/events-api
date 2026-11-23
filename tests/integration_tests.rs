use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use chrono::{Duration, TimeZone, Utc};
use events_api::application::use_cases::{CreateCalendarCommand, CreateEventCommand};
use events_api::domain::models::*;
use events_api::domain::repositories::{CalendarRepository, EventRepository};
use events_api::domain::services::scheduler::{NotificationScheduler, WebhookEventPayload};
use events_api::infrastructure::MockNotificationScheduler;
use events_api::presentation::{create_router, AppState};
use ikeda_core::criteria::Criteria;
use ikeda_core::error::AppError;
use ikeda_core::tenant::HEADER_TENANT_ID;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use uuid::Uuid;

#[derive(Default)]
struct SharedMockCalendarRepo {
    calendars: Mutex<Vec<Calendar>>,
}

#[async_trait::async_trait]
impl CalendarRepository for SharedMockCalendarRepo {
    async fn create(&self, _tenant_id: Uuid, calendar: &Calendar) -> Result<Calendar, AppError> {
        self.calendars.lock().unwrap().push(calendar.clone());
        Ok(calendar.clone())
    }

    async fn find_by_id(&self, tenant_id: Uuid, id: Uuid) -> Result<Option<Calendar>, AppError> {
        let lock = self.calendars.lock().unwrap();
        Ok(lock
            .iter()
            .find(|c| c.tenant_id == tenant_id && c.id == id && !c.is_deleted())
            .cloned())
    }

    async fn find_by_tenant(&self, tenant_id: Uuid) -> Result<Vec<Calendar>, AppError> {
        let lock = self.calendars.lock().unwrap();
        Ok(lock
            .iter()
            .filter(|c| c.tenant_id == tenant_id && !c.is_deleted())
            .cloned()
            .collect())
    }

    async fn update(&self, tenant_id: Uuid, calendar: &Calendar) -> Result<Calendar, AppError> {
        let mut lock = self.calendars.lock().unwrap();
        if let Some(pos) = lock
            .iter()
            .position(|c| c.tenant_id == tenant_id && c.id == calendar.id)
        {
            lock[pos] = calendar.clone();
            Ok(calendar.clone())
        } else {
            Err(AppError::NotFound("Calendar not found".into()))
        }
    }

    async fn soft_delete(&self, tenant_id: Uuid, id: Uuid) -> Result<bool, AppError> {
        let mut lock = self.calendars.lock().unwrap();
        if let Some(c) = lock
            .iter_mut()
            .find(|c| c.tenant_id == tenant_id && c.id == id)
        {
            c.mark_deleted();
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[derive(Default)]
struct SharedMockEventRepo {
    events: Mutex<Vec<Event>>,
    exceptions: Mutex<Vec<EventException>>,
}

#[async_trait::async_trait]
impl EventRepository for SharedMockEventRepo {
    async fn create(&self, _tenant_id: Uuid, event: &Event) -> Result<Event, AppError> {
        self.events.lock().unwrap().push(event.clone());
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
        tenant_id: Uuid,
        event_id: Uuid,
    ) -> Result<Vec<EventException>, AppError> {
        let lock = self.exceptions.lock().unwrap();
        Ok(lock
            .iter()
            .filter(|ex| ex.tenant_id == tenant_id && ex.event_id == event_id)
            .cloned()
            .collect())
    }

    async fn add_exception(
        &self,
        _tenant_id: Uuid,
        exception: &EventException,
    ) -> Result<EventException, AppError> {
        self.exceptions.lock().unwrap().push(exception.clone());
        Ok(exception.clone())
    }

    async fn update(&self, tenant_id: Uuid, event: &Event) -> Result<Event, AppError> {
        let mut lock = self.events.lock().unwrap();
        if let Some(pos) = lock
            .iter()
            .position(|e| e.tenant_id == tenant_id && e.id == event.id)
        {
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

fn setup_test_app() -> (axum::Router, AppState) {
    let cal_repo = Arc::new(SharedMockCalendarRepo::default());
    let event_repo = Arc::new(SharedMockEventRepo::default());
    let state = AppState::new(cal_repo, event_repo);
    let router = create_router(state.clone());
    (router, state)
}

#[tokio::test]
async fn test_tenant_isolation_tenant_b_cannot_read_or_modify_tenant_a() {
    let (app, state) = setup_test_app();
    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();

    // 1. Tenant A creates Calendar A
    let cal_a = Calendar::new(tenant_a, "Tenant A Private Calendar", "UTC", true, None);
    let created_cal = state.calendar_repo.create(tenant_a, &cal_a).await.unwrap();

    // 2. Tenant A creates Event A
    let now = Utc::now();
    let event_a = Event::new(
        tenant_a,
        created_cal.id,
        "Confidential Board Meeting",
        Some("Top secret".into()),
        TimeRange::new(now, now + Duration::hours(1)).unwrap(),
        "UTC",
        None,
        EventStatus::Confirmed,
        None,
        serde_json::json!({ "revenue": 1000000 }),
    );
    let created_event = state.event_repo.create(tenant_a, &event_a).await.unwrap();

    // 3. Tenant B attempts to read Tenant A's calendar list
    let req = Request::builder()
        .method("GET")
        .uri("/v1/calendars")
        .header(HEADER_TENANT_ID, tenant_b.to_string())
        .body(Body::empty())
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(body_json["data"].as_array().unwrap().len(), 0); // Tenant B sees nothing

    // 4. Tenant B attempts to access Calendar A directly by ID -> 404 Not Found
    let req = Request::builder()
        .method("GET")
        .uri(format!("/v1/calendars/{}", created_cal.id))
        .header(HEADER_TENANT_ID, tenant_b.to_string())
        .body(Body::empty())
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // 5. Tenant B attempts to access Event A directly by ID -> 404 Not Found
    let req = Request::builder()
        .method("GET")
        .uri(format!("/v1/events/{}", created_event.id))
        .header(HEADER_TENANT_ID, tenant_b.to_string())
        .body(Body::empty())
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // 6. Tenant B attempts to delete Event A -> 404 Not Found
    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/v1/events/{}", created_event.id))
        .header(HEADER_TENANT_ID, tenant_b.to_string())
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_overlapping_events_on_strict_calendar_returns_409_conflict() {
    let (app, _) = setup_test_app();
    let tenant_id = Uuid::new_v4();

    // 1. Create strict calendar (allow_overlaps = false)
    let cal_cmd = CreateCalendarCommand {
        name: "Doctor Examination Room".to_string(),
        timezone: Some("UTC".to_string()),
        allow_overlaps: Some(false),
        working_hours: None,
    };

    let req = Request::builder()
        .method("POST")
        .uri("/v1/calendars")
        .header(header::CONTENT_TYPE, "application/json")
        .header(HEADER_TENANT_ID, tenant_id.to_string())
        .body(Body::from(serde_json::to_string(&cal_cmd).unwrap()))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let cal_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    let cal_id = cal_json["data"]["id"].as_str().unwrap();

    let now = Utc::now();
    let start_1 = now + Duration::hours(2);
    let end_1 = now + Duration::hours(3);

    // 2. Schedule First Event (14:00 - 15:00) -> 201 Created
    let event1_cmd = CreateEventCommand {
        calendar_id: Uuid::parse_str(cal_id).unwrap(),
        title: "Surgery Appointment 1".to_string(),
        description: None,
        starts_at: start_1,
        ends_at: end_1,
        timezone: None,
        rrule: None,
        status: None,
        external_resource_id: None,
        metadata: None,
    };

    let req = Request::builder()
        .method("POST")
        .uri("/v1/events")
        .header(header::CONTENT_TYPE, "application/json")
        .header(HEADER_TENANT_ID, tenant_id.to_string())
        .body(Body::from(serde_json::to_string(&event1_cmd).unwrap()))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    // 3. Schedule Colliding Event (14:30 - 15:30) -> 409 Conflict
    let event2_cmd = CreateEventCommand {
        calendar_id: Uuid::parse_str(cal_id).unwrap(),
        title: "Conflicting Surgery".to_string(),
        description: None,
        starts_at: start_1 + Duration::minutes(30),
        ends_at: end_1 + Duration::minutes(30),
        timezone: None,
        rrule: None,
        status: None,
        external_resource_id: None,
        metadata: None,
    };

    let req = Request::builder()
        .method("POST")
        .uri("/v1/events")
        .header(header::CONTENT_TYPE, "application/json")
        .header(HEADER_TENANT_ID, tenant_id.to_string())
        .body(Body::from(serde_json::to_string(&event2_cmd).unwrap()))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let err_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(err_json["success"], false);
    assert!(err_json["message"].as_str().unwrap().contains("conflicts"));
}

#[tokio::test]
async fn test_recurring_event_with_cancellation_exception_returns_exact_occurrences() {
    let (app, state) = setup_test_app();
    let tenant_id = Uuid::new_v4();

    let cal = Calendar::new(tenant_id, "Gym Class", "UTC", true, None);
    let created_cal = state.calendar_repo.create(tenant_id, &cal).await.unwrap();

    // 1. Create a daily recurring event for 5 days
    let start_date = Utc.with_ymd_and_hms(2026, 12, 1, 9, 0, 0).unwrap();
    let end_date = Utc.with_ymd_and_hms(2026, 12, 1, 10, 0, 0).unwrap();

    let event = Event::new(
        tenant_id,
        created_cal.id,
        "Daily Yoga",
        None,
        TimeRange::new(start_date, end_date).unwrap(),
        "UTC",
        Some("FREQ=DAILY;COUNT=5".to_string()),
        EventStatus::Confirmed,
        None,
        serde_json::json!({}),
    );
    let created_event = state.event_repo.create(tenant_id, &event).await.unwrap();

    // 2. Add an exception cancelling day 3 (2026-12-03)
    let day3_dt = Utc.with_ymd_and_hms(2026, 12, 3, 9, 0, 0).unwrap();
    let exception = EventException::cancel(tenant_id, created_event.id, day3_dt);
    state
        .event_repo
        .add_exception(tenant_id, &exception)
        .await
        .unwrap();

    // 3. Query occurrences via GET /v1/events across the 5 days
    let query_start = Utc
        .with_ymd_and_hms(2026, 12, 1, 0, 0, 0)
        .unwrap()
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    let query_end = Utc
        .with_ymd_and_hms(2026, 12, 6, 0, 0, 0)
        .unwrap()
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();

    let uri = format!(
        "/v1/events?calendar_id={}&starts_at={}&ends_at={}",
        created_cal.id, query_start, query_end
    );

    let req = Request::builder()
        .method("GET")
        .uri(&uri)
        .header(HEADER_TENANT_ID, tenant_id.to_string())
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    let instances = body_json["data"].as_array().unwrap();

    // Exactly 4 occurrences returned (Day 1, Day 2, Day 4, Day 5) - Day 3 was cancelled!
    assert_eq!(instances.len(), 4);
}

#[tokio::test]
async fn test_qstash_scheduler_integration() {
    let scheduler = MockNotificationScheduler::default();
    let tenant_id = Uuid::new_v4();
    let event_id = Uuid::new_v4();
    let calendar_id = Uuid::new_v4();
    let now = Utc::now();
    let reminder_time = now + Duration::minutes(15);

    let payload = WebhookEventPayload {
        event_id,
        tenant_id,
        calendar_id,
        event_type: "event.reminder".to_string(),
        starts_at: now + Duration::hours(1),
        ends_at: now + Duration::hours(2),
        title: "Doctor Consultation".to_string(),
        metadata: serde_json::json!({ "customer_email": "patient@example.com" }),
    };

    let msg_id = scheduler
        .schedule_reminder(
            tenant_id,
            "https://api.client.com/webhooks/reminder",
            reminder_time,
            payload.clone(),
        )
        .await
        .unwrap();

    assert!(msg_id.starts_with("mock_msg_"));

    let lock = scheduler.scheduled_reminders.lock().unwrap();
    assert_eq!(lock.len(), 1);
    assert_eq!(lock[0].0, "https://api.client.com/webhooks/reminder");
    assert_eq!(lock[0].1, reminder_time);
    assert_eq!(lock[0].2, payload);
}
