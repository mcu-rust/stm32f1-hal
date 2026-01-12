mod utils;

pub mod bus_it;
pub mod device;

pub use crate::fugit::{HertzU32, KilohertzU32};
pub use embedded_hal::spi::{Mode, Phase, Polarity};

use crate::common::prelude::*;
use embedded_hal::spi::{ErrorKind, ErrorType, Operation};

pub trait SpiPeriph<WD: Word> {
    /// master mode only
    fn config(&mut self, mode: Mode, freq: KilohertzU32);

    fn is_tx_empty(&self) -> bool;
    fn uncheck_write(&mut self, data: WD);
    fn read(&mut self) -> Option<WD>;
    fn is_busy(&self) -> bool;
    fn get_and_clean_error(&mut self) -> Option<Error>;

    fn set_interrupt(&mut self, event: Event, enable: bool);
    /// Disable all interrupt
    fn disable_all_interrupt(&mut self);
}

pub trait SpiBusInterface<WD: Word> {
    fn transaction(&mut self, operations: &mut [Operation<'_, WD>]) -> Result<(), Error>;
    // TODO config speed and phase
}

pub trait Word: Copy + Default + 'static {}
impl Word for u8 {}
impl Word for u16 {}
impl Word for u32 {}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    TxEmpty,
    RxNotEmpty,
    Error,
}

/// SPI error
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// Overrun occurred
    Overrun,
    /// Underrun occurred. I2S only.
    Underrun,
    /// Mode fault occurred
    ModeFault,
    /// CRC error
    Crc,
    ChipSelectFault,
    Busy,
    Buffer,
    Timeout,
    Other,
}

impl embedded_hal::spi::Error for Error {
    fn kind(&self) -> ErrorKind {
        match *self {
            Self::Overrun => ErrorKind::Overrun,
            Self::ModeFault => ErrorKind::ModeFault,
            Self::Crc => ErrorKind::FrameFormat,
            Self::ChipSelectFault => ErrorKind::ChipSelectFault,
            Self::Busy | Self::Buffer | Self::Underrun | Self::Other | Self::Timeout => {
                ErrorKind::Other
            }
        }
    }
}
