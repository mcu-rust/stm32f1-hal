//! RTIC Monotonic implementation

use super::*;
use core::ops::{Deref, DerefMut};

pub struct MonoTimer<TIM, const FREQ: u32> {
    pub(super) timer: FTimer<TIM, FREQ>,
    pub(super) ovf: u32,
}

impl<TIM, const FREQ: u32> Deref for MonoTimer<TIM, FREQ> {
    type Target = FTimer<TIM, FREQ>;
    fn deref(&self) -> &Self::Target {
        &self.timer
    }
}

impl<TIM, const FREQ: u32> DerefMut for MonoTimer<TIM, FREQ> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.timer
    }
}

/// `MonoTimer` with precision of 1 μs (1 MHz sampling)
pub type MonoTimerUs<TIM> = MonoTimer<TIM, 1_000_000>;

pub trait MonoTimerExt: Sized {
    fn monotonic<const FREQ: u32>(self, mcu: &mut Mcu) -> MonoTimer<Self, FREQ>;
    fn monotonic_us(self, mcu: &mut Mcu) -> MonoTimer<Self, 1_000_000> {
        self.monotonic::<1_000_000>(mcu)
    }
}
