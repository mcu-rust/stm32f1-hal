use super::*;
use crate::common::{bus_device::*, os_trait::Mutex};
use core::marker::PhantomData;

pub struct I2cDeviceBuilder<OS, BUS, A>
where
    OS: OsInterface,
    A: AddressMode,
    BUS: I2cBusInterface<A>,
{
    bus: Arc<Mutex<OS, BUS>>,
    _a: PhantomData<A>,
}

impl<OS, BUS, A> I2cDeviceBuilder<OS, BUS, A>
where
    OS: OsInterface,
    A: AddressMode,
    BUS: I2cBusInterface<A>,
{
    pub fn new(bus: BUS) -> Self {
        Self {
            bus: Arc::new(OS::mutex(bus)),
            _a: PhantomData,
        }
    }

    pub fn new_device(&mut self, slave_addr: A) -> I2cBusDevice<OS, BUS, A> {
        I2cBusDevice {
            slave_addr,
            bus: self.bus.clone(),
        }
    }
}

// ------------------------------------------------------------------

pub struct I2cBusDevice<OS, BUS, A>
where
    OS: OsInterface,
    BUS: I2cBusInterface<A>,
    A: AddressMode,
{
    slave_addr: A,
    bus: Arc<Mutex<OS, BUS>>,
}

impl<OS, BUS, A> BusDevice<u8> for I2cBusDevice<OS, BUS, A>
where
    OS: OsInterface,
    BUS: I2cBusInterface<A>,
    A: AddressMode,
{
    #[inline]
    fn write_read(&mut self, write: &[&[u8]], read: &mut [&mut [u8]]) -> Result<(), BusError> {
        let mut bus = self.bus.lock();
        Ok(bus.write_read(self.slave_addr, write, read)?)
    }

    #[inline]
    fn write_read_in_place(&mut self, buf: &mut [u8]) -> Result<(), BusError> {
        let write = unsafe { core::slice::from_raw_parts(buf.as_ptr(), buf.len()) };
        self.write_read(&[write], &mut [buf])
    }
}

impl<OS, BUS, A> BusDeviceWithAddress<u8> for I2cBusDevice<OS, BUS, A>
where
    OS: OsInterface,
    BUS: I2cBusInterface<A>,
    A: AddressMode,
{
    fn set_address(&mut self, address: u16) {
        self.slave_addr = A::from_u16(address);
    }
}

// ------------------------------------------------------------------

pub struct I2cSoleDevice<BUS, A>
where
    BUS: I2cBusInterface<A>,
    A: AddressMode,
{
    slave_addr: A,
    bus: BUS,
}

impl<BUS, A> I2cSoleDevice<BUS, A>
where
    BUS: I2cBusInterface<A>,
    A: AddressMode,
{
    pub fn new(bus: BUS, slave_addr: A) -> Self {
        Self { bus, slave_addr }
    }
}

impl<BUS, A> BusDevice<u8> for I2cSoleDevice<BUS, A>
where
    BUS: I2cBusInterface<A>,
    A: AddressMode,
{
    #[inline]
    fn write_read(&mut self, write: &[&[u8]], read: &mut [&mut [u8]]) -> Result<(), BusError> {
        Ok(self.bus.write_read(self.slave_addr, write, read)?)
    }

    #[inline]
    fn write_read_in_place(&mut self, buf: &mut [u8]) -> Result<(), BusError> {
        let write = unsafe { core::slice::from_raw_parts(buf.as_ptr(), buf.len()) };
        self.write_read(&[write], &mut [buf])
    }
}

impl<BUS, A> BusDeviceWithAddress<u8> for I2cSoleDevice<BUS, A>
where
    BUS: I2cBusInterface<A>,
    A: AddressMode,
{
    fn set_address(&mut self, address: u16) {
        self.slave_addr = A::from_u16(address);
    }
}

// ------------------------------------------------------------------

impl From<Error> for BusError {
    fn from(value: Error) -> Self {
        match value {
            Error::Busy => Self::Busy,
            Error::ArbitrationLoss => Self::ArbitrationLoss,
            Error::NoAcknowledge(_) => Self::NoAcknowledge,
            Error::Timeout => Self::Timeout,
            _ => Self::Other,
        }
    }
}
