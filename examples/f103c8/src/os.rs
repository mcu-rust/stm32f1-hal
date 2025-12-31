pub use stm32f1_hal::{self as hal, os_trait, prelude::*, raw_os::RawOs as OS};

use os_trait::os_type_alias;
os_type_alias!(OS);
