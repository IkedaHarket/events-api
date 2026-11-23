use crate::domain::models::Calendar;
use crate::domain::repositories::CalendarRepository;
use crate::infrastructure::database::begin_tenant_tx;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use ikeda_core::error::AppError;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
struct CalendarRow {
    id: Uuid,
    tenant_id: Uuid,
    name: String,
    timezone: String,
    allow_overlaps: bool,
    working_hours: Option<serde_json::Value>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    deleted_at: Option<DateTime<Utc>>,
}

impl From<CalendarRow> for Calendar {
    fn from(row: CalendarRow) -> Self {
        Self {
            id: row.id,
            tenant_id: row.tenant_id,
            name: row.name,
            timezone: row.timezone,
            allow_overlaps: row.allow_overlaps,
            working_hours: row.working_hours,
            created_at: row.created_at,
            updated_at: row.updated_at,
            deleted_at: row.deleted_at,
        }
    }
}

/// PostgreSQL implementation of `CalendarRepository` using SQLx and RLS.
pub struct SqlxCalendarRepository {
    pool: PgPool,
}

impl SqlxCalendarRepository {
    /// Constructs a repository backed by a PostgreSQL connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CalendarRepository for SqlxCalendarRepository {
    async fn create(&self, tenant_id: Uuid, calendar: &Calendar) -> Result<Calendar, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;

        let row = sqlx::query_as::<_, CalendarRow>(
            r#"
            INSERT INTO calendars (
                id, tenant_id, name, timezone, allow_overlaps, working_hours, created_at, updated_at, deleted_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id, tenant_id, name, timezone, allow_overlaps, working_hours, created_at, updated_at, deleted_at
            "#,
        )
        .bind(calendar.id)
        .bind(tenant_id)
        .bind(&calendar.name)
        .bind(&calendar.timezone)
        .bind(calendar.allow_overlaps)
        .bind(&calendar.working_hours)
        .bind(calendar.created_at)
        .bind(calendar.updated_at)
        .bind(calendar.deleted_at)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(format!("Failed to insert calendar: {e}")))?;

        tx.commit().await.map_err(|e| {
            AppError::DatabaseError(format!("Failed to commit calendar transaction: {e}"))
        })?;

        Ok(row.into())
    }

    async fn find_by_id(&self, tenant_id: Uuid, id: Uuid) -> Result<Option<Calendar>, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;

        let row = sqlx::query_as::<_, CalendarRow>(
            r#"
            SELECT id, tenant_id, name, timezone, allow_overlaps, working_hours, created_at, updated_at, deleted_at
            FROM calendars
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(format!("Failed to find calendar: {e}")))?;

        tx.commit()
            .await
            .map_err(|e| AppError::DatabaseError(format!("Transaction commit failed: {e}")))?;

        Ok(row.map(Into::into))
    }

    async fn find_by_tenant(&self, tenant_id: Uuid) -> Result<Vec<Calendar>, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;

        let rows = sqlx::query_as::<_, CalendarRow>(
            r#"
            SELECT id, tenant_id, name, timezone, allow_overlaps, working_hours, created_at, updated_at, deleted_at
            FROM calendars
            WHERE deleted_at IS NULL
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(format!("Failed to list calendars: {e}")))?;

        tx.commit()
            .await
            .map_err(|e| AppError::DatabaseError(format!("Transaction commit failed: {e}")))?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn update(&self, tenant_id: Uuid, calendar: &Calendar) -> Result<Calendar, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;

        let row = sqlx::query_as::<_, CalendarRow>(
            r#"
            UPDATE calendars
            SET name = $1, timezone = $2, allow_overlaps = $3, working_hours = $4, updated_at = NOW()
            WHERE id = $5 AND deleted_at IS NULL
            RETURNING id, tenant_id, name, timezone, allow_overlaps, working_hours, created_at, updated_at, deleted_at
            "#,
        )
        .bind(&calendar.name)
        .bind(&calendar.timezone)
        .bind(calendar.allow_overlaps)
        .bind(&calendar.working_hours)
        .bind(calendar.id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(format!("Failed to update calendar: {e}")))?
        .ok_or_else(|| AppError::NotFound(format!("Calendar with ID '{}' not found", calendar.id)))?;

        tx.commit()
            .await
            .map_err(|e| AppError::DatabaseError(format!("Transaction commit failed: {e}")))?;

        Ok(row.into())
    }

    async fn soft_delete(&self, tenant_id: Uuid, id: Uuid) -> Result<bool, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;

        let result = sqlx::query(
            r#"
            UPDATE calendars
            SET deleted_at = NOW(), updated_at = NOW()
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(format!("Failed to delete calendar: {e}")))?;

        tx.commit()
            .await
            .map_err(|e| AppError::DatabaseError(format!("Transaction commit failed: {e}")))?;

        Ok(result.rows_affected() > 0)
    }
}
