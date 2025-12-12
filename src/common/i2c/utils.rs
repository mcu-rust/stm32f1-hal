use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// with sequence id
    Start(u8),
    Work,
    Addr,
    Data,
    Success,
    Stop,
}

impl Into<u16> for Mode {
    fn into(self) -> u16 {
        match self {
            Mode::Start(id) => 1 | ((id as u16) << 8),
            Mode::Work => 2 as u16,
            Mode::Addr => 3 as u16,
            Mode::Data => 4 as u16,
            Mode::Success => 5 as u16,
            Mode::Stop => 0 as u16,
        }
    }
}

impl From<u16> for Mode {
    fn from(value: u16) -> Self {
        let mode = value as u8;
        let id = (value >> 8) as u8;
        match mode {
            1 => Self::Start(id),
            2 => Self::Work,
            3 => Self::Addr,
            4 => Self::Data,
            5 => Self::Success,
            _ => Self::Stop,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// Start with sequence id
    Start(u8),
    SlaveAddr(u8),
    WriteMode,
    Data(u8),
    ReadMode,
    Len(u8),
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
            Error::Other => 10,
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
        compare_mode(Mode::Work);
        compare_mode(Mode::Addr);
        compare_mode(Mode::Data);
        compare_mode(Mode::Success);
        compare_mode(Mode::Stop);
    }

    #[test]
    fn teat_error() {
        let err = Some(Error::SMBusAlert);
        let i: u16 = err_to_int(err);
        assert_eq!(err, int_to_err(i));

        let err = Some(Error::NoAcknowledge(NoAcknowledgeSource::Data));
        let i: u16 = err_to_int(err);
        assert_eq!(err, int_to_err(i));
    }
}
