pub mod counter;
pub mod fix_timer;
pub mod pwm;

pub use counter::*;
pub use fix_timer::*;
pub use pwm::*;

use crate::fugit::HertzU32;

pub trait PwmChannel: embedded_hal::pwm::SetDutyCycle {
    fn config(&mut self, mode: PwmMode, polarity: PwmPolarity);
    fn set_enable(&mut self, en: bool);
}

// ----------------------------------------------------------------------------

pub trait GeneralTimer {
    fn reset_config(&mut self);
    fn enable_counter(&mut self);
    fn disable_counter(&mut self);
    fn is_counter_enabled(&self) -> bool;
    fn reset_counter(&mut self);
    fn enable_preload(&mut self, b: bool);
    fn max_auto_reload() -> u32;
    /// # Safety
    ///
    /// `arr` must be greater than 0
    unsafe fn set_auto_reload_unchecked(&mut self, arr: u32);
    fn set_auto_reload(&mut self, arr: u32) -> Result<(), Error>;
    fn read_auto_reload(&self) -> u32;
    fn set_prescaler(&mut self, psc: u16);
    fn read_prescaler(&self) -> u16;
    fn read_count(&self) -> u32;
    fn trigger_update(&mut self);
    fn stop_in_debug(&mut self, state: bool);
    fn config_freq(&mut self, clock: HertzU32, update_freq: HertzU32) -> Result<(), Error>;

    fn clear_interrupt_flag(&mut self, event: Event);
    fn listen_interrupt(&mut self, event: Event, b: bool);
    fn get_interrupt_flag(&self) -> Event;
    fn start_one_pulse(&mut self);
}

pub trait TimerDirection: GeneralTimer {
    fn set_count_direction(&mut self, dir: CountDirection);
}

pub trait MasterTimer: GeneralTimer {
    fn master_mode(&mut self, mode: MasterMode);
}

pub trait TimerWithPwm: GeneralTimer {
    fn start_pwm(&mut self);
    fn stop_pwm(&mut self);

    fn preload_output_channel_in_mode(&mut self, channel: Channel, mode: PwmMode);
    fn set_polarity(&mut self, channel: Channel, polarity: PwmPolarity);
}

pub trait TimerWithPwm1Ch: TimerWithPwm {
    fn enable_ch1(&mut self, en: bool);
    fn set_ch1_cc_value(&mut self, value: u32);
    fn get_ch1_cc_value(&self) -> u32;
}

pub trait TimerWithPwm2Ch: TimerWithPwm1Ch {
    fn enable_ch2(&mut self, en: bool);
    fn set_ch2_cc_value(&mut self, value: u32);
    fn get_ch2_cc_value(&self) -> u32;
}

pub trait TimerWithPwm3Ch: TimerWithPwm2Ch {
    fn enable_ch3(&mut self, en: bool);
    fn set_ch3_cc_value(&mut self, value: u32);
    fn get_ch3_cc_value(&self) -> u32;
}

pub trait TimerWithPwm4Ch: TimerWithPwm3Ch {
    fn enable_ch4(&mut self, en: bool);
    fn set_ch4_cc_value(&mut self, value: u32);
    fn get_ch4_cc_value(&self) -> u32;
}

// Enumerate ------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    C1,
    C2,
    C3,
    C4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CountDirection {
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PwmMode {
    Mode1,
    Mode2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PwmPolarity {
    ActiveHigh,
    ActiveLow,
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum Error {
    /// Timer is disabled
    Disabled,
    WrongAutoReload,
}

/// Interrupt events
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SysEvent {
    /// Timer timed out / count down ended
    Update,
}

bitflags::bitflags! {
    pub struct Event: u32 {
        const Update  = 1 << 0;
        const C1 = 1 << 1;
        const C2 = 1 << 2;
        const C3 = 1 << 3;
        const C4 = 1 << 4;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MasterMode {
    ///0: The UG bit from the TIMx_EGR register is used as trigger output
    Reset,
    ///1: The counter enable signal, CNT_EN, is used as trigger output
    Enable,
    ///2: The update event is selected as trigger output
    Update,
    ///3: The trigger output send a positive pulse when the CC1IF flag it to be set, as soon as a capture or a compare match occurred
    ComparePulse,
    ///4: OC1REF signal is used as trigger output
    CompareOc1,
    ///5: OC2REF signal is used as trigger output
    CompareOc2,
    ///6: OC3REF signal is used as trigger output
    CompareOc3,
    ///7: OC4REF signal is used as trigger output
    CompareOc4,
}
