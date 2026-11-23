use crate::domain::models::{Event, EventException, EventStatus, TimeRange};
use crate::domain::repositories::EventRepository;
use crate::infrastructure::database::begin_tenant_tx;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use ikeda_core::criteria::Criteria;
use ikeda_core::error::AppError;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
struct EventRow {
    id: Uuid,
    tenant_id: Uuid,
    calendar_id: Uuid,
    title: String,
    description: Option<String>,
    starts_at: DateTime<Utc>,
    ends_at: DateTime<Utc>,
    timezone: String,
    rrule: Option<String>,
    status: String,
    external_resource_id: Option<String>,
    metadata: serde_json::Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    deleted_at: Option<DateTime<Utc>>,
}

impl EventRow {
    fn into_event(self) -> Result<Event, AppError> {
        let time_range = TimeRange::new(self.starts_at, self.ends_at)?;
        let status = match self.status.as_str() {
            "CONFIRMED" => EventStatus::Confirmed,
            "TENTATIVE" => EventStatus::Tentative,
            "CANCELLED" => EventStatus::Cancelled,
            _ => EventStatus::Confirmed,
        };

        Ok(Event {
            id: self.id,
            tenant_id: self.tenant_id,
            calendar_id: self.calendar_id,
            title: self.title,
            description: self.description,
            time_range,
            timezone: self.timezone,
            rrule: self.rrule,
            status,
            external_resource_id: self.external_resource_id,
            metadata: self.metadata,
            created_at: self.created_at,
            updated_at: self.updated_at,
            deleted_at: self.deleted_at,
        })
    }
}

#[derive(Debug, FromRow)]
struct EventExceptionRow {
    id: Uuid,
    tenant_id: Uuid,
    event_id: Uuid,
    original_starts_at: DateTime<Utc>,
    is_cancelled: bool,
    rescheduled_starts_at: Option<DateTime<Utc>>,
    rescheduled_ends_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

impl EventExceptionRow {
    fn into_exception(self) -> Result<EventException, AppError> {
        let rescheduled_time_range = match (self.rescheduled_starts_at, self.rescheduled_ends_at) {
            (Some(s), Some(e)) => Some(TimeRange::new(s, e)?),
            _ => None,
        };

        Ok(EventException {
            id: self.id,
            tenant_id: self.tenant_id,
            event_id: self.event_id,
            original_starts_at: self.original_starts_at,
            is_cancelled: self.is_cancelled,
            rescheduled_time_range,
            created_at: self.created_at,
        })
    }
}

/// PostgreSQL implementation of `EventRepository` using SQLx and RLS.
pub struct SqlxEventRepository {
    pool: PgPool,
}

impl SqlxEventRepository {
    /// Constructs a repository backed by a PostgreSQL connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EventRepository for SqlxEventRepository {
    async fn create(&self, tenant_id: Uuid, event: &Event) -> Result<Event, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;

        // Idempotency check: if external_resource_id is set and already exists for this tenant/calendar, return it
        if let Some(ext_id) = &event.external_resource_id {
            let existing = sqlx::query_as::<_, EventRow>(
                r#"
                SELECT id, tenant_id, calendar_id, title, description, starts_at, ends_at,
                       timezone, rrule, status, external_resource_id, metadata,
                       created_at, updated_at, deleted_at
                FROM events
                WHERE calendar_id = $1 AND external_resource_id = $2 AND deleted_at IS NULL
                "#,
            )
            .bind(event.calendar_id)
            .bind(ext_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Idempotency lookup failed: {e}")))?;

            if let Some(row) = existing {
                tx.commit().await.map_err(|e| {
                    AppError::DatabaseError(format!("Transaction commit failed: {e}"))
                })?;
                return row.into_event();
            }
        }

        let status_str = match event.status {
            EventStatus::Confirmed => "CONFIRMED",
            EventStatus::Tentative => "TENTATIVE",
            EventStatus::Cancelled => "CANCELLED",
        };

        let row = sqlx::query_as::<_, EventRow>(
            r#"
            INSERT INTO events (
                id, tenant_id, calendar_id, title, description, starts_at, ends_at,
                timezone, rrule, status, external_resource_id, metadata,
                created_at, updated_at, deleted_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            RETURNING id, tenant_id, calendar_id, title, description, starts_at, ends_at,
                      timezone, rrule, status, external_resource_id, metadata,
                      created_at, updated_at, deleted_at
            "#,
        )
        .bind(event.id)
        .bind(tenant_id)
        .bind(event.calendar_id)
        .bind(&event.title)
        .bind(&event.description)
        .bind(event.time_range.starts_at)
        .bind(event.time_range.ends_at)
        .bind(&event.timezone)
        .bind(&event.rrule)
        .bind(status_str)
        .bind(&event.external_resource_id)
        .bind(&event.metadata)
        .bind(event.created_at)
        .bind(event.updated_at)
        .bind(event.deleted_at)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(format!("Failed to insert event: {e}")))?;

        tx.commit().await.map_err(|e| {
            AppError::DatabaseError(format!("Failed to commit event transaction: {e}"))
        })?;

        row.into_event()
    }

    async fn find_by_id(&self, tenant_id: Uuid, id: Uuid) -> Result<Option<Event>, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;

        let row = sqlx::query_as::<_, EventRow>(
            r#"
            SELECT id, tenant_id, calendar_id, title, description, starts_at, ends_at,
                   timezone, rrule, status, external_resource_id, metadata,
                   created_at, updated_at, deleted_at
            FROM events
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(format!("Failed to find event: {e}")))?;

        tx.commit()
            .await
            .map_err(|e| AppError::DatabaseError(format!("Transaction commit failed: {e}")))?;

        match row {
            Some(r) => Ok(Some(r.into_event()?)),
            None => Ok(None),
        }
    }

    async fn find_in_range(
        &self,
        tenant_id: Uuid,
        calendar_id: Uuid,
        range: &TimeRange,
    ) -> Result<Vec<Event>, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;

        // Fetches non-recurring events that overlap, or recurring events where starts_at <= range.ends_at
        let rows = sqlx::query_as::<_, EventRow>(
            r#"
            SELECT id, tenant_id, calendar_id, title, description, starts_at, ends_at,
                   timezone, rrule, status, external_resource_id, metadata,
                   created_at, updated_at, deleted_at
            FROM events
            WHERE calendar_id = $1
              AND deleted_at IS NULL
              AND (
                  (rrule IS NULL AND starts_at < $3 AND ends_at > $2)
                  OR (rrule IS NOT NULL AND starts_at <= $3)
              )
            ORDER BY starts_at ASC
            "#,
        )
        .bind(calendar_id)
        .bind(range.starts_at)
        .bind(range.ends_at)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(format!("Failed to find events in range: {e}")))?;

        tx.commit()
            .await
            .map_err(|e| AppError::DatabaseError(format!("Transaction commit failed: {e}")))?;

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            events.push(row.into_event()?);
        }

        Ok(events)
    }

    async fn find_by_criteria(
        &self,
        tenant_id: Uuid,
        calendar_id: Uuid,
        _criteria: &Criteria,
    ) -> Result<Vec<Event>, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;

        // Fallback default criteria search for events
        let rows = sqlx::query_as::<_, EventRow>(
            r#"
            SELECT id, tenant_id, calendar_id, title, description, starts_at, ends_at,
                   timezone, rrule, status, external_resource_id, metadata,
                   created_at, updated_at, deleted_at
            FROM events
            WHERE calendar_id = $1 AND deleted_at IS NULL
            ORDER BY starts_at ASC
            LIMIT 100
            "#,
        )
        .bind(calendar_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(format!("Failed to search events: {e}")))?;

        tx.commit()
            .await
            .map_err(|e| AppError::DatabaseError(format!("Transaction commit failed: {e}")))?;

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            events.push(row.into_event()?);
        }

        Ok(events)
    }

    async fn find_exceptions(
        &self,
        tenant_id: Uuid,
        event_id: Uuid,
    ) -> Result<Vec<EventException>, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;

        let rows = sqlx::query_as::<_, EventExceptionRow>(
            r#"
            SELECT id, tenant_id, event_id, original_starts_at, is_cancelled,
                   rescheduled_starts_at, rescheduled_ends_at, created_at
            FROM event_exceptions
            WHERE event_id = $1
            ORDER BY original_starts_at ASC
            "#,
        )
        .bind(event_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(format!("Failed to list event exceptions: {e}")))?;

        tx.commit()
            .await
            .map_err(|e| AppError::DatabaseError(format!("Transaction commit failed: {e}")))?;

        let mut exceptions = Vec::with_capacity(rows.len());
        for row in rows {
            exceptions.push(row.into_exception()?);
        }

        Ok(exceptions)
    }

    async fn add_exception(
        &self,
        tenant_id: Uuid,
        exception: &EventException,
    ) -> Result<EventException, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;

        let (rescheduled_s, rescheduled_e) = match exception.rescheduled_time_range {
            Some(range) => (Some(range.starts_at), Some(range.ends_at)),
            None => (None, None),
        };

        let row = sqlx::query_as::<_, EventExceptionRow>(
            r#"
            INSERT INTO event_exceptions (
                id, tenant_id, event_id, original_starts_at, is_cancelled,
                rescheduled_starts_at, rescheduled_ends_at, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, tenant_id, event_id, original_starts_at, is_cancelled,
                      rescheduled_starts_at, rescheduled_ends_at, created_at
            "#,
        )
        .bind(exception.id)
        .bind(tenant_id)
        .bind(exception.event_id)
        .bind(exception.original_starts_at)
        .bind(exception.is_cancelled)
        .bind(rescheduled_s)
        .bind(rescheduled_e)
        .bind(exception.created_at)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(format!("Failed to insert exception: {e}")))?;

        tx.commit()
            .await
            .map_err(|e| AppError::DatabaseError(format!("Transaction commit failed: {e}")))?;

        row.into_exception()
    }

    async fn update(&self, tenant_id: Uuid, event: &Event) -> Result<Event, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;

        let status_str = match event.status {
            EventStatus::Confirmed => "CONFIRMED",
            EventStatus::Tentative => "TENTATIVE",
            EventStatus::Cancelled => "CANCELLED",
        };

        let row = sqlx::query_as::<_, EventRow>(
            r#"
            UPDATE events
            SET title = $1, description = $2, starts_at = $3, ends_at = $4,
                timezone = $5, rrule = $6, status = $7, metadata = $8, updated_at = NOW()
            WHERE id = $9 AND deleted_at IS NULL
            RETURNING id, tenant_id, calendar_id, title, description, starts_at, ends_at,
                      timezone, rrule, status, external_resource_id, metadata,
                      created_at, updated_at, deleted_at
            "#,
        )
        .bind(&event.title)
        .bind(&event.description)
        .bind(event.time_range.starts_at)
        .bind(event.time_range.ends_at)
        .bind(&event.timezone)
        .bind(&event.rrule)
        .bind(status_str)
        .bind(&event.metadata)
        .bind(event.id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(format!("Failed to update event: {e}")))?
        .ok_or_else(|| AppError::NotFound(format!("Event with ID '{}' not found", event.id)))?;

        tx.commit()
            .await
            .map_err(|e| AppError::DatabaseError(format!("Transaction commit failed: {e}")))?;

        row.into_event()
    }

    async fn soft_delete(&self, tenant_id: Uuid, id: Uuid) -> Result<bool, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;

        let result = sqlx::query(
            r#"
            UPDATE events
            SET deleted_at = NOW(), updated_at = NOW()
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(format!("Failed to soft delete event: {e}")))?;

        tx.commit()
            .await
            .map_err(|e| AppError::DatabaseError(format!("Transaction commit failed: {e}")))?;

        Ok(result.rows_affected() > 0)
    }
}
