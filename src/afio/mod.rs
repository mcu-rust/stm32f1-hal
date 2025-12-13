//! # Alternate Function I/Os

pub mod i2c_remap;
pub mod timer_remap;
pub mod uart_remap;

use crate::gpio::{self, Debugger};
use crate::pac::{AFIO, afio};
use crate::rcc::Rcc;
use core::marker::PhantomData;

pub trait AfioInit {
    fn init(self, rcc: &mut Rcc) -> Afio;
}

impl AfioInit for AFIO {
    fn init(self, rcc: &mut Rcc) -> Afio {
        rcc.enable(&self);
        rcc.reset(&self);

        Afio {
            _reg: self,
            evcr: EVCR,
            mapr: MAPR { jtag_enabled: true },
            exticr1: EXTICR1,
            exticr2: EXTICR2,
            exticr3: EXTICR3,
            exticr4: EXTICR4,
            mapr2: MAPR2,
        }
    }
}

/// HAL wrapper around the AFIO registers
///
/// Aquired by calling [init](trait.AfioInit.html#init) on the [AFIO
/// registers](../pac/struct.AFIO.html)
///
/// ```rust
/// let p = pac::Peripherals::take().unwrap();
/// let mut rcc = p.RCC.init();
/// let mut afio = p.AFIO.init();
pub struct Afio {
    _reg: AFIO,
    pub evcr: EVCR,
    pub mapr: MAPR,
    pub exticr1: EXTICR1,
    pub exticr2: EXTICR2,
    pub exticr3: EXTICR3,
    pub exticr4: EXTICR4,
    pub mapr2: MAPR2,
}

#[non_exhaustive]
pub struct EVCR;

impl EVCR {
    pub fn evcr(&mut self) -> &afio::EVCR {
        unsafe { (*AFIO::ptr()).evcr() }
    }
}

// Remap Mode
pub trait RemapMode<REG> {
    fn remap(afio: &mut Afio);
}
pub struct RemapDefault<REG>(PhantomData<REG>);
pub struct RemapPartial1<REG>(PhantomData<REG>);
pub struct RemapPartial2<REG>(PhantomData<REG>);
pub struct RemapFull<REG>(PhantomData<REG>);
pub struct NonePin {}
pub const NONE_PIN: NonePin = NonePin {};

/// AF remap and debug I/O configuration register (MAPR)
///
/// Aquired through the [Afio](struct.Afio.html) struct.
///
/// ```rust
/// let dp = pac::Peripherals::take().unwrap();
/// let mut rcc = dp.RCC.init();
/// let mut afio = dp.AFIO.init();
/// function_using_mapr(&mut afio.mapr);
/// ```
#[non_exhaustive]
pub struct MAPR {
    jtag_enabled: bool,
}

impl MAPR {
    fn mapr(&mut self) -> &afio::MAPR {
        unsafe { (*AFIO::ptr()).mapr() }
    }

    pub fn modify_mapr<F>(&mut self, mod_fn: F)
    where
        F: for<'w> FnOnce(&afio::mapr::R, &'w mut afio::mapr::W) -> &'w mut afio::mapr::W,
    {
        let debug_bits = if self.jtag_enabled { 0b000 } else { 0b010 };
        self.mapr()
            .modify(unsafe { |r, w| mod_fn(r, w).swj_cfg().bits(debug_bits) });
    }

    /// Disables the JTAG to free up pa15, pb3 and pb4 for normal use
    #[allow(clippy::redundant_field_names, clippy::type_complexity)]
    pub fn disable_jtag(
        &mut self,
        pa15: gpio::PA15<Debugger>,
        pb3: gpio::PB3<Debugger>,
        pb4: gpio::PB4<Debugger>,
    ) -> (gpio::PA15, gpio::PB3, gpio::PB4) {
        self.jtag_enabled = false;
        // Avoid duplicating swj_cfg write code
        self.modify_mapr(|_, w| w);

        // NOTE(unsafe) The pins are now in the good state.
        unsafe { (pa15.activate(), pb3.activate(), pb4.activate()) }
    }
}

#[non_exhaustive]
pub struct EXTICR1;

impl EXTICR1 {
    pub fn exticr1(&mut self) -> &afio::EXTICR1 {
        unsafe { (*AFIO::ptr()).exticr1() }
    }
}

#[non_exhaustive]
pub struct EXTICR2;

impl EXTICR2 {
    pub fn exticr2(&mut self) -> &afio::EXTICR2 {
        unsafe { (*AFIO::ptr()).exticr2() }
    }
}

#[non_exhaustive]
pub struct EXTICR3;

impl EXTICR3 {
    pub fn exticr3(&mut self) -> &afio::EXTICR3 {
        unsafe { (*AFIO::ptr()).exticr3() }
    }
}

#[non_exhaustive]
pub struct EXTICR4;

impl EXTICR4 {
    pub fn exticr4(&mut self) -> &afio::EXTICR4 {
        unsafe { (*AFIO::ptr()).exticr4() }
    }
}

#[non_exhaustive]
pub struct MAPR2;

impl MAPR2 {
    pub fn mapr2(&mut self) -> &afio::MAPR2 {
        unsafe { (*AFIO::ptr()).mapr2() }
    }

    pub fn modify_mapr<F>(&mut self, mod_fn: F)
    where
        F: for<'w> FnOnce(&afio::mapr2::R, &'w mut afio::mapr2::W) -> &'w mut afio::mapr2::W,
    {
        self.mapr2().modify(|r, w| mod_fn(r, w));
    }
}
