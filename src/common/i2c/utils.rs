use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// with sequence id
    StartWrite(u8),
    /// with sequence id
    StartRead(u8),
    Write,
    WriteAddr,
    WriteData,
    Read,
    ReadAddr,
    ReadData,
    Stop,
}

impl Into<u16> for Mode {
    fn into(self) -> u16 {
        match self {
            Mode::StartWrite(id) => 1 | ((id as u16) << 8),
            Mode::StartRead(id) => 2 | ((id as u16) << 8),
            Mode::Write => 3 as u16,
            Mode::Read => 4 as u16,
            Mode::WriteAddr => 5 as u16,
            Mode::ReadAddr => 6 as u16,
            Mode::WriteData => 7 as u16,
            Mode::ReadData => 8 as u16,
            Mode::Stop => 0 as u16,
        }
    }
}

impl From<u16> for Mode {
    fn from(value: u16) -> Self {
        let mode = value as u8;
        let id = (value >> 8) as u8;
        match mode {
            1 => Self::StartWrite(id),
            2 => Self::StartRead(id),
            3 => Self::Write,
            4 => Self::Read,
            5 => Self::WriteAddr,
            6 => Self::ReadAddr,
            7 => Self::WriteData,
            8 => Self::ReadData,
            _ => Self::Stop,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// Start with sequence id
    Start(u8),
    SlaveAddr(u8),
    Data(u8),
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

    #[test]
    fn teat_mode() {
        let mode = Mode::StartWrite(12);
        let i: u16 = mode.into();
        assert_eq!(mode, i.into());

        let mode = Mode::StartRead(55);
        let i: u16 = mode.into();
        assert_eq!(mode, i.into());

        let mode = Mode::Write;
        let i: u16 = mode.into();
        assert_eq!(mode, i.into());

        let mode = Mode::Read;
        let i: u16 = mode.into();
        assert_eq!(mode, i.into());

        let mode = Mode::Stop;
        let i: u16 = mode.into();
        assert_eq!(mode, i.into());

        let mode = Mode::WriteData;
        let i: u16 = mode.into();
        assert_eq!(mode, i.into());

        let mode = Mode::ReadData;
        let i: u16 = mode.into();
        assert_eq!(mode, i.into());

        let mode = Mode::WriteAddr;
        let i: u16 = mode.into();
        assert_eq!(mode, i.into());

        let mode = Mode::ReadAddr;
        let i: u16 = mode.into();
        assert_eq!(mode, i.into());
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
