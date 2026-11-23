//! Domain layer for events-api.
//!
//! Contains pure domain entities, value objects, domain services,
//! and repository interface ports without external framework dependencies.

pub mod models;
pub mod repositories;
pub mod services;

pub use models::{
    AvailabilityResult, Calendar, Event, EventException, EventInstance, EventStatus, TimeRange,
};
pub use repositories::{CalendarRepository, EventRepository};
pub use services::{
    calculate_free_slots, detect_overlaps, expand_recurrence, find_overlapping,
    merge_busy_intervals, NotificationScheduler, WebhookEventPayload,
};
