use core::marker::PhantomData;
use embedded_hal::i2c::{AddressMode, ErrorType, I2c, Operation};
use os_trait::{Mutex, OsInterface, prelude::*};

pub struct I2cMutexDevice<OS: OsInterface, BUS, A>
where
    OS: OsInterface,
    BUS: I2c<A>,
    A: AddressMode,
{
    i2c: Arc<Mutex<OS, BUS>>,
    _a: PhantomData<A>,
}

impl<OS, BUS, A> I2cMutexDevice<OS, BUS, A>
where
    OS: OsInterface,
    BUS: I2c<A>,
    A: AddressMode,
{
    pub fn new(_os: OS, i2c: BUS) -> Self {
        Self {
            i2c: Arc::new(Mutex::<OS, BUS>::new(i2c)),
            _a: PhantomData,
        }
    }
}

impl<OS, I2C, A> I2c<A> for I2cMutexDevice<OS, I2C, A>
where
    OS: OsInterface,
    I2C: I2c<A>,
    A: AddressMode,
{
    #[inline]
    fn transaction(
        &mut self,
        address: A,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        let mut i2c = self.i2c.lock();
        i2c.transaction(address, operations)
    }
}

impl<OS, BUS, A> ErrorType for I2cMutexDevice<OS, BUS, A>
where
    OS: OsInterface,
    BUS: I2c<A>,
    A: AddressMode,
{
    type Error = BUS::Error;
}

impl<OS, BUS, A> Clone for I2cMutexDevice<OS, BUS, A>
where
    OS: OsInterface,
    BUS: I2c<A>,
    A: AddressMode,
{
    fn clone(&self) -> Self {
        Self {
            i2c: Arc::clone(&self.i2c),
            _a: PhantomData,
        }
    }
}
