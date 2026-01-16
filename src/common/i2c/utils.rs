use super::*;
use crate::common::{atomic_cell::AtomicCellMember, ringbuf::PushError};

#[maybe_derive_format]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Work {
    Start = 0,
    Addr,
    Data,
    Success,
    Stop,
}

impl AtomicCellMember for Work {
    #[inline]
    fn to_num(self) -> usize {
        self as usize
    }

    #[inline]
    unsafe fn from_num(val: usize) -> Self {
        unsafe { core::mem::transmute(val as u8) }
    }
}

#[maybe_derive_format]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Command {
    SlaveAddr(u8),
    SlaveAddr10(u16),
    Write(*const u8, usize),
    WriteEnd,
    /// With length
    Read(usize),
    ReadBuf(*mut u8, usize),
}

impl AtomicCellMember for Option<Error> {
    #[inline]
    fn to_num(self) -> usize {
        match self {
            None => 0,
            Some(err) => match err {
                Error::ArbitrationLoss => 1,
                Error::Bus => 2,
                Error::Crc => 3,
                Error::NoAcknowledge(nack) => match nack {
                    NoAcknowledgeSource::Unknown => 4,
                    NoAcknowledgeSource::Address => 4 | (1 << 8),
                    NoAcknowledgeSource::Data => 4 | (2 << 8),
                },
                Error::Overrun => 5,
                Error::Pec => 6,
                Error::SMBusAlert => 7,
                Error::Timeout => 8,
                Error::SMBusTimeout => 9,
                Error::Busy => 10,
                Error::Buffer => 11,
                Error::Other => 12,
            },
        }
    }

    #[inline]
    unsafe fn from_num(val: usize) -> Self {
        let nack = (val >> 8) as u8;
        let err = val as u8;

        if err == 0 {
            None
        } else {
            Some(match err {
                1 => Error::ArbitrationLoss,
                2 => Error::Bus,
                3 => Error::Crc,
                4 => match nack {
                    0 => Error::NoAcknowledge(NoAcknowledgeSource::Unknown),
                    1 => Error::NoAcknowledge(NoAcknowledgeSource::Address),
                    _ => Error::NoAcknowledge(NoAcknowledgeSource::Data),
                },
                5 => Error::Overrun,
                6 => Error::Pec,
                7 => Error::SMBusAlert,
                8 => Error::Timeout,
                9 => Error::SMBusTimeout,
                10 => Error::Busy,
                11 => Error::Buffer,
                _ => Error::Other,
            })
        }
    }
}

impl<T> From<PushError<T>> for Error {
    fn from(_value: PushError<T>) -> Self {
        Self::Buffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compare_error(err: Option<Error>) {
        let i: usize = err.to_num();
        assert_eq!(err, unsafe { Option::<Error>::from_num(i) });
    }

    #[test]
    fn teat_error() {
        compare_error(None);
        compare_error(Some(Error::NoAcknowledge(NoAcknowledgeSource::Unknown)));
        compare_error(Some(Error::NoAcknowledge(NoAcknowledgeSource::Address)));
        compare_error(Some(Error::NoAcknowledge(NoAcknowledgeSource::Data)));
        compare_error(Some(Error::SMBusAlert));
        compare_error(Some(Error::Busy));
        compare_error(Some(Error::Overrun));
        compare_error(Some(Error::Timeout));
        compare_error(Some(Error::Bus));
        compare_error(Some(Error::Crc));
        compare_error(Some(Error::ArbitrationLoss));
        compare_error(Some(Error::Pec));
        compare_error(Some(Error::SMBusTimeout));
        compare_error(Some(Error::Buffer));
        compare_error(Some(Error::Other));
    }
}
