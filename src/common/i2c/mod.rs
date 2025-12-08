pub mod i2c_master_it;
mod utils;

use embedded_hal::i2c::ErrorKind;
pub use embedded_hal::i2c::NoAcknowledgeSource;

pub trait I2cPeriph {
    fn it_reset(&mut self);
    fn it_send_start(&mut self);
    fn it_send_slave_addr(&mut self, slave_addr: u8, read: bool) -> bool;
    fn it_start_write_data(&mut self) -> bool;
    fn it_start_read_data(&mut self, total_len: usize) -> bool;
    fn it_write(&mut self, data: u8) -> bool;
    /// # Returns
    /// - `None`: need to wait
    /// - `Some(true)`: Wrote a data
    /// - `Some(false)`: No new data
    fn it_write_with(&mut self, f: impl FnOnce() -> Option<u8>) -> Option<bool>;
    fn it_read(&mut self, left_len: usize) -> Option<u8>;

    fn send_stop(&mut self);
    fn is_busy(&self) -> bool;
    /// Read and clean the flag
    fn get_and_clean_error(&mut self) -> Option<Error>;

    fn get_flag(&mut self, flag: Flag) -> bool;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Flag {
    /// Start condition generated
    Started,
    /// Stop detection
    Stopped,
    /// Address is sent in master mode or received and matches in slave mode
    AddressSent,
    /// Byte transfer finished
    ByteTransferFinished,
    /// 10-bit header sent
    Address10,
    /// Data register not empty
    RxNotEmpty,
    /// Data register empty
    TxEmpty,
    /// SMBus alert
    MasterSlave,
    /// Master/Slave
    Transmitter,
    /// General call address (Slave mode)
    GeneralCall,
    /// SMBus device default address (Slave mode)
    SMBusDefault,
    /// SMBus host header (Slave mode)
    SMBusHost,
    /// Dual flag (Slave mode)
    Dual,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Address {
    Seven(u8),
    Ten(u16),
}

impl From<u8> for Address {
    fn from(value: u8) -> Self {
        Self::Seven(value)
    }
}

impl From<u16> for Address {
    fn from(value: u16) -> Self {
        Self::Ten(value)
    }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
#[non_exhaustive]
pub enum Error {
    /// Overrun/underrun
    Overrun,
    /// No ack received
    NoAcknowledge(NoAcknowledgeSource),
    Timeout,
    /// Bus error
    Bus,
    Crc,
    /// Arbitration was lost
    ArbitrationLoss,
    /// SMBus alert
    SMBusAlert,
    /// SMBus PEC Error in reception
    Pec,
    /// SMBus timeout
    SMBusTimeout,
    Other,
}

impl Error {
    pub(crate) fn nack_addr(self) -> Self {
        match self {
            Self::NoAcknowledge(NoAcknowledgeSource::Unknown) => {
                Self::NoAcknowledge(NoAcknowledgeSource::Address)
            }
            e => e,
        }
    }
    pub(crate) fn nack_data(self) -> Self {
        match self {
            Self::NoAcknowledge(NoAcknowledgeSource::Unknown) => {
                Self::NoAcknowledge(NoAcknowledgeSource::Data)
            }
            e => e,
        }
    }
}

impl embedded_hal::i2c::Error for Error {
    fn kind(&self) -> ErrorKind {
        match *self {
            Self::Overrun => ErrorKind::Overrun,
            Self::Bus => ErrorKind::Bus,
            Self::ArbitrationLoss => ErrorKind::ArbitrationLoss,
            Self::NoAcknowledge(nack) => ErrorKind::NoAcknowledge(nack),
            Self::Crc
            | Self::Timeout
            | Self::SMBusAlert
            | Self::SMBusTimeout
            | Self::Pec
            | Self::Other => ErrorKind::Other,
        }
    }
}
