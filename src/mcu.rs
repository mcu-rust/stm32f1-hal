use super::*;

impl<RB, const A: usize> Steal for stm32f1::Periph<RB, A> {
    unsafe fn steal(&self) -> Self {
        unsafe { Self::steal() }
    }
}

pub struct Mcu {
    // pub apb1: APB1,
    // pub apb2: APB2,
    // pub flash: pac::flash::Parts,
    pub exti: pac::EXTI,
    pub scb: nvic_scb::Scb,
    pub nvic: nvic_scb::Nvic,
    pub rcc: rcc::Rcc,
    pub afio: afio::Afio,
}

impl Mcu {
    pub fn new(
        rcc: rcc::Rcc,
        afio: afio::Afio,
        scb: nvic_scb::Scb,
        nvic: nvic_scb::Nvic,
        exti: pac::EXTI,
    ) -> Self {
        Self {
            rcc,
            afio,
            scb,
            nvic,
            exti,
        }
    }
}
