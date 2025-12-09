pub mod fake_impls;
#[cfg(feature = "std")]
pub mod std_impls;
pub mod tick;

pub use fake_impls::*;
pub use fugit::{ExtU32, MicrosDurationU32};

pub trait Timeout {
    /// Set timeout and start waiting.
    fn start(&self, timeout: MicrosDurationU32) -> impl TimeoutStatus;
}

pub trait TimeoutStatus {
    /// Check if the time limit expires. This function may sleeps for a while,
    /// depends on the implementation.
    fn timeout(&mut self) -> bool;
    /// Reset the timeout condition.
    fn restart(&mut self);
}

/// The difference from [`Timeout`] is that the timeout is set when initialize.
pub trait PresetTimeout {
    /// Start waiting.
    fn start(&self) -> impl TimeoutStatus;
}

pub trait TickInstant: Copy {
    fn now() -> Self;
    /// Returns the amount of ticks elapsed from another instant to this one.
    fn tick_since(self, earlier: Self) -> u32;
    /// Returns the amount of ticks elapsed since this instant.
    fn tick_elapsed(self) -> u32 {
        Self::now().tick_since(self)
    }
}
