pub use crate::hal::{os_trait, prelude::*, raw_os::RawOs as OS};

use os_trait::os_type_alias;
os_type_alias!(OS);
