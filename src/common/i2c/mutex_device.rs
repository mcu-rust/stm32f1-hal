use super::{Address, Error, I2cBusInterface};
use embedded_hal::i2c::{ErrorType, I2c, Operation, SevenBitAddress, TenBitAddress};
use os_trait::{Mutex, OsInterface, prelude::*};

pub struct I2cMutexDevice<OS: OsInterface, BUS>
where
    OS: OsInterface,
    BUS: I2cBusInterface,
{
    bus: Arc<Mutex<OS, BUS>>,
}

impl<OS, BUS> I2cMutexDevice<OS, BUS>
where
    OS: OsInterface,
    BUS: I2cBusInterface,
{
    pub fn new(bus: Arc<Mutex<OS, BUS>>) -> Self {
        Self { bus }
    }
}

impl<OS, BUS> I2c<SevenBitAddress> for I2cMutexDevice<OS, BUS>
where
    OS: OsInterface,
    BUS: I2cBusInterface,
{
    #[inline]
    fn transaction(
        &mut self,
        address: SevenBitAddress,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        let mut bus = self.bus.lock();
        bus.bus_transaction(Address::Seven(address), operations)
    }
}

impl<OS, BUS> I2c<TenBitAddress> for I2cMutexDevice<OS, BUS>
where
    OS: OsInterface,
    BUS: I2cBusInterface,
{
    #[inline]
    fn transaction(
        &mut self,
        address: TenBitAddress,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        let mut bus = self.bus.lock();
        bus.bus_transaction(Address::Ten(address), operations)
    }
}

impl<OS, BUS> ErrorType for I2cMutexDevice<OS, BUS>
where
    OS: OsInterface,
    BUS: I2cBusInterface,
{
    type Error = Error;
}
