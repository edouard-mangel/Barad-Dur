//! Field-test harness: runs Barad-dûr across a pinned corpus of real
//! repositories and diffs the recommendations it produces.
//!
//! Not part of the shipped product; the `field-test` binary that drives
//! this module is gated behind the `field-test` cargo feature.

pub mod audit;
pub mod baseline;
pub mod corpus;
pub mod diff;
pub mod fetch;
pub mod mode;
pub mod runner;
pub mod surface;
pub mod sweep;
pub mod worktree;
