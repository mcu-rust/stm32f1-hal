use crate::{Mcu, l, pac::Interrupt};
use alloc::boxed::Box;
use core::{
    cell::{Cell, UnsafeCell},
    mem::MaybeUninit,
};

pub struct Callback {
    callback: UnsafeCell<MaybeUninit<Box<dyn FnMut()>>>,
    once_flag: critical_section::Mutex<Cell<bool>>,
    it_line: Interrupt,
}

unsafe impl Sync for Callback {}

/// # Safety
///
/// Sharing it across multiple interrupt callbacks may lead to a data race.
impl Callback {
    pub const fn new(it_line: Interrupt) -> Self {
        Self {
            callback: UnsafeCell::new(MaybeUninit::uninit()),
            once_flag: critical_section::Mutex::new(Cell::new(true)),
            it_line,
        }
    }

    /// Register the callback, and enable the interrupt line in NVIC.
    /// You can call it only once.
    pub fn set(&self, mcu: &mut Mcu, callback: impl FnMut() + 'static) {
        let cb = Box::new(callback);
        critical_section::with(|cs| {
            l::assert!(self.once_flag.borrow(cs).get());
            let callback = unsafe { &mut *self.callback.get() };
            callback.write(cb);
            self.once_flag.borrow(cs).set(false);
        });
        mcu.nvic.enable(self.it_line, true);
    }

    /// # Safety
    ///
    /// This function must only be called from interrupt context.
    #[inline(always)]
    pub unsafe fn call(&self) {
        let cb = unsafe { (&mut *self.callback.get()).assume_init_mut() }.as_mut();
        (*cb)();
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
