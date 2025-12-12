mod i2c1;
mod i2c2;

pub use crate::common::i2c::*;

use crate::{
    Steal,
    afio::{RemapMode, i2c_remap::*},
    prelude::*,
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
    fn write_data(&mut self, addr: u8);
    fn read_data(&self) -> u8;
    fn set_interrupt(&mut self, it: Interrupt, en: bool);
    fn disable_all_interrupt(&mut self);
    fn it_routine(&self);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Interrupt {
    Error,
    Event,
    Buffer,
}

// wrapper
pub struct I2c<I> {
    i2c: I,
}

impl<I: I2cConfig> I2c<I> {
    pub fn into_interrupt_bus<OS, A, REMAP>(
        mut self,
        _pins: (impl I2cSclPin<REMAP>, impl I2cSdaPin<REMAP>),
        mode: Mode,
        mcu: &mut Mcu,
    ) -> I2cDeviceBuilder<OS, I2cBusInterrupt<OS, I, A>, A>
    where
        OS: OsInterface,
        A: AddressMode,
        REMAP: RemapMode<I>,
    {
        REMAP::remap(&mut mcu.afio);
        self.i2c.config(mode, mcu);
        let (bus, it, it_err) = I2cBusInterrupt::<OS, I, A>::new(self.i2c, 10);
        I2cDeviceBuilder::new(bus)
    }

    pub fn into_interrupt_sole<OS, A, REMAP>(
        mut self,
        _pins: (impl I2cSclPin<REMAP>, impl I2cSdaPin<REMAP>),
        mode: Mode,
        mcu: &mut Mcu,
    ) -> (
        // I2cSoleDevice<I2cBusInterrupt<OS, I, A>, A>,
        impl BusDeviceWithAddress<u8>,
        I2cBusInterruptHandler<OS, I, A>,
        I2cBusErrorInterruptHandler<OS, I>,
    )
    where
        OS: OsInterface,
        A: AddressMode,
        REMAP: RemapMode<I>,
    {
        REMAP::remap(&mut mcu.afio);
        self.i2c.config(mode, mcu);
        let (bus, it, it_err) = I2cBusInterrupt::<OS, I, A>::new(self.i2c, 10);
        (I2cSoleDevice::new(bus, A::from_u16(0)), it, it_err)
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
