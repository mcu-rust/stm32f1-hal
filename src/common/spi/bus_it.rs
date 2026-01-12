use super::*;
use crate::{
    Steal,
    common::{
        atomic_cell::{AtomicCell, AtomicCellMember, Ordering},
        ringbuf::{Consumer, Producer, RingBuffer},
    },
    fugit::{KilohertzU32, NanosDurationU32},
    os_trait::Duration,
};
use core::{cell::UnsafeCell, mem::size_of};

pub struct SpiBus<OS: OsInterface, SPI> {
    spi: SPI,
    work: Arc<AtomicCell<Work>>,
    err_code: Arc<AtomicCell<Option<Error>>>,
    tx_cmd_w: Producer<TxCommand>,
    rx_cmd_w: Producer<RxCommand>,
    tx_cmd_r: Arc<UnsafeCell<Consumer<TxCommand>>>,
    rx_cmd_r: Arc<UnsafeCell<Consumer<RxCommand>>>,
    waiter: OS::NotifyWaiter,
    byte_period: NanosDurationU32,
}

unsafe impl<OS, SPI> Send for SpiBus<OS, SPI>
where
    OS: OsInterface,
    SPI: SpiPeriph + Steal,
{
}

impl<OS, SPI> SpiBus<OS, SPI>
where
    OS: OsInterface,
    SPI: SpiPeriph + Steal,
{
    #[allow(
        clippy::arc_with_non_send_sync,
        reason = "It's safe because it's only used when interrupts are disabled."
    )]
    pub fn new(
        mut spi: SPI,
        freq: KilohertzU32,
        max_operation: usize,
    ) -> (
        Self,
        InterruptHandler<OS, SPI>,
        ErrorInterruptHandler<OS, SPI>,
    ) {
        spi.disable_all_interrupt();
        let work = Arc::new(AtomicCell::new(Work::Success));
        let err_code = Arc::new(AtomicCell::new(None));
        let (notifier, waiter) = OS::notify();
        let (tx_cmd_w, tx_cmd_r) = RingBuffer::<TxCommand>::new(max_operation);
        let tx_cmd_r = Arc::new(UnsafeCell::new(tx_cmd_r));
        let (rx_cmd_w, rx_cmd_r) = RingBuffer::<RxCommand>::new(max_operation);
        let rx_cmd_r = Arc::new(UnsafeCell::new(rx_cmd_r));
        let byte_period = (freq.into_duration() as NanosDurationU32) * 10;
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
                tx_cache: TxCommand::Dummy(0),
                tx_i: 0,
                rx_cache: RxCommand::Dummy(0),
                rx_i: 0,
            },
            ErrorInterruptHandler {
                spi: spi3,
                err_code,
                notifier,
            },
        )
    }

    fn push_write_slice<W: Word>(&mut self, data: &[W]) -> Result<(), Error> {
        match size_of::<W>() {
            1 => self
                .tx_cmd_w
                .push(TxCommand::WriteU8(data.as_ptr() as *const u8, data.len()))?,
            2 => self
                .tx_cmd_w
                .push(TxCommand::WriteU16(data.as_ptr() as *const u16, data.len()))?,
            _ => self
                .tx_cmd_w
                .push(TxCommand::WriteU32(data.as_ptr() as *const u32, data.len()))?,
        }
        Ok(())
    }

    fn push_read_slice<W: Word>(&mut self, buf: &mut [W]) -> Result<(), Error> {
        match size_of::<W>() {
            1 => self
                .rx_cmd_w
                .push(RxCommand::ReadU8(buf.as_mut_ptr() as *mut u8, buf.len()))?,
            2 => self
                .rx_cmd_w
                .push(RxCommand::ReadU16(buf.as_mut_ptr() as *mut u16, buf.len()))?,
            _ => self
                .rx_cmd_w
                .push(RxCommand::ReadU32(buf.as_mut_ptr() as *mut u32, buf.len()))?,
        }
        Ok(())
    }

    fn push_write_data<W: Word>(&mut self, data: &[W]) -> Result<usize, Error> {
        if data.is_empty() {
            return Err(Error::Buffer);
        }

        self.push_write_slice(data)?;
        self.rx_cmd_w.push(RxCommand::Dummy(data.len()))?;
        Ok(data.len())
    }

    fn push_read_buf<W: Word>(&mut self, buf: &mut [W]) -> Result<usize, Error> {
        if buf.is_empty() {
            return Err(Error::Buffer);
        }

        self.tx_cmd_w.push(TxCommand::Dummy(buf.len()))?;
        self.push_read_slice(buf)?;
        Ok(buf.len())
    }

    fn push_transfer<W: Word>(&mut self, buf: &mut [W], data: &[W]) -> Result<usize, Error> {
        let max_len = buf.len().max(data.len());
        if max_len == 0 {
            return Err(Error::Buffer);
        }

        if !data.is_empty() {
            self.push_write_slice(data)?;
        }
        if !buf.is_empty() {
            self.push_read_slice(buf)?;
        }
        if max_len > data.len() {
            self.tx_cmd_w.push(TxCommand::Dummy(max_len - data.len()))?;
        } else if max_len > buf.len() {
            self.rx_cmd_w.push(RxCommand::Dummy(max_len - buf.len()))?;
        }
        Ok(max_len)
    }

    fn push_transfer_in_place<W: Word>(&mut self, buf: &mut [W]) -> Result<usize, Error> {
        if buf.is_empty() {
            return Err(Error::Buffer);
        }

        self.push_write_slice(buf)?;
        self.push_read_slice(buf)?;
        Ok(buf.len())
    }

    fn communicate<W: Word>(&mut self, data_len: usize) -> Result<(), Error> {
        if data_len == 0 {
            return Ok(());
        }

        let timeout_ns = ((data_len + 2) * size_of::<W>()) as u32 * self.byte_period.ticks();
        self.work
            .store(Work::Start(size_of::<W>() as u8), Ordering::Release);
        self.spi.set_interrupt(Event::TxEmpty, true);

        let rst: Result<(), Error> = self
            .waiter
            .wait_with(&Duration::<OS>::nanos(timeout_ns), 1, || {
                let work = self.work.load(Ordering::Acquire);
                let err_code = self.err_code.load(Ordering::Acquire);
                if Work::Success == work {
                    return Some(Ok(()));
                } else if let Some(err) = err_code {
                    return Some(Err(err));
                }
                None
            })
            .unwrap_or(Err(Error::Timeout));

        self.spi.disable_all_interrupt();

        rst
    }

    fn inner_transaction<W: Word>(
        &mut self,
        operations: &mut [Operation<'_, W>],
    ) -> Result<(), Error> {
        if operations.is_empty() {
            return Ok(());
        }

        if self.spi.is_busy() {
            return Err(Error::Busy);
        }

        // TODO: clear all errors
        while self.spi.read::<W>().is_some() {}

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
                    self.communicate::<W>(data_len)?;
                    OS::delay().delay_ns(*ns);
                    data_len = 0;
                }
            }
            Ok(())
        });
        let rst = op_rst.and_then(|_| self.communicate::<W>(data_len));
        if rst.is_err() {
            // TODO error handler
        }
        rst
    }
}

impl<OS, SPI> SpiBusInterface for SpiBus<OS, SPI>
where
    OS: OsInterface,
    SPI: SpiPeriph + Steal,
{
    #[inline]
    fn transaction<W: Word>(&mut self, operations: &mut [Operation<'_, W>]) -> Result<(), Error> {
        self.inner_transaction(operations)
    }

    #[inline]
    fn config<W: Word>(&mut self, mode: Mode, freq: KilohertzU32) {
        if self.spi.config::<W>(mode, freq) {
            self.byte_period = (freq.into_duration() as NanosDurationU32) * 10;
        }
    }
}

// Interrupt Handler ------------------------------------------------

pub struct InterruptHandler<OS: OsInterface, SPI> {
    spi: SPI,
    work: Arc<AtomicCell<Work>>,
    tx_cmd_r: Arc<UnsafeCell<Consumer<TxCommand>>>,
    rx_cmd_r: Arc<UnsafeCell<Consumer<RxCommand>>>,
    notifier: OS::Notifier,

    tx_cache: TxCommand,
    tx_i: usize,
    rx_cache: RxCommand,
    rx_i: usize,
}

impl<OS, SPI> InterruptHandler<OS, SPI>
where
    OS: OsInterface,
    SPI: SpiPeriph + Steal,
{
    pub fn handler(&mut self) {
        if let Work::Start(w) = self.work.load(Ordering::Acquire) {
            self.work.store(Work::Work(w), Ordering::Relaxed);
            self.tx_cache = TxCommand::Dummy(0);
            self.pop_rx_cache();
        }

        match self.work.load(Ordering::Relaxed) {
            Work::Work(1) => self.inner_handler::<u8>(),
            Work::Work(2) => self.inner_handler::<u16>(),
            Work::Work(4) => self.inner_handler::<u32>(),
            _ => (),
        }
    }

    fn inner_handler<W: Word>(&mut self) {
        while self.spi.is_tx_empty() {
            if let Some(data) = self.load_data() {
                self.spi.uncheck_write::<W>(data);
            } else {
                self.spi.set_interrupt(Event::RxNotEmpty, true);
                self.spi.set_interrupt(Event::TxEmpty, false);
                break;
            }
        }

        if let Some(data) = self.spi.read::<W>()
            && !self.store_data(data)
        {
            self.spi.disable_all_interrupt();
            self.work.store(Work::Success, Ordering::Release);
            self.notifier.notify();
        }
    }

    #[inline]
    fn load_data<W: Word>(&mut self) -> Option<W> {
        loop {
            let data = match &self.tx_cache {
                TxCommand::WriteU8(p, len) => {
                    if self.tx_i < *len {
                        Some(W::from_u32(unsafe { *(p.add(self.tx_i)) } as u32))
                    } else {
                        None
                    }
                }
                TxCommand::WriteU16(p, len) => {
                    if self.tx_i < *len {
                        Some(W::from_u32(unsafe { *(p.add(self.tx_i)) } as u32))
                    } else {
                        None
                    }
                }
                TxCommand::WriteU32(p, len) => {
                    if self.tx_i < *len {
                        Some(W::from_u32(unsafe { *(p.add(self.tx_i)) }))
                    } else {
                        None
                    }
                }
                TxCommand::Dummy(len) => {
                    if self.tx_i < *len {
                        Some(W::default())
                    } else {
                        None
                    }
                }
            };

            self.tx_i += 1;
            if data.is_some() {
                return data;
            }

            if let Ok(cmd) = unsafe { &mut *self.tx_cmd_r.get() }.pop() {
                self.tx_cache = cmd;
                self.tx_i = 0;
            } else {
                self.tx_cache = TxCommand::Dummy(0);
                return None;
            }
        }
    }

    #[inline]
    fn store_data<W: Word>(&mut self, data: W) -> bool {
        let len = match &self.rx_cache {
            RxCommand::ReadU8(p, len) => {
                if self.rx_i < *len {
                    unsafe { *(p.add(self.rx_i)) = data.into_u32() as u8 };
                }
                *len
            }
            RxCommand::ReadU16(p, len) => {
                if self.rx_i < *len {
                    unsafe { *(p.add(self.rx_i)) = data.into_u32() as u16 };
                }
                *len
            }
            RxCommand::ReadU32(p, len) => {
                if self.rx_i < *len {
                    unsafe { *(p.add(self.rx_i)) = data.into_u32() };
                }
                *len
            }
            RxCommand::Dummy(len) => *len,
        };

        self.rx_i += 1;
        if self.rx_i >= len {
            self.pop_rx_cache()
        } else {
            true
        }
    }

    fn pop_rx_cache(&mut self) -> bool {
        if let Ok(cmd) = unsafe { &mut *self.rx_cmd_r.get() }.pop() {
            self.rx_cache = cmd;
            self.rx_i = 0;
            true
        } else {
            self.rx_cache = RxCommand::Dummy(0);
            false
        }
    }
}

#[derive(Clone, Copy)]
pub enum TxCommand {
    WriteU8(*const u8, usize),
    WriteU16(*const u16, usize),
    WriteU32(*const u32, usize),
    Dummy(usize),
}

#[derive(Clone, Copy)]
pub enum RxCommand {
    ReadU8(*mut u8, usize),
    ReadU16(*mut u16, usize),
    ReadU32(*mut u32, usize),
    Dummy(usize),
}

#[derive(Clone, Copy, PartialEq)]
pub enum Work {
    Success,
    Start(u8),
    Work(u8),
}

impl AtomicCellMember for Work {
    #[inline]
    fn to_num(self) -> usize {
        match self {
            Self::Success => 0,
            Self::Start(w) => ((w as usize) << 8) | 1,
            Self::Work(w) => ((w as usize) << 8) | 2,
        }
    }

    #[inline]
    unsafe fn from_num(val: usize) -> Self {
        let w = (val >> 8) as u8;
        let v = val as u8;
        match v {
            0 => Self::Success,
            1 => Self::Start(w),
            _ => Self::Work(w),
        }
    }
}

// Error Interrupt Handler ------------------------------------------

pub struct ErrorInterruptHandler<OS: OsInterface, SPI> {
    spi: SPI,
    err_code: Arc<AtomicCell<Option<Error>>>,
    notifier: OS::Notifier,
}

impl<OS, SPI> ErrorInterruptHandler<OS, SPI>
where
    OS: OsInterface,
    SPI: SpiPeriph + Steal,
{
    pub fn handler(&mut self) {
        if let Some(err) = self.spi.get_and_clean_error() {
            self.err_code.store(Some(err), Ordering::Release);
            self.spi.disable_all_interrupt();
            self.notifier.notify();
        }
    }
}
