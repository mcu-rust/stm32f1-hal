pub trait BusDevice<WD: Word> {
    fn write_read(&mut self, write: &[&[WD]], read: &mut [&mut [WD]]) -> Result<(), BusError>;

    /// Write out the data in buffer, and read exactly same length of data back to the same buffer.
    fn write_read_in_place(&mut self, buf: &mut [WD]) -> Result<(), BusError>;

    #[inline]
    fn read(&mut self, buf: &mut [WD]) -> Result<(), BusError> {
        self.write_read(&[&[]], &mut [buf])
    }

    #[inline]
    fn write(&mut self, buf: &[WD]) -> Result<(), BusError> {
        self.write_read(&[buf], &mut [&mut []])
    }
}

pub trait BusDeviceWithAddress<WD: Word>: BusDevice<WD> {
    fn set_address(&mut self, address: u16);
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
