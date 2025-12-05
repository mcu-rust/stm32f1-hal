use super::*;

/// Timer wrapper for fixed precision timers.
///
/// Uses `fugit::TimerDurationU32` for most of operations
pub struct FTimer<TIM, const FREQ: u32> {
    pub(crate) tim: TIM,
    clk: Hertz,
}

/// `FTimer` with precision of 1 μs (1 MHz sampling)
pub type FTimerUs<TIM> = FTimer<TIM, 1_000_000>;

/// `FTimer` with precision of 1 ms (1 kHz sampling)
///
/// NOTE: don't use this if your system frequency more than 65 MHz
pub type FTimerMs<TIM> = FTimer<TIM, 1_000>;

impl<TIM: GeneralTimer, const FREQ: u32> FTimer<TIM, FREQ> {
    /// Initialize timer
    pub fn new(tim: TIM, clk: Hertz) -> Self {
        let mut t = Self { tim, clk };
        t.configure();
        t
    }

    /// Calculate prescaler depending on `Clocks` state
    pub fn configure(&mut self) {
        assert!(self.clk.raw() % FREQ == 0);
        let psc = self.clk.raw() / FREQ;
        self.tim.set_prescaler(u16::try_from(psc - 1).unwrap());
    }

    pub fn counter(self) -> Counter<TIM, FREQ> {
        Counter(self)
    }

    /// Releases the TIM peripheral
    pub fn release(self) -> TIM {
        self.tim
    }

    /// Starts listening for an `event`
    ///
    /// Note, you will also have to enable the TIM2 interrupt in the NVIC to start
    /// receiving events.
    pub fn listen(&mut self, event: Event) {
        self.tim.listen_interrupt(event, true);
    }

    /// Clears interrupt associated with `event`.
    ///
    /// If the interrupt is not cleared, it will immediately retrigger after
    /// the ISR has finished.
    pub fn clear_interrupt(&mut self, event: Event) {
        self.tim.clear_interrupt_flag(event);
    }

    pub fn get_interrupt(&mut self) -> Event {
        self.tim.get_interrupt_flag()
    }

    /// Stops listening for an `event`
    pub fn unlisten(&mut self, event: Event) {
        self.tim.listen_interrupt(event, false);
    }

    /// Stopping timer in debug mode can cause troubles when sampling the signal
    pub fn stop_in_debug(&mut self, state: bool) {
        self.tim.stop_in_debug(state);
    }
}

impl<TIM: MasterTimer, const FREQ: u32> FTimer<TIM, FREQ> {
    pub fn set_master_mode(&mut self, mode: MasterMode) {
        self.tim.master_mode(mode)
    }
}
