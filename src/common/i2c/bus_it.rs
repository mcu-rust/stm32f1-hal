use super::{utils::*, *};
use crate::{
    Steal,
    common::{
        atomic_cell::{AtomicCell, Ordering},
        fugit::NanosDurationU32,
        os_trait::{Duration, Timeout},
        ringbuf::{Consumer, Producer, RingBuffer},
    },
    embedded_hal::i2c,
};
use core::{
    cell::UnsafeCell,
    slice::{self, Iter, IterMut},
};

// BUS --------------------------------------------------------------

pub struct I2cBus<OS: OsInterface, I2C> {
    i2c: I2C,
    mode: Arc<AtomicCell<Work>>,
    err_code: Arc<AtomicCell<Option<Error>>>,
    cmd_w: Producer<Command>,
    cmd_r: Arc<UnsafeCell<Consumer<Command>>>,
    waiter: OS::NotifyWaiter,
    byte_period: NanosDurationU32,
}

impl<OS, I2C> I2cBus<OS, I2C>
where
    OS: OsInterface,
    I2C: I2cPeriph + Steal,
{
    pub fn new(
        i2c: I2C,
        speed: HertzU32,
        max_operation: usize,
    ) -> (
        Self,
        InterruptHandler<OS, I2C>,
        ErrorInterruptHandler<OS, I2C>,
    ) {
        let (notifier, waiter) = OS::notify();
        let (cmd_w, cmd_r) = RingBuffer::<Command>::new(max_operation + 8);
        #[allow(
            clippy::arc_with_non_send_sync,
            reason = "It's safe because it's only used when interrupts are disabled."
        )]
        let cmd_r = Arc::new(UnsafeCell::new(cmd_r));
        let mode = Arc::new(AtomicCell::new(Work::Stop));
        let err_code = Arc::new(AtomicCell::new(None));
        let i2c1 = unsafe { i2c.steal() };
        let i2c2 = unsafe { i2c.steal() };
        let it = InterruptHandler {
            i2c: i2c1,
            mode: Arc::clone(&mode),
            cmd_r: Arc::clone(&cmd_r),
            step: Step::End,
            sub_step: 0,
            data_iter: None,
            buf_iter: None,
            read_len: 0,
            slave_addr: Address::Seven(0),
            notifier: notifier.clone(),
            last_operation: false,
            // count: [0; 4],
            // reg: [0; 16],
        };
        let it_err = ErrorInterruptHandler {
            i2c: i2c2,
            err_code: Arc::clone(&err_code),
            notifier,
        };
        let byte_period = (speed.into_duration() as NanosDurationU32) * 12;
        (
            Self {
                i2c,
                byte_period,
                mode,
                err_code,
                cmd_w,
                cmd_r,
                waiter,
            },
            it,
            it_err,
        )
    }

    fn check_stopped(&mut self) -> bool {
        if self.i2c.is_stopped() {
            true
        } else {
            let mut t = Timeout::<OS>::millis(1);
            let mut i = 0;
            loop {
                if self.i2c.is_stopped() {
                    return true;
                } else if t.timeout() {
                    match i {
                        0 => {
                            self.i2c.handle_error(Error::Busy);
                            i += 1;
                        }
                        _ => break,
                    }
                } else {
                    OS::yield_thread();
                }
            }
            false
        }
    }

    fn push_write_data<OP: IntoI2cOperation>(
        &mut self,
        operations: &[OP],
        i: &mut usize,
    ) -> Result<usize, Error> {
        let mut write_len = 0;
        for op in operations.iter() {
            if let Some(data) = op.get_write_buf() {
                let d: &[u8] = data;
                if !d.is_empty() {
                    write_len += d.len();
                    self.cmd_w.push(Command::Write(d.as_ptr(), d.len()))?;
                }
                *i += 1;
            } else {
                break;
            }
        }

        if write_len > 0 {
            self.cmd_w.push(Command::WriteEnd)?;
        }
        Ok(write_len)
    }

    fn push_read_buf<OP: IntoI2cOperation>(
        &mut self,
        operations: &mut [OP],
        i: &mut usize,
    ) -> Result<usize, Error> {
        let mut buf_len = 0;
        for op in operations.iter_mut() {
            if let Some(buf) = op.get_read_buf() {
                if buf.is_empty() {
                    return Err(Error::Buffer);
                }
                buf_len += buf.len();
            } else {
                break;
            }
        }

        // push reading buffer
        if buf_len > 0 {
            self.cmd_w.push(Command::Read(buf_len))?;
            for op in operations.iter_mut() {
                if let Some(buf) = op.get_read_buf() {
                    let b: &mut [u8] = buf;
                    self.cmd_w.push(Command::ReadBuf(b.as_mut_ptr(), b.len()))?;
                    *i += 1;
                } else {
                    break;
                }
            }
        }
        Ok(buf_len)
    }

    fn inner_transaction<OP: IntoI2cOperation>(
        &mut self,
        slave_addr: Address,
        operations: &mut [OP],
    ) -> Result<(), Error> {
        // the bus is protected, so it must be stopped
        if !self.check_stopped() {
            return Err(Error::Busy);
        }

        self.i2c.disable_all_interrupt();

        // clean old commands
        let cmd = unsafe { &mut *self.cmd_r.get() };
        while cmd.pop().is_ok() {}

        // prepare commands
        match slave_addr {
            Address::Seven(addr) => self.cmd_w.push(Command::SlaveAddr(addr))?,
            Address::Ten(addr) => self.cmd_w.push(Command::SlaveAddr10(addr))?,
        }

        let mut data_len = 0;
        let mut i = 0;
        while i < operations.len() {
            data_len += self.push_write_data(&operations[i..], &mut i)?;
            data_len += self.push_read_buf(&mut operations[i..], &mut i)?;
        }
        let timeout_ns = (data_len as u32 + 2) * self.byte_period.ticks();

        // reset error code
        self.err_code.store(None, Ordering::Release);
        self.mode.store(Work::Start, Ordering::Release);
        self.i2c.it_send_start();

        let rst = self
            .waiter
            .wait_with(&Duration::<OS>::nanos(timeout_ns), 1, || {
                let mode = self.mode.load(Ordering::Acquire);
                let err_code = self.err_code.load(Ordering::Acquire);
                if Work::Success == mode {
                    return Some(Ok(()));
                } else if let Some(err) = err_code {
                    return Some(match mode {
                        Work::Addr => Err(err.nack_addr()),
                        Work::Data => Err(err.nack_data()),
                        _ => Err(err),
                    });
                } else if Work::Stop == mode {
                    return Some(Err(Error::Other));
                }
                None
            });

        self.i2c.disable_all_interrupt();

        self.mode.store(Work::Stop, Ordering::Release);
        if !self.i2c.is_stopped() {
            self.i2c.send_stop();
        }

        let rst = match rst {
            None => Err(Error::Timeout),
            Some(rst) => rst,
        };
        if let Err(err) = rst {
            self.i2c.handle_error(err);
        }
        rst
    }
}

impl<OS, I2C> I2cBusInterface for I2cBus<OS, I2C>
where
    OS: OsInterface,
    I2C: I2cPeriph + Steal,
{
    #[inline]
    fn transaction<OP: IntoI2cOperation>(
        &mut self,
        slave_addr: Address,
        operations: &mut [OP],
    ) -> Result<(), Error> {
        self.inner_transaction(slave_addr, operations)
    }
}

// Implement embedded-hal traits ------------------------------------

impl<OS, I2C> i2c::ErrorType for I2cBus<OS, I2C>
where
    OS: OsInterface,
    I2C: I2cPeriph + Steal,
{
    type Error = Error;
}

impl<OS, I2C> i2c::I2c<i2c::SevenBitAddress> for I2cBus<OS, I2C>
where
    OS: OsInterface,
    I2C: I2cPeriph + Steal,
{
    #[inline]
    fn transaction(
        &mut self,
        address: i2c::SevenBitAddress,
        operations: &mut [i2c::Operation<'_>],
    ) -> Result<(), Self::Error> {
        self.inner_transaction(Address::Seven(address), operations)
    }
}

impl<OS, I2C> i2c::I2c<i2c::TenBitAddress> for I2cBus<OS, I2C>
where
    OS: OsInterface,
    I2C: I2cPeriph + Steal,
{
    #[inline]
    fn transaction(
        &mut self,
        address: i2c::TenBitAddress,
        operations: &mut [i2c::Operation<'_>],
    ) -> Result<(), Self::Error> {
        self.inner_transaction(Address::Ten(address), operations)
    }
}

// Interrupt Handler ------------------------------------------------

pub struct InterruptHandler<OS: OsInterface, I2C> {
    i2c: I2C,
    mode: Arc<AtomicCell<Work>>,
    cmd_r: Arc<UnsafeCell<Consumer<Command>>>,
    notifier: OS::Notifier,

    step: Step,
    sub_step: u8,
    slave_addr: Address,
    data_iter: Option<Iter<'static, u8>>,
    buf_iter: Option<IterMut<'static, u8>>,
    read_len: usize,
    last_operation: bool,
    // count: [u32; 4],
    // reg: [u32; 16],
}

impl<OS, I2C> InterruptHandler<OS, I2C>
where
    OS: OsInterface,
    I2C: I2cPeriph,
{
    pub fn handler(&mut self) {
        // self.reg[(self.count[0] & 0x0F) as usize] = self.i2c.read_sr();
        // self.count[0] += 1;

        if Work::Start == self.mode.load(Ordering::Acquire) && self.prepare_cmd() {
            match self.cmd().pop() {
                Ok(Command::Write(p, l)) => {
                    self.setp_to_prepare_write(p, l);
                }
                Ok(Command::Read(len)) => {
                    self.step_to_prepare_read(len);
                }
                _ => {
                    self.step = Step::End;
                }
            }
        }

        match self.step {
            Step::PrepareWrite => {
                if self.prepare_write() {
                    self.mode.store(Work::Data, Ordering::Release);
                    self.step = Step::Write;
                }
            }
            Step::Write => {
                let cmd = unsafe { &mut *self.cmd_r.get() };
                let data_iter = &mut self.data_iter;
                if self
                    .i2c
                    .it_write_with(|| Self::load_data(data_iter, cmd))
                    .is_ok()
                {
                    match self.cmd().pop() {
                        Ok(Command::Read(len)) => {
                            self.step_to_prepare_read(len);
                            self.i2c.disable_data_interrupt();
                            self.i2c.it_send_start();
                        }
                        _ => {
                            self.i2c.send_stop();
                            self.step_to(Step::End);
                        }
                    }
                }
            }
            Step::PrepareRead => {
                if self.prepare_read() {
                    self.step_to(Step::Read);
                }
            }
            Step::Read => {
                if let Some(data) = self.i2c.it_read(self.read_len, self.last_operation) {
                    self.store_data(data);
                    self.read_len -= 1;
                    if self.read_len == 0 {
                        self.i2c.disable_data_interrupt();
                        match self.cmd().pop() {
                            Ok(Command::Write(p, l)) => {
                                self.setp_to_prepare_write(p, l);
                            }
                            _ => self.step_to(Step::End),
                        }
                    }
                }
            }
            Step::End => {
                // abnormal
                self.finish(false);
            }
        }

        if self.step == Step::End {
            self.notifier.notify();
        }
    }

    #[inline]
    fn prepare_write(&mut self) -> bool {
        self.i2c
            .it_prepare_write(self.slave_addr, &mut self.sub_step)
            .is_ok()
    }

    #[inline]
    fn prepare_read(&mut self) -> bool {
        self.i2c
            .it_prepare_read(
                self.slave_addr,
                self.read_len,
                self.last_operation,
                &mut self.sub_step,
            )
            .is_ok()
    }

    fn setp_to_prepare_write(&mut self, p: *const u8, len: usize) {
        let data = unsafe { slice::from_raw_parts(p, len) };
        self.data_iter = Some(data.iter());
        self.step_to(Step::PrepareWrite);
    }

    #[inline]
    fn load_data(
        data_iter: &mut Option<Iter<'static, u8>>,
        cmd_r: &mut Consumer<Command>,
    ) -> Option<u8> {
        match data_iter.as_mut() {
            Some(iter) => match iter.next() {
                Some(data) => Some(*data),
                None => match cmd_r.pop() {
                    Ok(Command::Write(p, l)) => {
                        let data = unsafe { slice::from_raw_parts(p, l) };
                        let mut iter = data.iter();
                        let data = iter.next().copied();
                        data_iter.replace(iter);
                        data
                    }
                    _ => None,
                },
            },
            None => None,
        }
    }

    fn step_to_prepare_read(&mut self, len: usize) {
        self.read_len = len;
        if let Ok(Command::ReadBuf(p, l)) = self.cmd().pop() {
            let data = unsafe { slice::from_raw_parts_mut(p, l) };
            self.buf_iter.replace(data.iter_mut());
        }
        self.last_operation = self.cmd().peek().is_err();
        self.step_to(Step::PrepareRead);
    }

    #[inline]
    fn store_data(&mut self, data: u8) {
        let byte = match &mut self.buf_iter {
            Some(iter) => iter.next(),
            None => match self.cmd().pop() {
                Ok(Command::ReadBuf(p, l)) => {
                    let data = unsafe { slice::from_raw_parts_mut(p, l) };
                    let mut iter = data.iter_mut();
                    let b = iter.next();
                    self.buf_iter.replace(iter);
                    self.last_operation = self.cmd().peek().is_err();
                    b
                }
                _ => None,
            },
        };
        if let Some(b) = byte {
            *b = data
        }
    }

    fn step_to(&mut self, step: Step) {
        self.step = step;
        match step {
            Step::PrepareWrite | Step::PrepareRead => {
                self.mode.store(Work::Addr, Ordering::Release);
                self.sub_step = 0;
            }
            Step::Write | Step::Read => {
                self.mode.store(Work::Data, Ordering::Release);
            }
            Step::End => self.finish(true),
        }
    }

    fn prepare_cmd(&mut self) -> bool {
        self.step = Step::End;
        self.data_iter = None;
        self.buf_iter = None;

        match self.cmd().pop() {
            Ok(Command::SlaveAddr(addr)) => {
                self.slave_addr = Address::Seven(addr);
                true
            }
            Ok(Command::SlaveAddr10(addr)) => {
                self.slave_addr = Address::Ten(addr);
                true
            }
            _ => false,
        }
    }

    #[inline]
    fn finish(&mut self, successful: bool) {
        self.i2c.disable_all_interrupt();
        self.data_iter = None;
        self.buf_iter = None;

        let mode = if successful {
            Work::Success
        } else {
            Work::Stop
        };
        self.mode.store(mode, Ordering::Release);
    }

    #[inline]
    fn cmd(&mut self) -> &mut Consumer<Command> {
        unsafe { &mut *self.cmd_r.get() }
    }
}

#[derive(Clone, Copy, PartialEq, PartialOrd)]
enum Step {
    PrepareWrite = 0,
    Write = 1,
    PrepareRead = 2,
    Read = 3,
    End = 200,
}

// Error Interrupt Handler ------------------------------------------

pub struct ErrorInterruptHandler<OS: OsInterface, I2C> {
    i2c: I2C,
    err_code: Arc<AtomicCell<Option<Error>>>,
    notifier: OS::Notifier,
}

impl<OS, I2C> ErrorInterruptHandler<OS, I2C>
where
    OS: OsInterface,
    I2C: I2cPeriph,
{
    pub fn handler(&mut self) {
        if let Some(err) = self.i2c.get_and_clean_error() {
            self.err_code.store(Some(err), Ordering::Release);
            self.i2c.disable_all_interrupt();
            self.notifier.notify();
        }
    }
}

#[cfg(test)]
mod tests {
    use fugit::{KilohertzU32, MicrosDurationU32, NanosDurationU32, RateExtU32};

    #[test]
    fn test_dur() {
        let speed: KilohertzU32 = 200.kHz();
        let dur = (speed.into_duration() as MicrosDurationU32).ticks();
        assert_eq!(dur, 5);
        let dur = (speed.into_duration() as NanosDurationU32).ticks();
        assert_eq!(dur, 5000);

        let speed: KilohertzU32 = 20.kHz();
        let dur = (speed.into_duration() as NanosDurationU32).ticks();
        assert_eq!(dur, 50000);

        let speed: KilohertzU32 = 400.kHz();
        let dur = (speed.into_duration() as NanosDurationU32).ticks();
        assert_eq!(dur, 2500);
    }

    #[test]
    fn for_loop() {
        let data = [1, 2, 3, 4, 5, 6];

        let mut count = 0;
        let mut i = 0;
        while i < data.len() {
            for d in data[i..].iter() {
                if *d < 4 {
                    i += 1;
                } else {
                    break;
                }
                count += 1;
            }

            assert_eq!(i, 3);
            assert_eq!(count, 3);

            for d in data[i..].iter() {
                if *d >= 4 {
                    i += 1;
                } else {
                    break;
                }
                count += 1;
            }

            assert_eq!(i, 6);
            assert_eq!(count, 6);
        }
    }
}
