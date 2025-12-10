mod i2c1;

pub use crate::common::i2c::*;

use crate::{
    Steal,
    afio::{RemapMode, i2c_remap::*},
    embedded_hal::i2c::NoAcknowledgeSource,
    // dma::{DmaBindRx, DmaBindTx, DmaRingbufTxLoader},
    rcc::{BusClock, Enable, Reset},
};

use crate::{Mcu, time::*};

pub trait I2cInit<T> {
    fn init(self, mcu: &mut Mcu) -> I2c<T>;
}

pub trait I2cConfig: I2cPeriph + BusClock + Enable + Reset + Steal {
    fn config(&mut self, mode: Mode, mcu: &mut Mcu);
    fn set_ack(&mut self, en: bool);
    /// Continue after the address has been sent.
    fn continue_after_addr(&mut self);
    fn send_addr(&mut self, addr: u8, read: bool);
    fn set_interrupt(&mut self, it: Interrupt, en: bool);
    fn disable_all_interrupt(&mut self);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Interrupt {
    Error,
    Event,
    Buffer,
}

// wrapper
pub struct I2c<T> {
    i2c: T,
}

impl<T: I2cConfig> I2c<T> {
    pub fn into_interrupt<REMAP: RemapMode<T>>(
        mut self,
        _pins: (impl I2cSclPin<REMAP>, impl I2cSdaPin<REMAP>),
        mode: Mode,
        mcu: &mut Mcu,
    ) {
        REMAP::remap(&mut mcu.afio);
        self.i2c.config(mode, mcu);
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum DutyCycle {
    Ratio2to1,
    Ratio16to9,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Mode {
    Standard {
        frequency: Hertz,
    },
    Fast {
        frequency: Hertz,
        duty_cycle: DutyCycle,
    },
}

impl Mode {
    pub fn standard(frequency: Hertz) -> Self {
        Mode::Standard { frequency }
    }

    pub fn fast(frequency: Hertz, duty_cycle: DutyCycle) -> Self {
        Mode::Fast {
            frequency,
            duty_cycle,
        }
    }

    pub fn get_frequency(&self) -> Hertz {
        match *self {
            Mode::Standard { frequency } => frequency,
            Mode::Fast { frequency, .. } => frequency,
        }
    }
}

impl From<Hertz> for Mode {
    fn from(frequency: Hertz) -> Self {
        if frequency <= kHz(100) {
            Self::Standard { frequency }
        } else {
            Self::Fast {
                frequency,
                duty_cycle: DutyCycle::Ratio2to1,
            }
        }
    }
}
