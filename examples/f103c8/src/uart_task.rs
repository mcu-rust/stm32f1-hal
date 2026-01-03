use crate::{
    embedded_io::{BufRead, Write},
    os::*,
};

pub struct UartLoopBackTask<W: Write, R: BufRead> {
    tx: W,
    rx: R,
}

impl<W, R> UartLoopBackTask<W, R>
where
    W: Write,
    R: BufRead,
{
    pub fn new(tx: W, rx: R) -> Self {
        Self { tx, rx }
    }

    pub fn poll(&mut self) {
        if let Ok(buf) = self.rx.fill_buf() {
            if let Ok(size) = self.tx.write(buf) {
                self.rx.consume(size);
            }
        }
    }
}
