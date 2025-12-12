pub mod atomic_mutex;
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
pub use fugit::{self, MicrosDurationU32};
pub use os_trait;
pub use rtrb;

use core::cell::UnsafeCell;

trait UnsafeCellMut<T> {
    unsafe fn unsafe_get_mut(&self) -> &mut T;
}

impl<T> UnsafeCellMut<T> for UnsafeCell<T> {
    #[inline]
    unsafe fn unsafe_get_mut(&self) -> &mut T {
        unsafe { &mut *self.get() }
    }
}
