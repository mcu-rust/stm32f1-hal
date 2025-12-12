use super::{utils::*, *};
use crate::common::{
    bus_device::*,
    os_trait::Mutex,
    ringbuf::{Consumer, Producer, RingBuffer},
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
    I2C: I2cPeriph,
{
    pub fn new(
        i2c: [I2C; 3],
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
        let [i2c1, i2c2, i2c3] = i2c;
        let it = I2cBusInterruptHandler {
            i2c: i2c2,
            mode: Arc::clone(&mode),
            cmd_r,
            data_w,
            step: 0,
            read_len: 0,
            slave_addr: 0,
            notifier: notifier.clone(),
        };
        let it_err = I2cBusErrorInterruptHandler {
            i2c: i2c3,
            err_code: Arc::clone(&err_code),
            notifier,
        };
        (
            Self {
                i2c: i2c1,
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

    pub fn write_read(&mut self, slave_addr: u8, data: &[u8], buf: &mut [u8]) -> Result<(), Error> {
        if buf.is_empty() && data.is_empty() {
            return Err(Error::Other);
        }

        // check stop, timeout > 25ms
        if self
            .waiter
            .wait_with(OS::O, 26.millis(), 16, || {
                self.i2c.is_stopped(true).then_some(())
            })
            .is_none()
        {
            return Err(Error::Timeout);
        }

        self.i2c.it_reset();

        // prepare
        self.seq_id = self.seq_id.wrapping_add(1);
        self.cmd_w.push(Command::Start(self.seq_id)).unwrap();
        self.cmd_w.push(Command::SlaveAddr(slave_addr)).unwrap();
        if !data.is_empty() {
            self.cmd_w.push(Command::WriteMode).unwrap();
            for d in data {
                self.cmd_w.push(Command::Data(*d)).unwrap();
            }
        }
        if !buf.is_empty() {
            self.cmd_w.push(Command::ReadMode).unwrap();
            self.cmd_w.push(Command::Len(buf.len() as u8)).unwrap();
        }
        while self.data_r.pop().is_ok() {}
        // reset error code
        self.err_code.store(0, Ordering::Release);
        self.mode
            .store(Mode::Start(self.seq_id).into(), Ordering::Release);
        self.i2c.it_send_start();

        // TODO calculate timeout
        let mut i = 0;
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

            while i < buf.len()
                && let Ok(data) = self.data_r.pop()
            {
                buf[i] = data;
                i += 1;
            }
            None
        });

        match rst {
            None => Err(Error::Timeout),
            Some(rst) => {
                if i != buf.len() {
                    Err(Error::Other)
                } else {
                    rst
                }
            }
        }
    }
}

// Handler ----------------------------------------------------------

pub struct I2cBusInterruptHandler<OS: OsInterface, I2C> {
    i2c: I2C,
    mode: Arc<AtomicU16>,
    cmd_r: Consumer<Command>,
    data_w: Producer<u8>,
    notifier: OS::Notifier,

    step: u8,
    read_len: u8,
    slave_addr: u8,
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
                    self.step = match cmd {
                        Command::ReadMode => 4, // jump to read
                        _ => 0,
                    };
                    mode = Mode::Work;
                    self.mode.store(mode.into(), Ordering::Relaxed);
                }
            }
        }

        match mode {
            Mode::Start(_) | Mode::Stop | Mode::Success => {
                if self.i2c.get_flag(Flag::Busy) {
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
                    if !self.i2c.it_send_slave_addr(self.slave_addr, false) {
                        break;
                    }
                    self.mode.store(Mode::Addr.into(), Ordering::Release);
                    self.next();
                }
                1 => {
                    if !self.i2c.it_start_write_data() {
                        break;
                    }
                    self.mode.store(Mode::Data.into(), Ordering::Release);
                    self.next();
                }
                2 => {
                    match self.i2c.it_write_with(|| match self.cmd_r.pop() {
                        Ok(Command::Data(data)) => Some(data),
                        _ => None,
                    }) {
                        Some(true) | None => break,
                        Some(false) => self.next(),
                    }
                }
                3 => {
                    // restart or stop
                    if let Ok(Command::Len(len)) = self.cmd_r.pop() {
                        self.read_len = len;
                        self.i2c.it_send_start();
                        self.next();
                    } else {
                        self.end();
                    }
                }
                4 => {
                    if !self.i2c.it_send_slave_addr(self.slave_addr, true) {
                        break;
                    }
                    self.mode.store(Mode::Addr.into(), Ordering::Release);
                    if let Ok(Command::Len(len)) = self.cmd_r.pop() {
                        self.read_len = len;
                    }
                    self.next();
                }
                5 => {
                    if !self.i2c.it_start_read_data(self.read_len as usize) {
                        break;
                    }
                    self.mode.store(Mode::Data.into(), Ordering::Release);
                    self.next();
                }
                6 => {
                    if let Some(data) = self.i2c.it_read(self.read_len as usize) {
                        self.data_w.push(data).unwrap();
                        self.read_len -= 1;
                        if self.read_len == 0 {
                            self.next();
                        }
                    } else {
                        break;
                    }
                }
                _ => {
                    self.i2c.send_stop();
                    self.finish(true);
                    break;
                }
            }
        }

        if self.step >= 6 {
            self.notifier.notify();
        }
    }

    fn prepare_cmd(&mut self, seq_id: u8) -> bool {
        // Clean old commands
        while let Ok(cmd) = self.cmd_r.pop() {
            if cmd == Command::Start(seq_id) {
                if let Ok(Command::SlaveAddr(slave_addr)) = self.cmd_r.pop() {
                    self.slave_addr = slave_addr;
                    return true;
                }
            }
        }
        false
    }

    #[inline]
    fn next(&mut self) {
        self.step += 1;
    }

    #[inline]
    fn end(&mut self) {
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
