use indicatif::ProgressBar;

/// Trait for reporting progress during collection phases.
/// Implemented by `ProgressBar` for real display and `NoProgress` for silent operation.
pub trait Progress: Send + Sync {
    fn inc(&self, delta: u64);
}

impl Progress for ProgressBar {
    fn inc(&self, delta: u64) {
        ProgressBar::inc(self, delta);
    }
}

/// No-op progress reporter — used in tests and when progress display is disabled.
pub struct NoProgress;

impl Progress for NoProgress {
    fn inc(&self, _delta: u64) {}
}
