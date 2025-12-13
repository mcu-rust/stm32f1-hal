use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// with sequence id
    Start(u8),
    Addr,
    Data,
    Success,
    Stop,
}

impl From<Mode> for u16 {
    fn from(mode: Mode) -> Self {
        match mode {
            Mode::Start(id) => 1 | ((id as u16) << 8),
            Mode::Addr => 2,
            Mode::Data => 3,
            Mode::Success => 4,
            Mode::Stop => 0,
        }
    }
}

impl From<u16> for Mode {
    fn from(value: u16) -> Self {
        let mode = value as u8;
        let id = (value >> 8) as u8;
        match mode {
            1 => Self::Start(id),
            2 => Self::Addr,
            3 => Self::Data,
            4 => Self::Success,
            _ => Self::Stop,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// Start with sequence id
    Start(u8),
    SlaveAddr(u8),
    SlaveAddr10(u16),
    WriteMode,
    Data(u8),
    EndWrite,
    Len(u16),
}

pub fn err_to_int(err: Option<Error>) -> u16 {
    match err {
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

pub fn int_to_err(err: u16) -> Option<Error> {
    let nack = (err >> 8) as u8;
    let err = err as u8;

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

#[cfg(test)]
mod tests {
    use super::*;

    fn compare_mode(mode: Mode) {
        let i: u16 = mode.into();
        assert_eq!(mode, i.into());
    }

    #[test]
    fn teat_mode() {
        compare_mode(Mode::Start(12));
        compare_mode(Mode::Addr);
        compare_mode(Mode::Data);
        compare_mode(Mode::Success);
        compare_mode(Mode::Stop);
    }

    fn compare_error(err: Option<Error>) {
        let i: u16 = err_to_int(err);
        assert_eq!(err, int_to_err(i));
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
