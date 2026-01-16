//! # Direct Memory Access

pub use crate::common::dma::*;
pub type DmaPriority = pac::dma1::ch::cr::PL;

use crate::{Steal, common::wrap_trait::*, pac, rcc::Rcc};

pub trait DmaInit {
    type Channels;

    fn split(self, rcc: &mut Rcc) -> Self::Channels;
}

macro_rules! dma {
    ($DMAX:ty: ($dmaX:ident, {
        $($CX:ident: ($ch: literal),)+
    }),) => {
        pub mod $dmaX {
            use super::*;

            #[non_exhaustive]
            #[allow(clippy::manual_non_exhaustive)]
            pub struct Channels((), $(pub $CX),+);

            $(
                pub type $CX = super::Ch<$DMAX, $ch>;
            )+

            impl DmaInit for $DMAX {
                type Channels = Channels;

                fn split(self, rcc: &mut Rcc) -> Channels {
                    rcc.enable(&self);

                    // reset the DMA control registers (stops all on-going transfers)
                    $(
                        self.ch($ch).cr().reset();
                    )+

                    Channels((), $(Ch::<$DMAX, $ch>{ dma: unsafe { self.steal() }}),+)
                }
            }
        }
    }
}

dma! {
    pac::DMA1: (dma1, {
        C1: (0),
        C2: (1),
        C3: (2),
        C4: (3),
        C5: (4),
        C6: (5),
        C7: (6),
    }),
}

dma! {
    pac::DMA2: (dma2, {
        C1: (0),
        C2: (1),
        C3: (2),
        C4: (3),
        C5: (4),
    }),
}

wrap_trait_deref! {
    (pac::DMA1, pac::DMA2,),
    pub trait RegisterBlock {
        fn isr(&self) -> &pac::dma1::ISR;
        fn ifcr(&self) -> &pac::dma1::IFCR;
        fn ch(&self, n: usize) -> &pac::dma1::CH;
    }
}

// DMA Channel ----------------------------------------------------------------

pub struct Ch<DMA, const C: u8> {
    dma: DMA,
}

impl<DMA, const C: u8> Ch<DMA, C>
where
    DMA: RegisterBlock,
{
    #[inline]
    pub fn set_priority(&mut self, priority: DmaPriority) {
        self.ch().cr().modify(|_, w| w.pl().variant(priority));
    }

    #[inline(always)]
    fn ch(&self) -> &pac::dma1::CH {
        self.dma.ch(C as usize)
    }
}

impl<DMA, const C: u8> Steal for Ch<DMA, C>
where
    DMA: RegisterBlock + Steal,
{
    unsafe fn steal(&self) -> Self {
        unsafe {
            Self {
                dma: self.dma.steal(),
            }
        }
    }
}

impl<DMA, const C: u8> DmaChannel for Ch<DMA, C>
where
    DMA: RegisterBlock,
{
    #[inline]
    fn start(&mut self) {
        self.dma.ifcr().write(|w| w.cgif(C).set_bit());
        self.ch().cr().modify(|_, w| w.en().set_bit());
    }

    #[inline]
    fn stop(&mut self) {
        self.ch().cr().modify(|_, w| w.en().clear_bit());
        self.dma.ifcr().write(|w| w.cgif(C).set_bit());
        self.set_transfer_length(0);
    }

    #[inline]
    fn set_peripheral_address<T: Sized>(
        &mut self,
        address: usize,
        mem_to_periph: bool,
        increase: bool,
        circular: bool,
    ) {
        self.ch()
            .par()
            .write(|w| unsafe { w.pa().bits(address as u32) });
        self.ch().cr().modify(|_, w| {
            w.mem2mem().clear_bit();
            w.pinc().bit(increase);
            w.circ().bit(circular);
            w.dir().bit(mem_to_periph);

            match core::mem::size_of::<T>() {
                2 => {
                    w.msize().bits16();
                    w.psize().bits16()
                }
                4 => {
                    w.msize().bits32();
                    w.psize().bits32()
                }
                _ => {
                    w.msize().bits8();
                    w.psize().bits8()
                }
            }
        });
    }

    #[inline(always)]
    fn set_memory_address(&mut self, address: usize, increase: bool) {
        self.ch()
            .mar()
            .write(|w| unsafe { w.ma().bits(address as u32) });
        self.ch().cr().modify(|_, w| w.minc().bit(increase));
    }

    #[inline(always)]
    fn set_transfer_length(&mut self, len: usize) {
        self.ch()
            .ndtr()
            .write(|w| w.ndt().set(u16::try_from(len).unwrap()));
    }

    #[inline]
    fn set_memory_to_memory<T: Sized>(&mut self, _src_addr: usize, _dst_addr: usize, _len: usize) {
        todo!()
    }

    #[inline]
    fn get_unprocessed_len(&self) -> usize {
        self.ch().ndtr().read().bits() as usize
    }

    #[inline]
    fn in_progress(&self) -> bool {
        self.get_unprocessed_len() != 0 && self.dma.isr().read().tcif(C).bit_is_clear()
    }

    #[inline]
    fn set_interrupt(&mut self, event: DmaEvent, enable: bool) {
        match event {
            DmaEvent::HalfTransfer => self.ch().cr().modify(|_, w| w.htie().bit(enable)),
            DmaEvent::TransferComplete => self.ch().cr().modify(|_, w| w.tcie().bit(enable)),
        };
    }

    #[inline]
    fn check_and_clear_interrupt(&mut self, event: DmaEvent) -> bool {
        match event {
            DmaEvent::TransferComplete => {
                if self.dma.isr().read().tcif(C).bit_is_set() {
                    self.dma.ifcr().write(|w| w.ctcif(C).set_bit());
                    true
                } else {
                    false
                }
            }
            DmaEvent::HalfTransfer => {
                if self.dma.isr().read().htif(C).bit_is_set() {
                    self.dma.ifcr().write(|w| w.chtif(C).set_bit());
                    true
                } else {
                    false
                }
            }
        }
    }
}

pub trait DmaBindTx<U>: DmaChannel {}
pub trait DmaBindRx<U>: DmaChannel {}

// table
// Do NOT manually modify the code.
// It's generated by scripts/generate_dma_table.py from scripts/table/stm32f1_dma_table.csv

impl DmaBindTx<pac::USART3> for dma1::C2 {}
impl DmaBindRx<pac::USART3> for dma1::C3 {}
impl DmaBindTx<pac::USART1> for dma1::C4 {}
impl DmaBindRx<pac::USART1> for dma1::C5 {}
impl DmaBindRx<pac::USART2> for dma1::C6 {}
impl DmaBindTx<pac::USART2> for dma1::C7 {}
impl DmaBindRx<pac::UART4> for dma2::C3 {}
impl DmaBindTx<pac::UART4> for dma2::C5 {}

impl DmaBindRx<pac::SPI1> for dma1::C2 {}
impl DmaBindTx<pac::SPI1> for dma1::C3 {}
impl DmaBindRx<pac::SPI2> for dma1::C4 {}
impl DmaBindTx<pac::SPI2> for dma1::C5 {}
impl DmaBindRx<pac::SPI3> for dma2::C1 {}
impl DmaBindTx<pac::SPI3> for dma2::C2 {}

impl DmaBindTx<pac::I2C2> for dma1::C4 {}
impl DmaBindRx<pac::I2C2> for dma1::C5 {}
impl DmaBindTx<pac::I2C1> for dma1::C6 {}
impl DmaBindRx<pac::I2C1> for dma1::C7 {}
