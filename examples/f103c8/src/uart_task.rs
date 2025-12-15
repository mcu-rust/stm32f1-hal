use crate::{
    embedded_io::{Read, Write},
    hal::ringbuf::*,
    os::*,
};

pub struct UartPollTask<W: Write, R: Read> {
    tx: W,
    rx: R,
    w: Producer<u8>,
    r: Consumer<u8>,
}

impl<W, R> UartPollTask<W, R>
where
    W: Write,
    R: Read,
{
    pub fn new(size: usize, tx: W, rx: R) -> Self {
        let (w, r) = RingBuffer::new(size);
        Self { tx, rx, w, r }
    }

    pub fn poll(&mut self) {
        if let Some(mut chunk) = self.w.get_write_chunk_uninit() {
            if let Ok(size) = self.rx.read(chunk.get_mut_slice()) {
                unsafe { chunk.commit(size) }
            }
        }

        if let Some(chunk) = self.r.get_read_chunk() {
            if let Ok(size) = self.tx.write(chunk.get_slice()) {
                chunk.commit(size);
            }
        }
    }
}
