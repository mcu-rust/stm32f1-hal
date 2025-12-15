use super::{utils::*, *};
use crate::{
    Steal,
    common::{
        atomic_cell::{AtomicCell, Ordering},
        bus_device::Operation,
        ringbuf::{Consumer, Producer, PushError, RingBuffer},
    },
};
use core::{
    cell::UnsafeCell,
    slice::{self, Iter, IterMut},
};

// BUS --------------------------------------------------------------

pub struct I2cBusInterrupt<OS: OsInterface, I2C> {
    i2c: I2C,
    mode: Arc<AtomicCell<Work>>,
    err_code: Arc<AtomicCell<Option<Error>>>,
    cmd_w: Producer<Command>,
    cmd_r: Arc<UnsafeCell<Consumer<Command>>>,
    waiter: OS::NotifyWaiter,
}

impl<OS, I2C> I2cBusInterrupt<OS, I2C>
where
    OS: OsInterface,
    I2C: I2cPeriph + Steal,
{
    pub fn new(
        i2c: I2C,
        max_operation: usize,
    ) -> (
        Self,
        I2cBusInterruptHandler<OS, I2C>,
        I2cBusErrorInterruptHandler<OS, I2C>,
    ) {
        let (notifier, waiter) = OS::notify();
        let (cmd_w, cmd_r) = RingBuffer::<Command>::new(max_operation + 8);
        let cmd_r = Arc::new(UnsafeCell::new(cmd_r));
        let mode = Arc::new(AtomicCell::new(Work::Stop));
        let err_code = Arc::new(AtomicCell::new(None));
        let i2c1 = unsafe { i2c.steal() };
        let i2c2 = unsafe { i2c.steal() };
        let it = I2cBusInterruptHandler {
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
            // count: [0; 4],
            // reg: [0; 16],
        };
        let it_err = I2cBusErrorInterruptHandler {
            i2c: i2c2,
            err_code: Arc::clone(&err_code),
            notifier,
        };
        (
            Self {
                i2c,
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

    pub fn i2c_transaction(
        &mut self,
        slave_addr: Address,
        operations: &mut [Operation<'_, u8>],
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

        let mut i = 0;
        while i < operations.len() {
            // Unsupported operations
            match &operations[i] {
                Operation::DelayNs(_)
                | Operation::Transfer(_, _)
                | Operation::TransferInPlace(_) => {
                    panic!()
                }
                _ => (),
            }

            // push writing buffer
            let mut has_write = false;
            for op in operations[i..].iter() {
                if let Operation::Write(data) = op {
                    let d: &[u8] = data;
                    self.cmd_w.push(Command::Write(d.as_ptr(), d.len()))?;
                    has_write = true;
                    i += 1;
                } else {
                    break;
                }
            }

            if has_write {
                self.cmd_w.push(Command::WriteEnd)?;
            }

            // push reading length
            let mut buf_len = 0;
            for op in operations[i..].iter() {
                if let Operation::Read(buf) = op {
                    if buf.len() == 0 {
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
                for op in operations[i..].iter_mut() {
                    if let Operation::Read(buf) = op {
                        let b: &mut [u8] = buf;
                        self.cmd_w.push(Command::ReadBuf(b.as_mut_ptr(), b.len()))?;
                        i += 1;
                    } else {
                        break;
                    }
                }
            }
        }

        // reset error code
        self.err_code.store(None, Ordering::Release);
        self.mode.store(Work::Start, Ordering::Release);
        self.i2c.it_send_start();

        // TODO calculate timeout
        let rst = self.waiter.wait_with(OS::O, 10.millis(), 2, || {
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

        match rst {
            None => Err(Error::Timeout),
            Some(rst) => rst,
        }
    }

    fn check_stopped(&mut self) -> bool {
        if !self.i2c.is_stopped() {
            let mut t = OS::Timeout::start_ms(1);
            while !self.i2c.is_stopped() {
                if t.timeout() {
                    self.i2c.send_stop();
                    break;
                }
                OS::yield_thread();
            }

            while !self.i2c.is_stopped() {
                if t.timeout() {
                    return false;
                }
                OS::yield_thread();
            }
        }
        true
    }
}

impl<T> From<PushError<T>> for Error {
    fn from(_value: PushError<T>) -> Self {
        Self::Buffer
    }
}

impl<OS, I2C> I2cBusInterface for I2cBusInterrupt<OS, I2C>
where
    OS: OsInterface,
    I2C: I2cPeriph + Steal,
{
    #[inline]
    fn transaction(
        &mut self,
        slave_addr: Address,
        operations: &mut [Operation<'_, u8>],
    ) -> Result<(), Error> {
        self.i2c_transaction(slave_addr, operations)
    }
}

// Interrupt Handler ------------------------------------------------

pub struct I2cBusInterruptHandler<OS: OsInterface, I2C> {
    i2c: I2C,
    mode: Arc<AtomicCell<Work>>,
    cmd_r: Arc<UnsafeCell<Consumer<Command>>>,
    notifier: OS::Notifier,

    step: Step,
    sub_step: u8,
    data_iter: Option<Iter<'static, u8>>,
    buf_iter: Option<IterMut<'static, u8>>,
    read_len: usize,
    slave_addr: Address,
    // count: [u32; 4],
    // reg: [u32; 16],
}

impl<OS, I2C> I2cBusInterruptHandler<OS, I2C>
where
    OS: OsInterface,
    I2C: I2cPeriph,
{
    pub fn handler(&mut self) {
        // self.reg[self.count[0] as usize] = self.i2c.read_sr();
        // self.count[0] = (self.count[0] + 1) & 0x0F;

        if Work::Start == self.mode.load(Ordering::Acquire) {
            if self.prepare_cmd() {
                match self.cmd().pop() {
                    Ok(Command::Write(p, l)) => {
                        self.to_prepare_write(p, l);
                    }
                    Ok(Command::Read(len)) => {
                        self.to_prepare_read(len);
                    }
                    _ => {
                        self.step = Step::End;
                    }
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
                            self.to_prepare_read(len);
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
                    if self.read_len == 1 {
                        if self.cmd().peek().is_ok() {
                            self.i2c.it_send_start();
                        } else {
                            self.i2c.send_stop();
                        }
                    }
                    self.step_to(Step::Read);
                }
            }
            Step::Read => {
                if let Some(data) = self.i2c.it_read(self.read_len) {
                    self.store_data(data);
                    self.read_len -= 1;
                    if self.read_len == 1 {
                        if self.cmd().peek().is_ok() {
                            self.i2c.it_send_start();
                        } else {
                            self.i2c.send_stop();
                        }
                    } else if self.read_len == 0 {
                        self.i2c.disable_data_interrupt();
                        match self.cmd().pop() {
                            Ok(Command::Write(p, l)) => {
                                self.to_prepare_write(p, l);
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

        if self.step >= Step::Read {
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
            .it_prepare_read(self.slave_addr, self.read_len, &mut self.sub_step)
            .is_ok()
    }

    fn to_prepare_write(&mut self, p: *const u8, len: usize) {
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
                        let data = iter.next().map(|d| *d);
                        data_iter.replace(iter);
                        data
                    }
                    _ => None,
                },
            },
            None => None,
        }
    }

    fn to_prepare_read(&mut self, len: usize) {
        self.read_len = len;
        if let Ok(Command::ReadBuf(p, l)) = self.cmd().pop() {
            let data = unsafe { slice::from_raw_parts_mut(p, l) };
            self.buf_iter.replace(data.iter_mut());
        }
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
                    b
                }
                _ => None,
            },
        };
        byte.map(|b| *b = data);
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

pub struct I2cBusErrorInterruptHandler<OS: OsInterface, I2C> {
    i2c: I2C,
    err_code: Arc<AtomicCell<Option<Error>>>,
    notifier: OS::Notifier,
}

impl<OS, I2C> I2cBusErrorInterruptHandler<OS, I2C>
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
    fn flat(data: &[&[u8]]) -> Vec<u8> {
        let mut v = vec![];
        for d in data.iter().flat_map(|d| d.iter()) {
            v.push(*d);
        }
        v
    }

    fn flat_mut(buf: &mut [&mut [u8]]) {
        let mut i = 0;
        for b in buf.iter_mut().flat_map(|b| b.iter_mut()) {
            i += 1;
            *b = i;
        }
    }

    #[test]
    fn test_flat() {
        let a = [1u8, 2, 3];
        let b = [4u8, 5];
        assert_eq!(flat(&[&a, &b]), vec![1u8, 2, 3, 4, 5]);

        let mut a = [0u8; 3];
        let mut b = [0; 2];
        let mut c = [a.as_mut_slice(), b.as_mut_slice()];
        flat_mut(c.as_mut_slice());
        assert_eq!(a, [1, 2, 3]);
        assert_eq!(b, [4, 5]);
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
