//! Kite Mesh daemon library.
//!
//! Exposed as a lib so the integration test can drive two daemons in-process.

#![allow(
    clippy::single_match,
    clippy::large_enum_variant,
    clippy::too_many_arguments
)]

pub mod config;
pub mod daemon;
pub mod event_loop;
pub mod metrics;

pub use daemon::{Inbox, Kited, KitedEvent, KitedHandle};
