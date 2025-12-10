use super::*;
use crate::common::embedded_hal::pwm::{ErrorType, SetDutyCycle};
use core::convert::Infallible;

pub struct PwmTimer<TIM> {
    tim: TIM,
    clk: Hertz,
}
impl<TIM: TimerWithPwm> PwmTimer<TIM> {
    pub fn new(tim: TIM, clk: Hertz) -> Self {
        Self { tim, clk }
    }

    #[inline(always)]
    pub fn start(&mut self) {
        self.tim.start_pwm();
    }

    #[inline(always)]
    pub fn stop(&mut self) {
        self.tim.stop_pwm();
    }

    #[inline]
    pub fn get_count_value(&self) -> u32 {
        self.tim.read_count()
    }

    #[inline]
    pub fn get_max_duty(&self) -> u32 {
        self.tim.read_auto_reload().wrapping_add(1)
    }

    #[inline]
    pub fn config_freq(&mut self, update_freq: Hertz) {
        self.tim.config_freq(self.clk, update_freq);
    }
}

// Channels -------------------------------------------------------------------

macro_rules! pwm_channel {
    ($name:ident, $Tim:ident, $ch:expr, $en:ident) => {
        pub struct $name<TIM> {
            tim: TIM,
        }

        impl<TIM> $name<TIM> {
            pub fn new(tim: TIM) -> Self {
                Self { tim }
            }
        }

        impl<TIM: $Tim> ErrorType for $name<TIM> {
            type Error = Infallible;
        }

        impl<TIM: $Tim> SetDutyCycle for $name<TIM> {
            #[inline(always)]
            fn max_duty_cycle(&self) -> u16 {
                (self.tim.read_auto_reload() as u16).wrapping_add(1)
            }

            #[inline(always)]
            fn set_duty_cycle(&mut self, duty: u16) -> Result<(), Self::Error> {
                self.tim.set_ch1_cc_value(duty as u32);
                Ok(())
            }
        }

        impl<TIM: $Tim> PwmChannel for $name<TIM> {
            #[inline(always)]
            fn config(&mut self, mode: PwmMode, polarity: PwmPolarity) {
                self.tim.preload_output_channel_in_mode($ch, mode.into());
                self.tim.set_polarity($ch, polarity);
            }

            #[inline(always)]
            fn set_enable(&mut self, en: bool) {
                self.tim.$en(en);
            }
        }
    };
}
pwm_channel!(PwmChannel1, TimerWithPwm1Ch, Channel::C1, enable_ch1);
pwm_channel!(PwmChannel2, TimerWithPwm2Ch, Channel::C2, enable_ch2);
pwm_channel!(PwmChannel3, TimerWithPwm3Ch, Channel::C3, enable_ch3);
pwm_channel!(PwmChannel4, TimerWithPwm4Ch, Channel::C4, enable_ch4);
