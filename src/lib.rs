#![cfg_attr(not(feature = "std"), no_std)]

cfg_if::cfg_if! {
    if #[cfg(feature = "mcu")] {
        extern crate alloc;

        pub mod afio;
        pub mod backup_domain;
        pub mod bb;
        pub mod dma;
        pub mod flash;
        pub mod gpio;
        pub mod interrupt;
        pub mod nvic_scb;
        pub mod prelude;
        pub mod rcc;
        pub mod time;
        pub mod timer;
        pub mod uart;
        pub mod mcu;
        pub use mcu::Mcu;
        pub use cortex_m;
        pub use cortex_m_rt;
        pub mod i2c;
        pub mod raw_os;
    }
}

pub mod common;

pub use common::ringbuf;
pub use common::simplest_heap::Heap;
pub use critical_section;
pub use fugit;
pub use os_trait;

pub use embedded_hal;
pub use embedded_io;
pub use nb;
#[cfg(feature = "stm32f100")]
pub use stm32f1::stm32f100 as pac;
#[cfg(feature = "stm32f101")]
pub use stm32f1::stm32f101 as pac;
#[cfg(feature = "stm32f103")]
pub use stm32f1::stm32f103 as pac;
#[cfg(any(feature = "stm32f105", feature = "stm32f107"))]
pub use stm32f1::stm32f107 as pac;

pub trait Steal {
    /// Steal an instance of this peripheral
    ///
    /// # Safety
    ///
    /// Ensure that the new instance of the peripheral cannot be used in a way
    /// that may race with any existing instances, for example by only
    /// accessing read-only or write-only registers, or by consuming the
    /// original peripheral and using critical sections to coordinate
    /// access between multiple new instances.
    ///
    /// Additionally the HAL may rely on only one
    /// peripheral instance existing to ensure memory safety; ensure
    /// no stolen instances are passed to such software.
    unsafe fn steal(&self) -> Self;
}
