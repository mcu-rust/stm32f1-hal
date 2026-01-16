#![allow(unused_variables)]
type TimerX = pac::TIM9;
type Width = u16;

// $sync begin

use super::*;
use crate::{Mcu, pac};

impl TimerConfig for TimerX {}

impl TimerInit<TimerX> for TimerX {
    fn init(self, mcu: &mut Mcu) -> Timer<TimerX> {
        Timer::new(self, mcu)
    }
}

impl GeneralTimer for TimerX {
    #[inline(always)]
    fn reset_config(&mut self) {
        self.cr1().reset();
    }

    #[inline(always)]
    fn enable_counter(&mut self) {
        self.cr1().modify(|_, w| w.cen().set_bit());
    }

    #[inline(always)]
    fn disable_counter(&mut self) {
        self.cr1().modify(|_, w| w.cen().clear_bit());
    }

    #[inline(always)]
    fn is_counter_enabled(&self) -> bool {
        self.cr1().read().cen().is_enabled()
    }

    #[inline(always)]
    fn reset_counter(&mut self) {
        self.cnt().reset();
    }

    #[inline(always)]
    fn max_auto_reload() -> u32 {
        Width::MAX as u32
    }

    #[inline(always)]
    unsafe fn set_auto_reload_unchecked(&mut self, arr: u32) {
        unsafe {
            self.arr().write(|w| w.bits(arr));
        }
    }

    #[inline(always)]
    fn set_auto_reload(&mut self, arr: u32) -> Result<(), Error> {
        // Note: Make it impossible to set the ARR value to 0, since this
        // would cause an infinite loop.
        if arr > 0 && arr <= Self::max_auto_reload() {
            unsafe { self.set_auto_reload_unchecked(arr) }
            Ok(())
        } else {
            Err(Error::WrongAutoReload)
        }
    }

    #[inline(always)]
    fn read_auto_reload(&self) -> u32 {
        self.arr().read().bits()
    }

    #[inline(always)]
    fn set_prescaler(&mut self, psc: u16) {
        self.psc().write(|w| w.psc().set(psc));
    }

    #[inline(always)]
    fn read_prescaler(&self) -> u16 {
        self.psc().read().psc().bits()
    }

    #[inline(always)]
    fn read_count(&self) -> u32 {
        self.cnt().read().bits()
    }

    #[inline(always)]
    fn trigger_update(&mut self) {
        // Sets the URS bit to prevent an interrupt from being triggered by
        // the UG bit
        self.cr1().modify(|_, w| w.urs().set_bit());
        self.egr().write(|w| w.ug().set_bit());
        self.cr1().modify(|_, w| w.urs().clear_bit());
    }

    #[inline]
    fn config_freq(&mut self, clock: HertzU32, update_freq: HertzU32) -> Result<(), Error> {
        let (prescaler, arr) = compute_prescaler_arr(clock.raw(), update_freq.raw());
        self.set_prescaler(prescaler as u16);
        self.set_auto_reload(arr)?;
        // Trigger update event to load the registers
        self.trigger_update();
        Ok(())
    }

    #[inline(always)]
    fn clear_interrupt_flag(&mut self, event: Event) {
        self.sr()
            .write(|w| unsafe { w.bits(0xffff & !event.bits()) });
    }

    #[inline(always)]
    fn listen_interrupt(&mut self, event: Event, b: bool) {
        self.dier().modify(|r, w| unsafe {
            w.bits(if b {
                r.bits() | event.bits()
            } else {
                r.bits() & !event.bits()
            })
        });
    }

    #[inline(always)]
    fn get_interrupt_flag(&self) -> Event {
        Event::from_bits_truncate(self.sr().read().bits())
    }

    #[inline(always)]
    fn start_one_pulse(&mut self) {
        self.cr1().modify(|_, w| w.opm().set_bit().cen().set_bit());
    }

    #[inline(always)]
    fn stop_in_debug(&mut self, state: bool) {
        let dbg = unsafe { DBG::steal() };
        // $sync dbg_t9
        // TODO: restore these timers once stm32-rs has been updated
        // dbg.cr().modify(|_, w| w.dbg_tim9_stop().bit(state));
        // $sync dbg_end
    }

    #[inline(always)]
    fn enable_preload(&mut self, b: bool) {
        self.cr1().modify(|_, w| w.arpe().bit(b));
    }
}

// $sync pwm
// PWM ------------------------------------------------------------------------

impl TimerWithPwm for TimerX {
    fn stop_pwm(&mut self) {
        self.disable_counter();
    }

    // $sync start_pwm

    #[inline(always)]
    fn start_pwm(&mut self) {
        self.reset_counter();
        self.enable_counter();
    }

    // $sync pwm_cfg_2

    #[inline(always)]
    fn preload_output_channel_in_mode(&mut self, channel: Channel, mode: PwmMode) {
        let mode = Ocm::from(mode);
        match channel {
            Channel::C1 => {
                self.ccmr1_output()
                    .modify(|_, w| w.oc1pe().set_bit().oc1m().set(mode as _));
            }
            Channel::C2 => {
                self.ccmr1_output()
                    .modify(|_, w| w.oc2pe().set_bit().oc2m().set(mode as _));
            }
            _ => (),
        }
    }

    fn set_polarity(&mut self, channel: Channel, polarity: PwmPolarity) {
        match channel {
            Channel::C1 => {
                self.ccer()
                    .modify(|_, w| w.cc1p().bit(polarity == PwmPolarity::ActiveLow));
            }
            Channel::C2 => {
                self.ccer()
                    .modify(|_, w| w.cc2p().bit(polarity == PwmPolarity::ActiveLow));
            }
            _ => (),
        }
    }
}

// $sync pwm_ch1
// PWM Channels ---------------------------------------------------------------

impl TimerWithPwm1Ch for TimerX {
    #[inline(always)]
    fn enable_ch1(&mut self, en: bool) {
        self.ccer().modify(|_, w| w.cc1e().bit(en));
    }

    #[inline(always)]
    fn set_ch1_cc_value(&mut self, value: u32) {
        unsafe { self.ccr1().write(|w| w.bits(value)) };
    }

    #[inline(always)]
    fn get_ch1_cc_value(&self) -> u32 {
        self.ccr1().read().bits()
    }
}

// $sync pwm_ch2

impl TimerWithPwm2Ch for TimerX {
    #[inline(always)]
    fn enable_ch2(&mut self, en: bool) {
        self.ccer().modify(|_, w| w.cc2e().bit(en));
    }

    #[inline(always)]
    fn set_ch2_cc_value(&mut self, value: u32) {
        unsafe { self.ccr2().write(|w| w.bits(value)) };
    }

    #[inline(always)]
    fn get_ch2_cc_value(&self) -> u32 {
        self.ccr2().read().bits()
    }
}

// $sync end
