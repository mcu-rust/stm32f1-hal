use super::{utils::*, *};
use crate::{
    Steal,
    common::ringbuf::{Consumer, Producer, PushError, RingBuffer},
};
use core::sync::atomic::{AtomicU16, Ordering};

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
        buff_size: usize,
    ) -> (
        Self,
        I2cBusInterruptHandler<OS, I2C>,
        I2cBusErrorInterruptHandler<OS, I2C>,
    ) {
        let (notifier, waiter) = OS::notify();
        let (data_w, data_r) = RingBuffer::<u8>::new(buff_size);
        let (cmd_w, cmd_r) = RingBuffer::<Command>::new(buff_size + 6);
        let mode = Arc::new(AtomicU16::new(0));
        let err_code = Arc::new(AtomicU16::new(0));
        let i2c1 = unsafe { i2c.steal() };
        let i2c2 = unsafe { i2c.steal() };
        let it = I2cBusInterruptHandler {
            i2c: i2c1,
            mode: Arc::clone(&mode),
            cmd_r,
            data_w,
            step: 0,
            sub_step: 0,
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
                i2c: i2c,
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

    pub fn write_and_read(
        &mut self,
        slave_addr: Address,
        data: &[&[u8]],
        buf: &mut [&mut [u8]],
    ) -> Result<(), Error> {
        if buf.is_empty() && data.is_empty() {
            return Err(Error::Buffer);
        }

        // check stop, timeout > 25ms
        if self
            .waiter
            .wait_with(OS::O, 26.millis(), 16, || {
                self.i2c.is_stopped(true).then_some(())
            })
            .is_none()
        {
            return Err(Error::Busy);
        }

        self.i2c.it_reset();

        // prepare
        self.seq_id = self.seq_id.wrapping_add(1);
        self.cmd_w.push(Command::Start(self.seq_id))?;
        match slave_addr {
            Address::Seven(addr) => self.cmd_w.push(Command::SlaveAddr(addr))?,
            Address::Ten(addr) => self.cmd_w.push(Command::SlaveAddr10(addr))?,
        }
        if !data.is_empty() {
            self.push_all_data(data)?;
        }
        if !buf.is_empty() {
            self.cmd_w.push(Command::Len(self.get_all_len(buf)))?;
        }
        while self.data_r.pop().is_ok() {}
        // reset error code
        self.err_code.store(0, Ordering::Release);
        self.mode
            .store(Mode::Start(self.seq_id).into(), Ordering::Release);
        self.i2c.it_send_start();

        // TODO calculate timeout
        let mut buf_iter = buf.iter_mut().flat_map(|b| b.iter_mut());
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

            while let Ok(data) = self.data_r.pop() {
                if let Some(b) = buf_iter.next() {
                    *b = data;
                }
            }
            None
        });

        match rst {
            None => Err(Error::Timeout),
            Some(rst) => {
                if buf_iter.next().is_some() {
                    Err(Error::Other)
                } else {
                    rst
                }
            }
        }
    }

    fn push_all_data(&mut self, data_buf: &[&[u8]]) -> Result<(), Error> {
        self.cmd_w.push(Command::WriteMode)?;
        for d in data_buf.iter().flat_map(|d| d.iter()) {
            self.cmd_w.push(Command::Data(*d))?;
        }
        self.cmd_w.push(Command::EndWrite)?;
        Ok(())
    }

    fn get_all_len(&self, buf: &[&mut [u8]]) -> u16 {
        let mut rst = 0;
        for b in buf.iter() {
            rst += b.len();
        }
        rst as u16
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
    fn write_read(
        &mut self,
        slave_addr: Address,
        write: &[&[u8]],
        read: &mut [&mut [u8]],
    ) -> Result<(), Error> {
        self.write_and_read(slave_addr, write, read)
    }
}

// Interrupt Handler ------------------------------------------------

pub struct I2cBusInterruptHandler<OS: OsInterface, I2C> {
    i2c: I2C,
    mode: Arc<AtomicU16>,
    cmd_r: Consumer<Command>,
    data_w: Producer<u8>,
    notifier: OS::Notifier,

    step: u8,
    sub_step: u8,
    read_len: u16,
    slave_addr: Address,
}

impl<OS, I2C> I2cBusInterruptHandler<OS, I2C>
where
    OS: OsInterface,
    I2C: I2cPeriph,
{
    pub fn handler(&mut self) {
        let mut mode = Mode::from(self.mode.load(Ordering::Acquire));
        if let Mode::Start(seq_id) = mode {
            if self.prepare_cmd(seq_id) {
                if let Ok(cmd) = self.cmd_r.peek() {
                    mode = Mode::Addr;
                    self.mode.store(mode.into(), Ordering::Relaxed);
                    self.sub_step = 0;
                    self.step = match cmd {
                        Command::Len(len) => {
                            self.read_len = *len;
                            2
                        } // jump to read
                        _ => 0,
                    };
                }
            }
        }

        match mode {
            Mode::Start(_) | Mode::Stop | Mode::Success => {
                if !self.i2c.is_stopped(true) {
                    self.i2c.send_stop();
                }
                self.finish(mode == Mode::Success);
                self.notifier.notify();
                return;
            }
            _ => (),
        }

        loop {
            match self.step {
                0 => {
                    if self
                        .i2c
                        .it_prepare_write(self.slave_addr, &mut self.sub_step)
                        .is_err()
                    {
                        break;
                    }
                    self.mode.store(Mode::Data.into(), Ordering::Release);
                    self.next_step();
                }
                1 => {
                    if self
                        .i2c
                        .it_write_with(|| match self.cmd_r.pop() {
                            Ok(Command::Data(data)) => Some(data),
                            _ => None,
                        })
                        .is_err()
                    {
                        break;
                    }
                    // restart or stop
                    if let Ok(Command::Len(len)) = self.cmd_r.pop() {
                        self.read_len = len;
                        self.i2c.it_send_start();
                        self.mode.store(Mode::Addr.into(), Ordering::Release);
                        // TODO extract sub step
                        self.sub_step = 0;
                        self.next_step();
                    } else {
                        self.end_step();
                    }
                }
                2 => {
                    if self
                        .i2c
                        .it_prepare_read(
                            self.slave_addr,
                            self.read_len as usize,
                            &mut self.sub_step,
                        )
                        .is_err()
                    {
                        break;
                    }
                    self.mode.store(Mode::Data.into(), Ordering::Release);
                    self.next_step();
                }
                3 => {
                    if let Some(data) = self.i2c.it_read(self.read_len as usize) {
                        self.data_w.push(data).unwrap();
                        self.read_len -= 1;
                        if self.read_len == 0 {
                            self.end_step();
                        }
                    } else {
                        break;
                    }
                }
                _ => {
                    self.i2c.send_stop(); // TODO move to task
                    self.finish(true);
                    break;
                }
            }
        }

        if self.step >= 3 {
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
    fn next_step(&mut self) {
        self.step += 1;
    }

    #[inline]
    fn end_step(&mut self) {
        self.step = 200;
    }

    #[inline]
    fn finish(&mut self, successful: bool) {
        self.i2c.it_reset();
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
            self.i2c.it_reset();
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
}
