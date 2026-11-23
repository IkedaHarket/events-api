//! Application layer for events-api.
//!
//! Orchestrates domain entities and executes business use cases
//! (e.g. CreateCalendar, CreateEvent, FindEventsByRange, CheckAvailability).

pub mod use_cases;

#[cfg(test)]
pub mod tests;

pub use use_cases::*;
