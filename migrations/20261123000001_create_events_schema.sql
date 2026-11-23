-- IkedaDev Platform: Events API Database Schema with PostgreSQL Row-Level Security (RLS)

-- Enable UUID extension if not already available
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- ============================================================================
-- 1. Calendars Table
-- ============================================================================
CREATE TABLE IF NOT EXISTS calendars (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    name VARCHAR(255) NOT NULL,
    timezone VARCHAR(100) NOT NULL DEFAULT 'UTC',
    allow_overlaps BOOLEAN NOT NULL DEFAULT true,
    working_hours JSONB DEFAULT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ DEFAULT NULL
);

-- ============================================================================
-- 2. Events Table
-- ============================================================================
CREATE TABLE IF NOT EXISTS events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    calendar_id UUID NOT NULL REFERENCES calendars(id) ON DELETE CASCADE,
    title VARCHAR(255) NOT NULL,
    description TEXT DEFAULT NULL,
    starts_at TIMESTAMPTZ NOT NULL,
    ends_at TIMESTAMPTZ NOT NULL,
    timezone VARCHAR(100) NOT NULL DEFAULT 'UTC',
    rrule TEXT DEFAULT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'CONFIRMED',
    external_resource_id VARCHAR(255) DEFAULT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ DEFAULT NULL,
    CONSTRAINT chk_event_time_range CHECK (ends_at >= starts_at)
);

-- ============================================================================
-- 3. Event Exceptions Table (Cancelled or Rescheduled Recurrence Instances)
-- ============================================================================
CREATE TABLE IF NOT EXISTS event_exceptions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    event_id UUID NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    original_starts_at TIMESTAMPTZ NOT NULL,
    is_cancelled BOOLEAN NOT NULL DEFAULT false,
    rescheduled_starts_at TIMESTAMPTZ DEFAULT NULL,
    rescheduled_ends_at TIMESTAMPTZ DEFAULT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_rescheduled_time_range CHECK (
        (is_cancelled = true) OR
        (rescheduled_starts_at IS NOT NULL AND rescheduled_ends_at IS NOT NULL AND rescheduled_ends_at >= rescheduled_starts_at)
    )
);

-- ============================================================================
-- 4. Attendees Table
-- ============================================================================
CREATE TABLE IF NOT EXISTS attendees (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    event_id UUID NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    email VARCHAR(255) NOT NULL,
    rsvp_status VARCHAR(50) NOT NULL DEFAULT 'NEEDS_ACTION',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================================
-- 5. Performance Indexes
-- ============================================================================
CREATE INDEX IF NOT EXISTS idx_calendars_tenant ON calendars(tenant_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_events_tenant_calendar ON events(tenant_id, calendar_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_events_time_range ON events(tenant_id, starts_at, ends_at) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_events_external_id ON events(tenant_id, external_resource_id) WHERE external_resource_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_event_exceptions_event ON event_exceptions(tenant_id, event_id);
CREATE INDEX IF NOT EXISTS idx_attendees_event ON attendees(tenant_id, event_id);
CREATE INDEX IF NOT EXISTS idx_attendees_email ON attendees(tenant_id, email);

-- ============================================================================
-- 6. Row-Level Security (RLS) Multi-Tenant Policies
-- ============================================================================
-- Force and enable RLS on all tenant-isolated tables
ALTER TABLE calendars ENABLE ROW LEVEL SECURITY;
ALTER TABLE calendars FORCE ROW LEVEL SECURITY;

ALTER TABLE events ENABLE ROW LEVEL SECURITY;
ALTER TABLE events FORCE ROW LEVEL SECURITY;

ALTER TABLE event_exceptions ENABLE ROW LEVEL SECURITY;
ALTER TABLE event_exceptions FORCE ROW LEVEL SECURITY;

ALTER TABLE attendees ENABLE ROW LEVEL SECURITY;
ALTER TABLE attendees FORCE ROW LEVEL SECURITY;

-- Tenant Isolation Policies via PostgreSQL current_setting transaction session
CREATE POLICY tenant_isolation_calendars ON calendars
    FOR ALL
    USING (tenant_id = NULLIF(current_setting('app.current_tenant', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('app.current_tenant', true), '')::uuid);

CREATE POLICY tenant_isolation_events ON events
    FOR ALL
    USING (tenant_id = NULLIF(current_setting('app.current_tenant', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('app.current_tenant', true), '')::uuid);

CREATE POLICY tenant_isolation_event_exceptions ON event_exceptions
    FOR ALL
    USING (tenant_id = NULLIF(current_setting('app.current_tenant', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('app.current_tenant', true), '')::uuid);

CREATE POLICY tenant_isolation_attendees ON attendees
    FOR ALL
    USING (tenant_id = NULLIF(current_setting('app.current_tenant', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('app.current_tenant', true), '')::uuid);
