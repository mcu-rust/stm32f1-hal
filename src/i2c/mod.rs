mod i2c1;
mod i2c2;

pub use crate::common::i2c::*;

use crate::{
    Mcu, Steal,
    afio::{RemapMode, i2c_remap::*},
    os_trait::Mutex,
    prelude::*,
    rcc::{BusClock, Enable, Reset},
    time::*,
};
use core::marker::PhantomData;

pub trait I2cInit<T> {
    fn init<OS: OsInterface>(self, mcu: &mut Mcu) -> I2c<OS, T>;
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
    fn it_clean_needless_flag(&self);
    fn it_prepare_read_inner(
        &mut self,
        addr: Address,
        total_len: usize,
        step: &mut u8,
    ) -> Result<(), bool>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Interrupt {
    Error,
    Event,
    Buffer,
}

// wrapper
pub struct I2c<OS: OsInterface, I> {
    i2c: I,
    _os: PhantomData<OS>,
}

#[allow(clippy::type_complexity)]
impl<OS, I> I2c<OS, I>
where
    OS: OsInterface,
    I: I2cConfig,
{
    pub fn into_interrupt_bus<REMAP>(
        mut self,
        _pins: (impl I2cSclPin<REMAP>, impl I2cSdaPin<REMAP>),
        mode: Mode,
        mcu: &mut Mcu,
    ) -> (
        I2cDeviceBuilder<OS, I2cBusInterrupt<OS, I>>,
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
        (I2cDeviceBuilder::new(bus), it, it_err)
    }

    pub fn into_interrupt_sole<REMAP>(
        mut self,
        _pins: (impl I2cSclPin<REMAP>, impl I2cSdaPin<REMAP>),
        mode: Mode,
        slave_addr: Address,
        mcu: &mut Mcu,
    ) -> (
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
