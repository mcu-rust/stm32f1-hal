use crate::{Mcu, pac::Interrupt};
use alloc::boxed::Box;
use core::cell::{Cell, OnceCell};

pub struct Callback {
    callback: OnceCell<Cell<Box<dyn FnMut()>>>,
    it_line: Interrupt,
}

unsafe impl Sync for Callback {}

/// # Safety
///
/// Sharing it across multiple interrupt callbacks may lead to a data race.
impl Callback {
    pub const fn new(it_line: Interrupt) -> Self {
        Self {
            callback: OnceCell::new(),
            it_line,
        }
    }

    /// Register the callback, and enable the interrupt line in NVIC.
    /// You can call it only once.
    pub fn set(&self, mcu: &mut Mcu, callback: impl FnMut() + 'static) {
        let cb = Cell::new(Box::new(callback));
        critical_section::with(|_| {
            assert!(self.callback.set(cb).is_ok());
        });
        mcu.nvic.enable(self.it_line, true);
    }

    /// # Safety
    ///
    /// Only call this in interrupt
    pub unsafe fn call(&self) {
        if let Some(cb) = self.callback.get() {
            unsafe { (*cb.as_ptr())() }
        }
    }
}

#[macro_export]
macro_rules! interrupt_handler {
    ($(
        ($LINE:ident, $CALLBACK:ident),
    )+) => {
        use $crate::pac::interrupt;
        $(
            pub static $CALLBACK: $crate::interrupt::Callback =
                $crate::interrupt::Callback::new($crate::pac::Interrupt::$LINE);

            #[allow(non_snake_case)]
            #[interrupt]
            fn $LINE() {
                unsafe { $CALLBACK.call() }
            }
        )+
    };
}
