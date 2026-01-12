use super::*;
use crate::common::{atomic_cell::AtomicCellMember, ringbuf::PushError};

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
            Some(unsafe { core::mem::transmute::<u8, Error>((val - 1) as u8) })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fugit::{HertzU32, KilohertzU32, RateExtU32};

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

    #[test]
    fn hertz() {
        let a: HertzU32 = 20.kHz();
        let b: KilohertzU32 = 2.kHz();
        assert_eq!(a.raw(), 20_000);
        assert_eq!(b.raw(), 2);
        assert_eq!(a / b, 10);
    }
}
