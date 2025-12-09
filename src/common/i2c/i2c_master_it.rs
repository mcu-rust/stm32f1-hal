use super::utils::*;
use super::*;
use crate::common::os::*;
use crate::common::ringbuf::{Consumer, Producer, RingBuffer};
use core::sync::atomic::{AtomicU16, Ordering};

pub struct I2cBusInterrupt<T> {
    i2c: T,
    mode: Arc<AtomicU16>,
    err_code: Arc<AtomicU16>,
    cmd_w: Producer<Command>,
    data_r: Consumer<u8>,
    seq_id: u8,
}

impl<T> I2cBusInterrupt<T>
where
    T: I2cPeriph,
{
    pub fn new(
        i2c: [T; 3],
        buff_size: usize,
    ) -> (
        Self,
        I2cBusInterruptHandler<T>,
        I2cBusErrorInterruptHandler<T>,
    ) {
        let (data_w, data_r) = RingBuffer::<u8>::new(buff_size);
        let (cmd_w, cmd_r) = RingBuffer::<Command>::new(buff_size + 4);
        let mode = Arc::new(AtomicU16::new(0));
        let err_code = Arc::new(AtomicU16::new(0));
        let [i2c1, i2c2, i2c3] = i2c;
        let it = I2cBusInterruptHandler {
            i2c: i2c2,
            mode: Arc::clone(&mode),
            cmd_r,
            data_w,
            step: 0,
            data_len: 0,
            slave_addr: 0,
        };
        let it_err = I2cBusErrorInterruptHandler {
            i2c: i2c3,
            err_code: Arc::clone(&err_code),
        };
        (
            Self {
                i2c: i2c1,
                mode,
                err_code,
                cmd_w,
                data_r,
                seq_id: 0,
            },
            it,
            it_err,
        )
    }

    pub fn write(&mut self, slave_addr: u8, reg_addr: u8, data: &[u8]) -> Result<(), Error> {
        if data.is_empty() {
            return Err(Error::Other);
        }

        // let mut t = self.timeout.start();
        // time > 25ms
        while self.i2c.is_busy() {
            // if t.timeout() {
            //     return;
            // }
        }

        self.i2c.it_reset();

        // Prepare
        self.seq_id = self.seq_id.wrapping_add(1);
        self.cmd_w.push(Command::Start(self.seq_id)).unwrap();
        self.cmd_w.push(Command::SlaveAddr(slave_addr)).unwrap();
        self.cmd_w.push(Command::Data(reg_addr)).unwrap();
        for d in data {
            self.cmd_w.push(Command::Data(*d)).unwrap();
        }
        self.mode
            .store(Mode::StartWrite(self.seq_id).into(), Ordering::Release);
        // reset error code
        self.err_code.store(0, Ordering::Release);
        self.i2c.it_send_start();

        while Mode::Stop != self.mode.load(Ordering::Acquire).into() {
            // if t.timeout() {
            //     return;
            // }
        }

        // get error code
        if let Some(err) = int_to_err(self.err_code.load(Ordering::Acquire)) {
            let mode: Mode = self.mode.load(Ordering::Acquire).into();
            if mode == Mode::WriteAddr {
                Err(err.nack_addr())
            } else if mode == Mode::WriteData {
                Err(err.nack_data())
            } else {
                Err(err)
            }
        } else {
            Ok(())
        }
    }

    pub fn read(&mut self, slave_addr: u8, reg_addr: u8, buff: &mut [u8]) -> Result<(), Error> {
        if buff.is_empty() {
            return Err(Error::Other);
        }

        // let mut t = self.timeout.start();
        // time > 25ms
        while self.i2c.is_busy() {
            // if t.timeout() {
            //     return;
            // }
        }

        self.i2c.it_reset();

        // Prepare
        self.seq_id = self.seq_id.wrapping_add(1);
        self.cmd_w.push(Command::Start(self.seq_id)).unwrap();
        self.cmd_w.push(Command::SlaveAddr(slave_addr)).unwrap();
        self.cmd_w.push(Command::Data(reg_addr)).unwrap();
        self.cmd_w.push(Command::Len(buff.len() as u8)).unwrap();
        self.mode
            .store(Mode::StartRead(self.seq_id).into(), Ordering::Release);
        while self.data_r.pop().is_ok() {}
        // reset error code
        self.err_code.store(0, Ordering::Release);
        self.i2c.it_send_start();

        for b in buff.as_mut() {
            loop {
                if let Ok(data) = self.data_r.pop() {
                    *b = data;
                    break;
                }
                // else if t.timeout() {
                //     return;
                // }
            }
        }

        // get error code
        if let Some(err) = int_to_err(self.err_code.load(Ordering::Acquire)) {
            let mode: Mode = self.mode.load(Ordering::Acquire).into();
            if mode == Mode::ReadAddr {
                Err(err.nack_addr())
            } else if mode == Mode::ReadData {
                Err(err.nack_data())
            } else {
                Err(err)
            }
        } else {
            Ok(())
        }
    }
}

// Handler ----------------------------------------------------------

pub struct I2cBusInterruptHandler<T> {
    i2c: T,
    mode: Arc<AtomicU16>,
    cmd_r: Consumer<Command>,
    data_w: Producer<u8>,

    step: u8,
    data_len: u8,
    slave_addr: u8,
}

impl<T> I2cBusInterruptHandler<T>
where
    T: I2cPeriph,
{
    pub fn handler(&mut self) {
        let mut mode = Mode::from(self.mode.load(Ordering::Acquire));
        match mode {
            Mode::StartWrite(seq_id) => {
                if self.prepare_cmd(seq_id) {
                    mode = Mode::Write;
                    self.mode.store(mode.into(), Ordering::Relaxed);
                    self.step = 0;
                }
            }
            Mode::StartRead(seq_id) => {
                if self.prepare_cmd(seq_id) {
                    mode = Mode::Read;
                    self.mode.store(mode.into(), Ordering::Relaxed);
                    self.step = 0;
                }
            }
            _ => (),
        }

        match mode {
            Mode::Write | Mode::WriteAddr | Mode::WriteData => self.write(),
            Mode::Read | Mode::ReadAddr | Mode::ReadData => self.read(),
            Mode::Stop => self.i2c.it_reset(),
            Mode::StartWrite(_) | Mode::StartRead(_) => (),
        }
    }

    fn write(&mut self) {
        match self.step {
            0 => {
                if self.i2c.it_send_slave_addr(self.slave_addr, false) {
                    self.mode.store(Mode::WriteAddr.into(), Ordering::Release);
                    self.next();
                }
            }
            1 => {
                if self.i2c.it_start_write_data() {
                    self.mode.store(Mode::WriteData.into(), Ordering::Release);
                    self.next();
                }
            }
            2 => {
                // first data is the register address
                if let Some(false) = self.i2c.it_write_with(|| {
                    if let Ok(Command::Data(data)) = self.cmd_r.pop() {
                        Some(data)
                    } else {
                        None
                    }
                }) {
                    self.finish();
                    self.i2c.send_stop();
                }
            }
            _ => {}
        };
    }

    fn read(&mut self) {
        match self.step {
            0 => {
                if self.i2c.it_send_slave_addr(self.slave_addr, false) {
                    self.mode.store(Mode::ReadAddr.into(), Ordering::Release);
                    self.next();
                }
            }
            1 => {
                if self.i2c.it_start_write_data() {
                    self.mode.store(Mode::ReadData.into(), Ordering::Release);
                    self.next();
                }
            }
            2 => {
                // first data is the register address
                if let Some(true) = self.i2c.it_write_with(|| {
                    if let Ok(Command::Data(data)) = self.cmd_r.pop() {
                        Some(data)
                    } else {
                        None
                    }
                }) {
                    // restart
                    self.i2c.it_send_start();
                    self.next();
                }
            }
            3 => {
                if self.i2c.it_send_slave_addr(self.slave_addr, true) {
                    self.mode.store(Mode::ReadAddr.into(), Ordering::Release);
                    if let Ok(Command::Len(len)) = self.cmd_r.pop() {
                        self.data_len = len;
                    }
                    self.next();
                }
            }
            4 => {
                if self.i2c.it_start_read_data(self.data_len as usize) {
                    self.mode.store(Mode::ReadData.into(), Ordering::Release);
                    self.next();
                }
            }
            5 => {
                if let Some(data) = self.i2c.it_read(self.data_len as usize) {
                    self.data_w.push(data).unwrap();
                    self.data_len -= 1;
                    if self.data_len == 0 {
                        self.finish();
                        self.i2c.send_stop();
                    }
                }
            }
            _ => {}
        };
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
    fn finish(&mut self) {
        self.i2c.it_reset();
        // Clean old commands
        while self.cmd_r.pop().is_ok() {}
        // self.mode.stop();
        self.mode.store(Mode::Stop.into(), Ordering::Release);
    }
}

pub struct I2cBusErrorInterruptHandler<T> {
    i2c: T,
    err_code: Arc<AtomicU16>,
}

impl<T> I2cBusErrorInterruptHandler<T>
where
    T: I2cPeriph,
{
    pub fn handler(&mut self) -> bool {
        let rst = if let Some(err) = self.i2c.get_and_clean_error() {
            self.err_code
                .store(err_to_int(Some(err)), Ordering::Release);
            true
        } else {
            false
        };

        self.i2c.it_reset();
        rst
    }
}
