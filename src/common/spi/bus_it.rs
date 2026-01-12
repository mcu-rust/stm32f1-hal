use super::{utils::*, *};
use crate::{
    Steal,
    common::{
        atomic_cell::{AtomicCell, AtomicCellMember, Ordering},
        ringbuf::{Consumer, Producer, RingBuffer},
    },
    fugit::{KilohertzU32, NanosDurationU32},
    os_trait::Duration,
};
use core::{
    cell::UnsafeCell,
    marker::PhantomData,
    slice::{self, Iter},
};

pub struct SpiBus<OS: OsInterface, SPI, WD: Word> {
    spi: SPI,
    work: Arc<AtomicCell<Work>>,
    err_code: Arc<AtomicCell<Option<Error>>>,
    tx_cmd_w: Producer<TxCommand<WD>>,
    rx_cmd_w: Producer<RxCommand<WD>>,
    tx_cmd_r: Arc<UnsafeCell<Consumer<TxCommand<WD>>>>,
    rx_cmd_r: Arc<UnsafeCell<Consumer<RxCommand<WD>>>>,
    waiter: OS::NotifyWaiter,
    byte_period: NanosDurationU32,
}

impl<OS, SPI, WD> SpiBus<OS, SPI, WD>
where
    OS: OsInterface,
    WD: Word,
    SPI: SpiPeriph<WD> + Steal,
{
    pub fn new(
        mut spi: SPI,
        speed: KilohertzU32,
        max_operation: usize,
    ) -> (
        Self,
        InterruptHandler<OS, SPI, WD>,
        ErrorInterruptHandler<OS, SPI, WD>,
    ) {
        spi.disable_all_interrupt();
        let work = Arc::new(AtomicCell::new(Work::Stop));
        let err_code = Arc::new(AtomicCell::new(None));
        let (notifier, waiter) = OS::notify();
        let (tx_cmd_w, tx_cmd_r) = RingBuffer::<TxCommand<WD>>::new(max_operation);
        let tx_cmd_r = Arc::new(UnsafeCell::new(tx_cmd_r));
        let (rx_cmd_w, rx_cmd_r) = RingBuffer::<RxCommand<WD>>::new(max_operation);
        let rx_cmd_r = Arc::new(UnsafeCell::new(rx_cmd_r));
        let byte_period = (speed.into_duration() as NanosDurationU32) * 10;
        let spi2 = unsafe { spi.steal() };
        let spi3 = unsafe { spi.steal() };
        (
            Self {
                spi,
                work: Arc::clone(&work),
                err_code: Arc::clone(&err_code),
                tx_cmd_w,
                rx_cmd_w,
                tx_cmd_r: Arc::clone(&tx_cmd_r),
                rx_cmd_r: Arc::clone(&rx_cmd_r),
                waiter,
                byte_period,
            },
            InterruptHandler {
                spi: spi2,
                work,
                tx_cmd_r,
                rx_cmd_r,
                notifier: notifier.clone(),
                tx_cache: TxCache::Dummy(0),
                rx_cache: RxCache::Dummy(0),
                rx_i: 0,
            },
            ErrorInterruptHandler {
                spi: spi3,
                err_code,
                notifier,
                _wd: PhantomData,
            },
        )
    }

    fn push_write_data(&mut self, data: &[WD]) -> Result<usize, Error> {
        if data.is_empty() {
            return Err(Error::Buffer);
        }

        self.tx_cmd_w
            .push(TxCommand::Write(data.as_ptr(), data.len()))?;
        self.rx_cmd_w.push(RxCommand::Dummy(data.len()))?;
        Ok(data.len())
    }

    fn push_read_buf(&mut self, buf: &mut [WD]) -> Result<usize, Error> {
        if buf.is_empty() {
            return Err(Error::Buffer);
        }

        self.tx_cmd_w.push(TxCommand::Dummy(buf.len()))?;
        self.rx_cmd_w
            .push(RxCommand::Read(buf.as_mut_ptr(), buf.len()))?;
        Ok(buf.len())
    }

    fn push_transfer(&mut self, buf: &mut [WD], data: &[WD]) -> Result<usize, Error> {
        let max_len = buf.len().max(data.len());
        if max_len == 0 {
            return Err(Error::Buffer);
        }

        if !data.is_empty() {
            self.tx_cmd_w
                .push(TxCommand::Write(data.as_ptr(), data.len()))?;
        }
        if !buf.is_empty() {
            self.rx_cmd_w
                .push(RxCommand::Read(buf.as_mut_ptr(), buf.len()))?;
        }
        if max_len > data.len() {
            self.tx_cmd_w.push(TxCommand::Dummy(max_len - data.len()))?;
        } else if max_len > buf.len() {
            self.rx_cmd_w.push(RxCommand::Dummy(max_len - buf.len()))?;
        }
        Ok(max_len)
    }

    fn push_transfer_in_place(&mut self, buf: &mut [WD]) -> Result<usize, Error> {
        if buf.is_empty() {
            return Err(Error::Buffer);
        }

        self.tx_cmd_w
            .push(TxCommand::Write(buf.as_ptr(), buf.len()))?;
        self.rx_cmd_w
            .push(RxCommand::Read(buf.as_mut_ptr(), buf.len()))?;
        Ok(buf.len())
    }

    fn communicate(&mut self, data_len: usize) -> Result<(), Error> {
        if data_len == 0 {
            return Ok(());
        }

        let timeout_ns = (data_len as u32 + 2) * self.byte_period.ticks();
        self.work.store(Work::Start, Ordering::Release);
        self.spi.set_interrupt(Event::TxEmpty, true);

        let rst: Result<(), Error> = self
            .waiter
            .wait_with(&Duration::<OS>::nanos(timeout_ns), 1, || {
                let work = self.work.load(Ordering::Acquire);
                let err_code = self.err_code.load(Ordering::Acquire);
                if Work::Success == work {
                    return Some(Ok(()));
                } else if Work::Stop == work {
                    return Some(Err(Error::Other));
                } else if let Some(err) = err_code {
                    return Some(Err(err));
                }
                None
            })
            .unwrap_or(Err(Error::Timeout));

        self.spi.disable_all_interrupt();

        rst
    }

    fn inner_transaction(&mut self, operations: &mut [Operation<'_, WD>]) -> Result<(), Error> {
        if operations.is_empty() {
            return Ok(());
        }

        if self.spi.is_busy() {
            return Err(Error::Busy);
        }

        // TODO: clear all errors
        while self.spi.read().is_some() {}

        // clean old commands
        let cmd = unsafe { &mut *self.tx_cmd_r.get() };
        while cmd.pop().is_ok() {}
        let cmd = unsafe { &mut *self.rx_cmd_r.get() };
        while cmd.pop().is_ok() {}

        let mut data_len = 0;
        let op_rst: Result<(), Error> = operations.iter_mut().try_for_each(|op| {
            match op {
                Operation::Write(data) => data_len += self.push_write_data(data)?,
                Operation::Read(buf) => data_len += self.push_read_buf(buf)?,
                Operation::Transfer(buf, data) => data_len += self.push_transfer(buf, data)?,
                Operation::TransferInPlace(buf) => data_len += self.push_transfer_in_place(buf)?,
                Operation::DelayNs(ns) => {
                    self.communicate(data_len)?;
                    OS::delay().delay_ns(*ns);
                    data_len = 0;
                }
            }
            Ok(())
        });
        let rst = op_rst.and_then(|_| self.communicate(data_len));
        // TODO error handler
        rst
    }
}

impl<OS, SPI, WD> SpiBusInterface<WD> for SpiBus<OS, SPI, WD>
where
    OS: OsInterface,
    WD: Word,
    SPI: SpiPeriph<WD> + Steal,
{
    #[inline]
    fn transaction(&mut self, operations: &mut [Operation<'_, WD>]) -> Result<(), Error> {
        self.inner_transaction(operations)
    }
}

// Interrupt Handler ------------------------------------------------

pub struct InterruptHandler<OS: OsInterface, SPI, WD: Word> {
    spi: SPI,
    work: Arc<AtomicCell<Work>>,
    tx_cmd_r: Arc<UnsafeCell<Consumer<TxCommand<WD>>>>,
    rx_cmd_r: Arc<UnsafeCell<Consumer<RxCommand<WD>>>>,
    notifier: OS::Notifier,

    tx_cache: TxCache<WD>,
    rx_cache: RxCache<WD>,
    rx_i: usize,
}

impl<OS, SPI, WD> InterruptHandler<OS, SPI, WD>
where
    OS: OsInterface,
    WD: Word,
    SPI: SpiPeriph<WD> + Steal,
{
    pub fn handler(&mut self) {
        if let Work::Start = self.work.load(Ordering::Acquire) {
            self.work.store(Work::Work, Ordering::Relaxed);
            self.tx_cache = TxCache::Dummy(0);
            self.pop_rx_cache();
        }

        while self.spi.is_tx_empty() {
            if let Some(data) = self.load_data() {
                self.spi.uncheck_write(data);
            } else {
                self.spi.set_interrupt(Event::RxNotEmpty, true);
                self.spi.set_interrupt(Event::TxEmpty, false);
                break;
            }
        }

        if let Some(data) = self.spi.read() {
            if !self.store_data(data) {
                self.spi.disable_all_interrupt();
                self.work.store(Work::Success, Ordering::Release);
                self.notifier.notify();
            }
        }
    }

    #[inline]
    fn load_data(&mut self) -> Option<WD> {
        match &mut self.tx_cache {
            TxCache::Write(iter) => {
                if let Some(d) = iter.next() {
                    return Some(*d);
                }
            }
            TxCache::Dummy(i) => {
                if *i > 0 {
                    *i -= 1;
                    return Some(WD::default());
                }
            }
        }

        if let Ok(cmd) = unsafe { &mut *self.tx_cmd_r.get() }.pop() {
            match cmd {
                TxCommand::Write(p, l) => {
                    let mut iter = unsafe { slice::from_raw_parts(p, l) }.iter();
                    let d = iter.next().copied();
                    self.tx_cache = TxCache::Write(iter);
                    d
                }
                TxCommand::Dummy(len) => {
                    self.tx_cache = TxCache::Dummy(len - 1);
                    Some(WD::default())
                }
            }
        } else {
            self.tx_cache = TxCache::Dummy(0);
            None
        }
    }

    #[inline]
    fn store_data(&mut self, data: WD) -> bool {
        let last = match &mut self.rx_cache {
            RxCache::Read(buf) => {
                if self.rx_i < buf.len() {
                    buf[self.rx_i] = data;
                    self.rx_i += 1;
                }
                self.rx_i >= buf.len()
            }
            RxCache::Dummy(len) => {
                if self.rx_i < *len {
                    self.rx_i += 1;
                }
                self.rx_i >= *len
            }
        };

        if last { self.pop_rx_cache() } else { true }
    }

    fn pop_rx_cache(&mut self) -> bool {
        if let Ok(cmd) = unsafe { &mut *self.rx_cmd_r.get() }.pop() {
            match cmd {
                RxCommand::Read(p, l) => {
                    let buf = unsafe { slice::from_raw_parts_mut(p, l) };
                    self.rx_cache = RxCache::Read(buf);
                }
                RxCommand::Dummy(len) => {
                    self.rx_cache = RxCache::Dummy(len);
                }
            }
            self.rx_i = 0;
            true
        } else {
            self.rx_cache = RxCache::Dummy(0);
            false
        }
    }
}

pub enum TxCache<WD: Word> {
    Write(Iter<'static, WD>),
    Dummy(usize),
}

pub enum RxCache<WD: Word> {
    Read(&'static mut [WD]),
    Dummy(usize),
}

#[derive(Clone, Copy, PartialEq)]
pub enum Work {
    Stop,
    Start,
    Work,
    Success,
}

impl AtomicCellMember for Work {
    #[inline]
    fn to_num(self) -> usize {
        self as usize
    }

    #[inline]
    unsafe fn from_num(val: usize) -> Self {
        unsafe { core::mem::transmute(val as u8) }
    }
}

// Error Interrupt Handler ------------------------------------------

pub struct ErrorInterruptHandler<OS: OsInterface, SPI, WD: Word> {
    spi: SPI,
    err_code: Arc<AtomicCell<Option<Error>>>,
    notifier: OS::Notifier,
    _wd: PhantomData<WD>,
}

impl<OS, SPI, WD> ErrorInterruptHandler<OS, SPI, WD>
where
    OS: OsInterface,
    WD: Word,
    SPI: SpiPeriph<WD> + Steal,
{
    pub fn handler(&mut self) {
        if let Some(err) = self.spi.get_and_clean_error() {
            self.err_code.store(Some(err), Ordering::Release);
            self.spi.disable_all_interrupt();
            self.notifier.notify();
        }
    }
}
