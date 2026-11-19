// SPDX-License-Identifier: AGPL-3.0-only
//! Backend-neutral, bounded offline speech services. No native or GUI dependencies.
pub mod settings;
pub mod tasks;
pub mod terms;
pub mod wav;
pub use settings::Settings;
pub use tasks::{Audio, Backend, Cancel, Job, Operation, Outcome, Scheduler, Voice};
pub type Result<T> = std::result::Result<T, String>;
#[cfg(test)]
mod stream_tests;
