mod i2c1;
mod i2c2;

pub use crate::common::i2c::*;

use crate::{
    Steal,
    afio::{RemapMode, i2c_remap::*},
    os_trait::Mutex,
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
    pub fn into_interrupt_bus<OS, REMAP>(
        mut self,
        _pins: (impl I2cSclPin<REMAP>, impl I2cSdaPin<REMAP>),
        mode: Mode,
        mcu: &mut Mcu,
    ) -> I2cDeviceBuilder<OS, I2cBusInterrupt<OS, I>>
    where
        OS: OsInterface,
        REMAP: RemapMode<I>,
    {
        REMAP::remap(&mut mcu.afio);
        self.i2c.config(mode, mcu);
        // TODO shift left addr
        let (bus, it, it_err) = I2cBusInterrupt::<OS, I>::new(self.i2c, 10);
        I2cDeviceBuilder::new(bus)
    }

    pub fn into_interrupt_sole<OS, REMAP>(
        mut self,
        _pins: (impl I2cSclPin<REMAP>, impl I2cSdaPin<REMAP>),
        mode: Mode,
        slave_addr: Address,
        mcu: &mut Mcu,
    ) -> (
        // I2cSoleDevice<I2cBusInterrupt<OS, I, A>, A>,
        impl BusDeviceWithAddress<u8>,
        I2cBusInterruptHandler<OS, I>,
        I2cBusErrorInterruptHandler<OS, I>,
    )
    where
        OS: OsInterface,
        REMAP: RemapMode<I>,
    {
        REMAP::remap(&mut mcu.afio);
        self.i2c.config(mode, mcu);
        let (bus, it, it_err) = I2cBusInterrupt::<OS, I>::new(self.i2c, 10);
        // TODO
        (
            I2cSoleDevice::new(bus, convert_addr(slave_addr)),
            it,
            it_err,
        )
    }
}

pub struct I2cDeviceBuilder<OS, BUS>
where
    OS: OsInterface,
    BUS: I2cBusInterface,
{
    bus: Arc<Mutex<OS, BUS>>,
}

impl<OS, BUS> I2cDeviceBuilder<OS, BUS>
where
    OS: OsInterface,
    BUS: I2cBusInterface,
{
    fn new(bus: BUS) -> Self {
        Self {
            bus: Arc::new(OS::mutex(bus)),
        }
    }

    pub fn new_device(&mut self, slave_addr: Address) -> I2cBusDevice<OS, BUS> {
        I2cBusDevice::new(convert_addr(slave_addr), self.bus.clone())
    }
}

fn convert_addr(addr: Address) -> Address {
    match addr {
        Address::Seven(addr) => Address::Seven(addr << 1),
        Address::Ten(addr) => {
            let [msb, lsb] = addr.to_be_bytes();
            let msb = ((msb & 0b11) << 1) | 0b11110000;
            Address::Ten(u16::from_be_bytes([msb, lsb]))
        }
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
