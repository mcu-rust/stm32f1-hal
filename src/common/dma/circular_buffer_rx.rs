use super::*;

/// A buffer used for DMA cyclic data reception, continuously read by the user.
pub struct DmaCircularBufferRx<T: Sized, CH> {
    ch: CH,
    buf: CircularBuffer<T>,
}

impl<T, CH> DmaCircularBufferRx<T, CH>
where
    T: Sized + Copy,
    CH: DmaChannel,
{
    pub fn new(mut ch: CH, peripheral_addr: usize, buf_size: usize) -> Self {
        let buf = CircularBuffer::<T>::new(buf_size);
        ch.stop();
        ch.set_memory_buf_for_peripheral(buf.as_slice());
        ch.set_peripheral_address::<T>(peripheral_addr, false, false, true);
        ch.start();
        Self { ch, buf }
    }

    #[inline]
    pub fn read_slice<'b>(&mut self, max: usize) -> Option<&'b [T]> {
        self.buf.read_slice(self.ch.get_unprocessed_len(), max)
    }

    #[inline]
    pub fn consume(&mut self, len: usize) {
        self.buf.consume(len);
    }

    pub fn has_data(&self) -> bool {
        let recv_idx = self.buf.get_recv_index(self.ch.get_unprocessed_len());
        self.buf.read_idx != recv_idx
    }
}

pub struct CircularBuffer<T> {
    recv_buf: Vec<T>,
    read_idx: usize,
}

impl<T: Sized + Copy> CircularBuffer<T> {
    fn new(buf_size: usize) -> Self {
        let mut recv_buf = Vec::<T>::with_capacity(buf_size);
        #[allow(clippy::uninit_vec)]
        unsafe {
            recv_buf.set_len(buf_size)
        }

        Self {
            recv_buf,
            read_idx: 0,
        }
    }

    fn get_recv_index(&self, unprocessed_len: usize) -> usize {
        if unprocessed_len == 0 {
            0
        } else {
            self.recv_buf.len() - unprocessed_len
        }
    }

    fn read_slice<'b>(&mut self, unprocessed_len: usize, max: usize) -> Option<&'b [T]> {
        let recv_idx = self.get_recv_index(unprocessed_len);

        if self.read_idx == recv_idx {
            return None;
        }

        let rst;
        if recv_idx < self.read_idx {
            if max > self.recv_buf.len() - self.read_idx {
                rst = &self.recv_buf[self.read_idx..];
            } else {
                let end = self.read_idx + max;
                rst = &self.recv_buf[self.read_idx..end];
            }
        } else if max > recv_idx - self.read_idx {
            rst = &self.recv_buf[self.read_idx..recv_idx];
        } else {
            let end = self.read_idx + max;
            rst = &self.recv_buf[self.read_idx..end];
        }
        Some(unsafe { core::slice::from_raw_parts(rst.as_ptr(), rst.len()) })
    }

    fn consume(&mut self, len: usize) {
        let end = self.read_idx + len;
        if end >= self.recv_buf.len() {
            self.read_idx = end - self.recv_buf.len();
        } else {
            self.read_idx = end;
        }
    }

    // for unit test
    #[allow(dead_code)]
    fn pop_slice(&mut self, unprocessed_len: usize, max: usize) -> Option<&[T]> {
        if let Some(data) = self.read_slice(unprocessed_len, max) {
            self.consume(data.len());
            Some(data)
        } else {
            None
        }
    }

    fn as_slice(&self) -> &[T] {
        self.recv_buf.as_slice()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circular_buffer() {
        let buf_size = 13;
        let mut buf = CircularBuffer::new(buf_size);
        assert_eq!(buf.recv_buf.len(), buf_size);

        for i in 0..buf_size {
            buf.recv_buf[i] = i as u8;
        }

        assert_eq!(
            buf.pop_slice(5, usize::MAX),
            Some([0u8, 1, 2, 3, 4, 5, 6, 7].as_slice())
        );
        assert_eq!(buf.pop_slice(5, usize::MAX), None);
        // Single wraparound
        assert_eq!(
            buf.pop_slice(0, usize::MAX),
            Some([8u8, 9, 10, 11, 12].as_slice())
        );
        assert_eq!(buf.pop_slice(0, usize::MAX), None);
        assert_eq!(buf.pop_slice(buf_size, usize::MAX), None);
        // small max
        assert_eq!(buf.pop_slice(5, 5), Some([0u8, 1, 2, 3, 4].as_slice()));
        assert_eq!(buf.pop_slice(5, 5), Some([5u8, 6, 7].as_slice()));
        assert_eq!(buf.pop_slice(5, 5), None);
        assert_eq!(
            buf.pop_slice(0, usize::MAX),
            Some([8u8, 9, 10, 11, 12].as_slice())
        );
        // Multiple wraparounds
        assert_eq!(
            buf.pop_slice(5, usize::MAX),
            Some([0u8, 1, 2, 3, 4, 5, 6, 7].as_slice())
        );
        assert_eq!(
            buf.pop_slice(10, usize::MAX),
            Some([8u8, 9, 10, 11, 12].as_slice())
        );
        assert_eq!(buf.pop_slice(10, usize::MAX), Some([0u8, 1, 2].as_slice()));
        assert_eq!(buf.pop_slice(10, usize::MAX), None);
    }
}
