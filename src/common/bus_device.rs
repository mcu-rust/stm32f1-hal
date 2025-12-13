pub trait BusDevice<WD: Word> {
    fn write_read(&mut self, write: &[&[WD]], read: &mut [&mut [WD]]) -> Result<(), BusError>;

    #[inline]
    fn read(&mut self, buf: &mut [WD]) -> Result<(), BusError> {
        self.write_read(&[&[]], &mut [buf])
    }

    #[inline]
    fn write(&mut self, buf: &[WD]) -> Result<(), BusError> {
        self.write_read(&[buf], &mut [&mut []])
    }
}

pub trait BusDeviceTransfer<WD: Word>: BusDevice<WD> {
    /// Read data into the first buffer, while writing data from the second buffer.
    fn transfer(&mut self, read: &mut [WD], write: &[WD]) -> Result<(), BusError>;
    /// Write data out while reading data into the provided buffer.
    fn transfer_in_place(&mut self, buf: &mut [WD]) -> Result<(), BusError>;
}

pub use super::i2c::Address;
pub trait BusDeviceWithAddress<WD: Word>: BusDevice<WD> {
    fn set_address(&mut self, address: Address);
}

pub trait Word: Copy + 'static {}
impl Word for u8 {}
impl Word for u16 {}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BusError {
    Busy,
    ArbitrationLoss,
    NoAcknowledge,
    Timeout,
    Other,
}
