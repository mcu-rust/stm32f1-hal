pub mod atomic_cell;
pub mod atomic_mutex;
pub mod bus_device;
pub mod dma;
pub mod i2c;
pub mod prelude;
pub mod ringbuf;
pub mod simplest_heap;
pub mod timer;
pub mod uart;
pub mod wrap_trait;

pub use critical_section;
pub use embedded_hal;
pub use embedded_hal_nb;
pub use embedded_io;
pub use fugit::{self, HertzU32, KilohertzU32, MicrosDurationU32};
pub use os_trait;
pub use rtrb;

use atomic_cell::AtomicCellMember;

impl AtomicCellMember for MicrosDurationU32 {
    fn to_num(self) -> usize {
        self.ticks() as usize
    }

    unsafe fn from_num(value: usize) -> Self {
        Self::from_ticks(value as u32)
    }
}

impl AtomicCellMember for KilohertzU32 {
    fn to_num(self) -> usize {
        self.raw() as usize
    }

    unsafe fn from_num(value: usize) -> Self {
        Self::from_raw(value as u32)
    }
}

impl AtomicCellMember for HertzU32 {
    fn to_num(self) -> usize {
        self.raw() as usize
    }

    unsafe fn from_num(value: usize) -> Self {
        Self::from_raw(value as u32)
    }
}
