use super::{utils::*, *};
use crate::{
    Steal,
    common::{
        bus_device::Operation,
        ringbuf::{Consumer, Producer, PushError, RingBuffer},
    },
};
use core::{
    slice::{self, Iter, IterMut},
    sync::atomic::{AtomicU16, Ordering},
};

// BUS --------------------------------------------------------------

pub struct I2cBusInterrupt<OS: OsInterface, I2C> {
    i2c: I2C,
    mode: Arc<AtomicU16>,
    err_code: Arc<AtomicU16>,
    cmd_w: Producer<Command>,
    data_r: Consumer<u8>,
    waiter: OS::NotifyWaiter,
    seq_id: u8,
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
        let (data_w, data_r) = RingBuffer::<u8>::new(max_operation);
        let (cmd_w, cmd_r) = RingBuffer::<Command>::new(max_operation + 8);
        let mode = Arc::new(AtomicU16::new(0));
        let err_code = Arc::new(AtomicU16::new(0));
        let i2c1 = unsafe { i2c.steal() };
        let i2c2 = unsafe { i2c.steal() };
        let it = I2cBusInterruptHandler {
            i2c: i2c1,
            mode: Arc::clone(&mode),
            cmd_r,
            data_w,
            step: Step::End,
            sub_step: 0,
            data_iter: None,
            buf_iter: None,
            read_len: 0,
            slave_addr: Address::Seven(0),
            notifier: notifier.clone(),
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
                data_r,
                seq_id: 0,
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

        self.i2c.it_disable();

        // prepare
        self.seq_id = self.seq_id.wrapping_add(1);
        self.cmd_w.push(Command::Start(self.seq_id))?;
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

        while self.data_r.pop().is_ok() {}
        // reset error code
        self.err_code.store(0, Ordering::Release);
        self.mode
            .store(Mode::Start(self.seq_id).into(), Ordering::Release);
        self.i2c.it_send_start();

        // TODO calculate timeout
        let rst = self.waiter.wait_with(OS::O, 10.millis(), 2, || {
            let mode = self.mode.load(Ordering::Acquire).into();
            let err_code = int_to_err(self.err_code.load(Ordering::Acquire));
            if Mode::Success == mode {
                return Some(Ok(()));
            } else if let Some(err) = err_code {
                return Some(match mode {
                    Mode::Addr => Err(err.nack_addr()),
                    Mode::Data => Err(err.nack_data()),
                    _ => Err(err),
                });
            } else if Mode::Stop == mode {
                return Some(Err(Error::Other));
            }
            None
        });

        self.mode.store(Mode::Stop.into(), Ordering::Release);
        self.i2c.send_stop();

        match rst {
            None => Err(Error::Timeout),
            Some(rst) => rst,
        }
    }

    fn check_stopped(&mut self) -> bool {
        if !self.i2c.is_stopped(true) {
            let mut t = OS::Timeout::start_ms(1);
            while !self.i2c.is_stopped(true) {
                if t.timeout() {
                    self.i2c.send_stop();
                    break;
                }
                OS::yield_thread();
            }

            while !self.i2c.is_stopped(true) {
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
    mode: Arc<AtomicU16>,
    cmd_r: Consumer<Command>,
    data_w: Producer<u8>,
    notifier: OS::Notifier,

    step: Step,
    sub_step: u8,
    data_iter: Option<Iter<'static, u8>>,
    buf_iter: Option<IterMut<'static, u8>>,
    read_len: usize,
    slave_addr: Address,
}

impl<OS, I2C> I2cBusInterruptHandler<OS, I2C>
where
    OS: OsInterface,
    I2C: I2cPeriph,
{
    pub fn handler(&mut self) {
        if let Mode::Start(seq_id) = Mode::from(self.mode.load(Ordering::Acquire)) {
            if self.prepare_cmd(seq_id) {
                self.choose_step(false);
            } else {
                self.step = Step::End;
            }
        }

        match self.step {
            Step::PrepareWrite => {
                if self.prepare_write() {
                    self.mode.store(Mode::Data.into(), Ordering::Release);
                    self.step = Step::Write;
                }
            }
            Step::Write => {
                if self
                    .i2c
                    .it_write_with(|| Self::load_data(&mut self.data_iter, &mut self.cmd_r))
                    .is_ok()
                {
                    self.choose_step(true);
                }
            }
            Step::PrepareRead => {
                if self.prepare_read() {
                    self.mode.store(Mode::Data.into(), Ordering::Release);
                    self.step = Step::Read;
                }
            }
            Step::Read => {
                if let Some(data) = self.i2c.it_read(self.read_len) {
                    self.store_data(data);
                    self.read_len -= 1;
                    if self.read_len == 0 {
                        self.choose_step(true);
                    }
                }
            }
            _ => {
                // abnormal
                self.finish(false);
            }
        }

        if self.step >= Step::Read {
            self.notifier.notify();
        }
    }

    fn prepare_cmd(&mut self, seq_id: u8) -> bool {
        // Clean old commands
        while let Ok(cmd) = self.cmd_r.pop() {
            if cmd == Command::Start(seq_id) {
                match self.cmd_r.pop() {
                    Ok(Command::SlaveAddr(addr)) => {
                        self.slave_addr = Address::Seven(addr);
                        return true;
                    }
                    Ok(Command::SlaveAddr10(addr)) => {
                        self.slave_addr = Address::Ten(addr);
                        return true;
                    }
                    _ => (),
                }
            }
        }
        false
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

    #[inline]
    fn load_data(
        data_iter: &mut Option<Iter<'static, u8>>,
        cmd_r: &mut Consumer<Command>,
    ) -> Option<u8> {
        match data_iter.as_mut() {
            Some(iter) => iter.next().map(|d| *d),
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
        }
        // match self.cmd_r.pop() {
        //     Ok(Command::Data(data)) => Some(data),
        //     _ => None,
        // }
    }

    #[inline]
    fn store_data(&mut self, data: u8) {
        let byte = match &mut self.buf_iter {
            Some(iter) => iter.next(),
            None => match self.cmd_r.pop() {
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
        // self.data_w.push(data).unwrap();
    }

    fn choose_step(&mut self, first: bool) {
        // restart or end
        match self.cmd_r.pop() {
            Ok(Command::Write(p, l)) => {
                let data = unsafe { slice::from_raw_parts(p, l) };
                self.data_iter = Some(data.iter());
                if self.step != Step::Write {
                    if !first {
                        self.i2c.it_send_start();
                    }
                    self.mode.store(Mode::Addr.into(), Ordering::Release);
                    self.sub_step = 0;
                    self.step = Step::PrepareWrite;
                }
            }
            Ok(Command::Read(len)) => {
                self.read_len = len;
                if self.step != Step::Read {
                    if !first {
                        self.i2c.it_send_start();
                    }
                    self.mode.store(Mode::Addr.into(), Ordering::Release);
                    self.sub_step = 0;
                    self.step = Step::PrepareRead;
                }
            }
            _ => {
                self.step = Step::End;
                self.finish(!first);
            }
        }
    }

    #[inline]
    fn finish(&mut self, successful: bool) {
        self.i2c.it_disable();
        self.data_iter.take();
        self.buf_iter.take();
        // clean old commands
        while self.cmd_r.pop().is_ok() {}
        let mode = if successful {
            Mode::Success
        } else {
            Mode::Stop
        };
        self.mode.store(mode.into(), Ordering::Release);
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

impl From<u8> for Step {
    fn from(v: u8) -> Self {
        match v {
            0 => Self::PrepareWrite,
            1 => Self::Write,
            2 => Self::PrepareRead,
            3 => Self::Read,
            _ => Self::End,
        }
    }
}

// Error Interrupt Handler ------------------------------------------

pub struct I2cBusErrorInterruptHandler<OS: OsInterface, I2C> {
    i2c: I2C,
    err_code: Arc<AtomicU16>,
    notifier: OS::Notifier,
}

impl<OS, I2C> I2cBusErrorInterruptHandler<OS, I2C>
where
    OS: OsInterface,
    I2C: I2cPeriph,
{
    pub fn handler(&mut self) -> bool {
        if let Some(err) = self.i2c.get_and_clean_error() {
            self.err_code
                .store(err_to_int(Some(err)), Ordering::Release);
            self.i2c.it_disable();
            self.notifier.notify();
            true
        } else {
            false
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
