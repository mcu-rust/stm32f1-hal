pub use crate::hal::{os_trait, prelude::*, raw_os::RawOs as OS};

pub type OsTimeout = os_trait::Timeout<OS>;
pub type OsDuration = os_trait::Duration<OS>;
