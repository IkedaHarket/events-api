pub mod availability;
pub mod overlap;
pub mod recurrence;
pub mod scheduler;

pub use availability::{calculate_free_slots, merge_busy_intervals};
pub use overlap::{detect_overlaps, find_overlapping};
pub use recurrence::expand_recurrence;
pub use scheduler::{NotificationScheduler, WebhookEventPayload};
