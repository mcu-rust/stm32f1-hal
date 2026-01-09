use super::*;
use crate::common::{atomic_cell::AtomicCellMember, ringbuf::PushError};

pub(crate) trait SpiBusInterface<WD: Word> {
    fn transaction(&mut self, operations: &mut [Operation<'_, WD>]) -> Result<(), Error>;
    // TODO config speed and phase
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TxCommand<WD: Word> {
    Write(*const WD, usize),
    Dummy(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RxCommand<WD: Word> {
    Read(*mut WD, usize),
    Dummy(usize),
}

impl<T> From<PushError<T>> for Error {
    fn from(_value: PushError<T>) -> Self {
        Self::Buffer
    }
}

impl AtomicCellMember for Option<Error> {
    #[inline]
    fn to_num(self) -> usize {
        match self {
            None => 0,
            Some(err) => err as usize + 1,
        }
    }

    #[inline]
    unsafe fn from_num(val: usize) -> Self {
        if val == 0 {
            None
        } else {
            Some(unsafe { core::mem::transmute((val - 1) as u8) })
        }
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
        compare_error(Some(Error::Busy));
        compare_error(Some(Error::Overrun));
        compare_error(Some(Error::Timeout));
        compare_error(Some(Error::Crc));
        compare_error(Some(Error::Buffer));
        compare_error(Some(Error::Other));
    }
}
