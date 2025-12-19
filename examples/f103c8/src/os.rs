pub use crate::hal::{prelude::*, raw_os::RawOs as OS};

pub type OsTimeoutState = <OS as OsInterface>::TimeoutState;
