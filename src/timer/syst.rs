//! SysTick: System Timer

use super::*;
use crate::Mcu;
use crate::os_trait::{utils::FrequencyHolder, *};
use core::ops::{Deref, DerefMut};
use cortex_m::peripheral::{SYST, syst::SystClkSource};
use fugit::{HertzU32, TimerDurationU32, TimerInstantU32};

pub trait SysTimerInit: Sized {
    /// Creates timer which takes [HertzU32] as Duration
    fn counter_hz(self, mcu: &Mcu) -> SysCounterHz;
    /// Creates timer with custom precision (core frequency recommended is known)
    fn counter<const FREQ: u32>(self, mcu: &Mcu) -> SysCounter<FREQ>;
    /// Creates timer with precision of 1 μs (1 MHz sampling)
    fn counter_us(self, mcu: &Mcu) -> SysCounterUs {
        self.counter::<1_000_000>(mcu)
    }
    /// It's important for `TickInstant`
    fn store_tick_frequency(&mut self, mcu: &Mcu);
}

impl SysTimerInit for SYST {
    fn counter_hz(self, mcu: &Mcu) -> SysCounterHz {
        SystemTimer::syst(self, mcu).counter_hz()
    }
    fn counter<const FREQ: u32>(self, mcu: &Mcu) -> SysCounter<FREQ> {
        SystemTimer::syst(self, mcu).counter()
    }
    fn store_tick_frequency(&mut self, mcu: &Mcu) {
        let clk = match self.get_clock_source() {
            SystClkSource::Core => mcu.rcc.clocks.hclk(),
            SystClkSource::External => mcu.rcc.clocks.hclk() / 8,
        };
        FREQUENCY.set(KilohertzU32::Hz(clk.to_Hz()));
    }
}

pub static FREQUENCY: FrequencyHolder = FrequencyHolder::new(KilohertzU32::MHz(1));

pub struct SystemTimer {
    pub(super) syst: SYST,
    pub(super) clk: HertzU32,
}
impl SystemTimer {
    /// Initialize SysTick timer
    pub fn syst(mut syst: SYST, mcu: &Mcu) -> Self {
        syst.set_clock_source(SystClkSource::Core);
        let clk = mcu.rcc.clocks.hclk();
        FREQUENCY.set(KilohertzU32::Hz(clk.to_Hz()));
        Self { syst, clk }
    }

    /// Initialize SysTick timer and set it frequency to `HCLK / 8`
    pub fn syst_external(mut syst: SYST, mcu: &Mcu) -> Self {
        syst.set_clock_source(SystClkSource::External);
        let clk = mcu.rcc.clocks.hclk() / 8;
        FREQUENCY.set(KilohertzU32::Hz(clk.to_Hz()));
        Self { syst, clk }
    }

    pub fn release(self) -> SYST {
        self.syst
    }

    /// Starts listening for an `event`
    pub fn listen(&mut self, event: SysEvent) {
        match event {
            SysEvent::Update => self.syst.enable_interrupt(),
        }
    }

    /// Stops listening for an `event`
    pub fn unlisten(&mut self, event: SysEvent) {
        match event {
            SysEvent::Update => self.syst.disable_interrupt(),
        }
    }

    /// Resets the counter
    pub fn reset(&mut self) {
        // According to the Cortex-M3 Generic User Guide, the interrupt request is only generated
        // when the counter goes from 1 to 0, so writing zero should not trigger an interrupt
        self.syst.clear_current();
    }
}

// Counter --------------------------------------------------------------------

impl SystemTimer {
    /// Creates [SysCounterHz] which takes [HertzU32] as Duration
    pub fn counter_hz(self) -> SysCounterHz {
        SysCounterHz(self)
    }

    /// Creates [SysCounter] with custom precision (core frequency recommended is known)
    pub fn counter<const FREQ: u32>(self) -> SysCounter<FREQ> {
        SysCounter(self)
    }

    /// Creates [SysCounter] 1 microsecond precision
    pub fn counter_us(self) -> SysCounterUs {
        SysCounter(self)
    }
}

/// Hardware timers
pub struct SysCounterHz(SystemTimer);

impl Deref for SysCounterHz {
    type Target = SystemTimer;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SysCounterHz {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl SysCounterHz {
    pub fn start(&mut self, timeout: HertzU32) -> Result<(), Error> {
        let rvr = self.clk.raw() / timeout.raw() - 1;

        if rvr >= (1 << 24) {
            return Err(Error::WrongAutoReload);
        }

        self.syst.set_reload(rvr);
        self.syst.clear_current();
        self.syst.enable_counter();

        Ok(())
    }

    pub fn wait(&mut self) -> nb::Result<(), Error> {
        if self.syst.has_wrapped() {
            Ok(())
        } else {
            Err(nb::Error::WouldBlock)
        }
    }

    pub fn cancel(&mut self) -> Result<(), Error> {
        if !self.syst.is_counter_enabled() {
            return Err(Error::Disabled);
        }

        self.syst.disable_counter();
        Ok(())
    }
}

pub type SysCounterUs = SysCounter<1_000_000>;

/// SysTick timer with precision of 1 μs (1 MHz sampling)
pub struct SysCounter<const FREQ: u32>(SystemTimer);

impl<const FREQ: u32> Deref for SysCounter<FREQ> {
    type Target = SystemTimer;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<const FREQ: u32> DerefMut for SysCounter<FREQ> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<const FREQ: u32> SysCounter<FREQ> {
    /// Starts listening for an `event`
    pub fn listen(&mut self, event: SysEvent) {
        match event {
            SysEvent::Update => self.syst.enable_interrupt(),
        }
    }

    /// Stops listening for an `event`
    pub fn unlisten(&mut self, event: SysEvent) {
        match event {
            SysEvent::Update => self.syst.disable_interrupt(),
        }
    }

    pub fn now(&self) -> TimerInstantU32<FREQ> {
        TimerInstantU32::from_ticks(SYST::get_current() / (self.clk.raw() / FREQ))
    }

    pub fn start(&mut self, timeout: TimerDurationU32<FREQ>) -> Result<(), Error> {
        let rvr = timeout.ticks() * (self.clk.raw() / FREQ) - 1;

        if rvr >= (1 << 24) {
            return Err(Error::WrongAutoReload);
        }

        self.syst.set_reload(rvr);
        self.syst.clear_current();
        self.syst.enable_counter();

        Ok(())
    }

    pub fn wait(&mut self) -> nb::Result<(), Error> {
        if self.syst.has_wrapped() {
            Ok(())
        } else {
            Err(nb::Error::WouldBlock)
        }
    }

    pub fn cancel(&mut self) -> Result<(), Error> {
        if !self.syst.is_counter_enabled() {
            return Err(Error::Disabled);
        }

        self.syst.disable_counter();
        Ok(())
    }
}

// ----------------------------------------------------------------------------

/// A `TickInstant` implementation
#[derive(Copy, Clone)]
pub struct SysTickInstant {
    tick: u32,
}
impl TickInstant for SysTickInstant {
    fn frequency() -> KilohertzU32 {
        FREQUENCY.get()
    }

    #[inline(always)]
    fn now() -> Self {
        Self {
            tick: SYST::get_current(),
        }
    }

    #[inline(always)]
    fn tick_since(self, earlier: Self) -> u32 {
        if self.tick <= earlier.tick {
            earlier.tick - self.tick
        } else {
            earlier.tick + (SYST::get_reload() - self.tick + 1)
        }
    }
}
