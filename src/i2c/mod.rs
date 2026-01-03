mod i2c1;
mod i2c2;

pub use crate::common::{bus_device::*, i2c::*};

use crate::{
    Mcu, Steal,
    afio::{RemapMode, i2c_remap::*},
    os_trait::Mutex,
    prelude::*,
    rcc::{Enable, GetClock, Reset},
    time::*,
};
use core::marker::PhantomData;

pub trait I2cInit<T> {
    fn init<OS: OsInterface>(self, mcu: &mut Mcu) -> I2c<OS, T>;
}

pub trait I2cPeriphConfig: I2cPeriph + GetClock + Enable + Reset + Steal {
    fn config(&mut self, mode: Mode);
    fn set_ack(&mut self, en: bool);
    /// Continue after the address has been sent.
    fn continue_after_addr(&mut self);
    fn write_data(&mut self, addr: u8);
    fn read_data(&self) -> u8;
    fn set_interrupt(&mut self, it: Interrupt, en: bool);
    fn it_clean_needless_flag(&self);
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
    I: I2cPeriphConfig,
{
    pub fn into_interrupt_bus<REMAP>(
        self,
        _pins: (impl I2cSclPin<REMAP>, impl I2cSdaPin<REMAP>),
        max_operation: usize,
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
        let (bus, it, it_err) = I2cBusInterrupt::<OS, I>::new(self.i2c, max_operation);
        (I2cDeviceBuilder::new(bus), it, it_err)
    }

    pub fn into_interrupt_sole<'a, 'b, REMAP>(
        self,
        _pins: (impl I2cSclPin<REMAP>, impl I2cSdaPin<REMAP>),
        slave_addr: Address,
        speed: HertzU32,
        max_operation: usize,
        mcu: &'a mut Mcu,
    ) -> (
        impl BusDeviceAddress<u8> + 'b,
        I2cBusInterruptHandler<OS, I>,
        I2cBusErrorInterruptHandler<OS, I>,
    )
    where
        I: 'b,
        OS: OsInterface,
        REMAP: RemapMode<I>,
    {
        REMAP::remap(&mut mcu.afio);
        assert!(speed <= kHz(400));
        let (bus, it, it_err) = I2cBusInterrupt::<OS, I>::new(self.i2c, max_operation);
        (
            I2cSoleDevice::new(bus, convert_addr(slave_addr), speed),
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

    pub fn new_device(&self, slave_addr: Address, speed: HertzU32) -> I2cBusDevice<OS, BUS> {
        assert!(speed <= kHz(400));
        I2cBusDevice::new(self.bus.clone(), convert_addr(slave_addr), speed)
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
        frequency: HertzU32,
    },
    Fast {
        frequency: HertzU32,
        duty_cycle: DutyCycle,
    },
}

impl Mode {
    pub fn standard(frequency: HertzU32) -> Self {
        Mode::Standard { frequency }
    }

    pub fn fast(frequency: HertzU32, duty_cycle: DutyCycle) -> Self {
        Mode::Fast {
            frequency,
            duty_cycle,
        }
    }

    pub fn get_frequency(&self) -> HertzU32 {
        match *self {
            Mode::Standard { frequency } => frequency,
            Mode::Fast { frequency, .. } => frequency,
        }
    }
}

impl From<HertzU32> for Mode {
    fn from(frequency: HertzU32) -> Self {
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
