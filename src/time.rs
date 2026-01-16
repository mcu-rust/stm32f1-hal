//! Time units
//!
//! See [`HertzU32`], [`KilohertzU32`] and [`MegahertzU32`] for creating increasingly higher frequencies.
//!
//! The [`fugit::ExtU32`] [`U32Ext`] trait adds various methods like `.Hz()`, `.MHz()`, etc to the `u32` primitive type,
//! allowing it to be converted into frequencies.
//!
//! # Examples
//!
//! ## Create a 2 MHz frequency
//!
//! This example demonstrates various ways of creating a 2 MHz (2_000_000 Hz) frequency. They are
//! all equivalent, however the `2.MHz()` variant should be preferred for readability.
//!
//! ```rust
//! use stm32f1xx_hal::{
//!     time::HertzU32,
//!     // Imports U32Ext trait
//!     prelude::*,
//! };
//!
//! let freq_hz = 2_000_000.Hz();
//! let freq_khz = 2_000.kHz();
//! let freq_mhz = 2.MHz();
//!
//! assert_eq!(freq_hz, freq_khz);
//! assert_eq!(freq_khz, freq_mhz);
//! ```

#![allow(non_snake_case)]

use core::ops;
use cortex_m::peripheral::{DCB, DWT};

use crate::rcc::{self, Rcc};
use crate::{
    l::maybe_derive_format,
    os_trait::{TickDuration, TickInstant},
    prelude::*,
};

/// Bits per second
#[maybe_derive_format]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Debug)]
pub struct Bps(pub u32);

pub use fugit::{
    Duration, HertzU32, KilohertzU32, MegahertzU32, MicrosDurationU32, MillisDurationU32,
    RateExtU32,
};

/// Extension trait that adds convenience methods to the `u32` type
pub trait U32Ext {
    /// Wrap in `Bps`
    fn bps(self) -> Bps;
}

impl U32Ext for u32 {
    fn bps(self) -> Bps {
        Bps(self)
    }
}

pub const fn Hz(val: u32) -> HertzU32 {
    HertzU32::from_raw(val)
}

pub const fn kHz(val: u32) -> KilohertzU32 {
    KilohertzU32::from_raw(val)
}

pub const fn MHz(val: u32) -> MegahertzU32 {
    MegahertzU32::from_raw(val)
}

pub const fn ms(val: u32) -> MillisDurationU32 {
    MillisDurationU32::from_ticks(val)
}

pub const fn us(val: u32) -> MicrosDurationU32 {
    MicrosDurationU32::from_ticks(val)
}

/// Macro to implement arithmetic operations (e.g. multiplication, division)
/// for wrapper types.
macro_rules! impl_arithmetic {
    ($wrapper:ty, $wrapped:ty) => {
        impl ops::Mul<$wrapped> for $wrapper {
            type Output = Self;
            fn mul(self, rhs: $wrapped) -> Self {
                Self(self.0 * rhs)
            }
        }

        impl ops::MulAssign<$wrapped> for $wrapper {
            fn mul_assign(&mut self, rhs: $wrapped) {
                self.0 *= rhs;
            }
        }

        impl ops::Div<$wrapped> for $wrapper {
            type Output = Self;
            fn div(self, rhs: $wrapped) -> Self {
                Self(self.0 / rhs)
            }
        }

        impl ops::Div<$wrapper> for $wrapper {
            type Output = $wrapped;
            fn div(self, rhs: $wrapper) -> $wrapped {
                self.0 / rhs.0
            }
        }

        impl ops::DivAssign<$wrapped> for $wrapper {
            fn div_assign(&mut self, rhs: $wrapped) {
                self.0 /= rhs;
            }
        }
    };
}

impl_arithmetic!(Bps, u32);

/// A monotonic non-decreasing timer
///
/// This uses the timer in the debug watch trace peripheral. This means, that if the
/// core is stopped, the timer does not count up. This may be relevant if you are using
/// cortex_m_semihosting::hprintln for debugging in which case the timer will be stopped
/// while printing
#[derive(Clone, Copy)]
pub struct MonoTimer {
    frequency: HertzU32,
}

impl MonoTimer {
    /// Creates a new `Monotonic` timer
    pub fn new(mut dwt: DWT, mut dcb: DCB, rcc: &Rcc) -> Self {
        dcb.enable_trace();
        dwt.enable_cycle_counter();
        let frequency = rcc.clocks().hclk();
        // now the CYCCNT counter can't be stopped or reset

        MonoTimer { frequency }
    }

    /// Returns the frequency at which the monotonic timer is operating at
    pub fn frequency(self) -> HertzU32 {
        self.frequency
    }

    /// Returns an `Instant` corresponding to "now"
    pub fn now(self) -> Instant {
        Instant {
            now: DWT::cycle_count(),
        }
    }
}

/// A measurement of a monotonically non-decreasing clock
#[derive(Clone, Copy)]
pub struct Instant {
    now: u32,
}

impl Instant {
    /// Ticks elapsed since the `Instant` was created
    pub fn elapsed(self) -> u32 {
        DWT::cycle_count().wrapping_sub(self.now)
    }
}

// ----------------------------------------------------------------------------

/// A `TickInstant` implementation
#[derive(Copy, Clone)]
pub struct DwtInstant {
    tick: u32,
    elapsed: u64,
}

impl DwtInstant {
    fn update(&mut self) {
        let now = DWT::cycle_count();
        let tick_diff = if now >= self.tick {
            now - self.tick
        } else {
            now + (u32::MAX - self.tick + 1)
        };
        self.elapsed = self.elapsed.wrapping_add(tick_diff as u64);
        self.tick = now;
    }
}

impl TickInstant for DwtInstant {
    fn frequency() -> KilohertzU32 {
        rcc::get_clocks().hclk().convert()
    }

    #[inline]
    fn now() -> Self {
        Self {
            tick: DWT::cycle_count(),
            elapsed: 0,
        }
    }

    #[inline]
    fn elapsed(&mut self) -> TickDuration<Self> {
        self.update();
        TickDuration::from_ticks(self.elapsed)
    }

    #[inline]
    fn move_forward(&mut self, dur: &TickDuration<Self>) {
        self.elapsed = self.elapsed.wrapping_sub(dur.as_ticks());
    }
}
