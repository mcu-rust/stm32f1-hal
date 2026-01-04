use super::*;
use crate::common::{bus_device::*, os_trait::Mutex};

pub struct I2cBusDevice<OS, BUS>
where
    OS: OsInterface,
    BUS: I2cBusInterface,
{
    slave_addr: Address,
    bus: Arc<Mutex<OS, BUS>>,
}

unsafe impl<OS, BUS> Send for I2cBusDevice<OS, BUS>
where
    OS: OsInterface,
    BUS: I2cBusInterface,
{
}

impl<OS, BUS> I2cBusDevice<OS, BUS>
where
    OS: OsInterface,
    BUS: I2cBusInterface,
{
    pub fn new(bus: Arc<Mutex<OS, BUS>>, slave_addr: Address) -> Self {
        Self { slave_addr, bus }
    }
}

impl<OS, BUS> BusDevice<u8> for I2cBusDevice<OS, BUS>
where
    OS: OsInterface,
    BUS: I2cBusInterface,
{
    #[inline]
    fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), BusError> {
        let mut bus = self.bus.lock();
        Ok(bus.transaction(self.slave_addr, operations)?)
    }
}

impl<OS, BUS> BusDeviceAddress<u8> for I2cBusDevice<OS, BUS>
where
    OS: OsInterface,
    BUS: I2cBusInterface,
{
    fn set_address(&mut self, address: Address) {
        self.slave_addr = address;
    }
}

// ------------------------------------------------------------------

pub struct I2cSoleDevice<BUS>
where
    BUS: I2cBusInterface,
{
    slave_addr: Address,
    bus: BUS,
}

impl<BUS> I2cSoleDevice<BUS>
where
    BUS: I2cBusInterface,
{
    pub fn new(bus: BUS, slave_addr: Address) -> Self {
        Self { bus, slave_addr }
    }
}

unsafe impl<BUS> Send for I2cSoleDevice<BUS> where BUS: I2cBusInterface {}

impl<BUS> BusDevice<u8> for I2cSoleDevice<BUS>
where
    BUS: I2cBusInterface,
{
    #[inline]
    fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), BusError> {
        Ok(self.bus.transaction(self.slave_addr, operations)?)
    }
}

impl<BUS> BusDeviceAddress<u8> for I2cSoleDevice<BUS>
where
    BUS: I2cBusInterface,
{
    fn set_address(&mut self, address: Address) {
        self.slave_addr = address;
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
