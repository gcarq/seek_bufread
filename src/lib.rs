// Copyright 2016 gcarq. See the LICENSE file at the top-level
// directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::fmt;
use std::io::{self, BufRead, Read, Seek, SeekFrom};

const DEFAULT_BUF_SIZE: usize = 8 * 1024;

/// The `BufReader` struct adds buffering with seeking support to any reader.
///
/// It can be excessively inefficient to work directly with a `Read` instance.
/// For example, every call to `read` on `TcpStream` results in a system call.
/// A `BufReader` performs large, infrequent reads on the underlying `Read`
/// and maintains an in-memory buffer of the results.
///
/// This implementation of `BufRead` respects the internal buffer on `seek` calls,
/// which leads to a huge performance gain in some circumstances.
///
pub struct BufReader<R> {
    inner: R,              // internal reader
    buf: Box<[u8]>,        // internal buffer
    buf_pos: usize,        // position within buf
    cap: usize,            // buf capacity
    absolute_pos: u64,     // absolute position
}

impl<R: Read> BufReader<R> {

    /// Creates a new `BufReader` with a default buffer capacity (8192 bytes).
    pub fn new(inner: R) -> BufReader<R> {
        BufReader::with_capacity(DEFAULT_BUF_SIZE, inner)
    }

    /// Creates a new `BufReader` with the specified buffer capacity.
    pub fn with_capacity(cap: usize, inner: R) -> BufReader<R> {
        BufReader {
            inner: inner,
            buf: vec![0; cap].into_boxed_slice(),
            buf_pos: 0,
            cap: 0,
            absolute_pos: 0,
        }
    }

    /// Returns the absolute file pointer position.
    pub fn position(&self) -> u64 { self.absolute_pos }

    /// Returns the total buffer capacity.
    pub fn capacity(&self) -> usize { self.cap }

    /// Returns the current number of bytes available in the buffer.
    pub fn available(&self) -> usize {
        match self.cap.checked_sub(self.buf_pos) {
            Some(remaining) => remaining,
            None => 0
        }
    }

    fn reset_buffer(&mut self) {
        self.buf_pos = self.cap;
    }
}

impl<R: Read> Read for BufReader<R> {
    /// Reads the next available bytes from buffer or inner stream.
    /// Doesn't guarantee the whole buffer is filled.
    /// Returns number of read bytes.
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n_exp = buf.len();
        let mut n_total = 0;
        while n_total < n_exp {
            let n_read = try!(try!(self.fill_buf()).read(&mut buf[n_total..]));
            if n_read == 0 {
                break;
            }
            self.consume(n_read);
            n_total += n_read;
        }
        Ok(n_total)
    }
}

impl<R: Read> BufRead for BufReader<R> {
    #[inline]
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        // If we've reached the end of our internal buffer then we need to fetch
        // some more data from the underlying reader.
        if self.cap == self.buf_pos {
            self.cap = try!(self.inner.read(&mut self.buf));
            self.buf_pos = 0;
        }
        Ok(&self.buf[self.buf_pos..self.cap])
    }

    #[inline]
    fn consume(&mut self, amt: usize) {
        self.buf_pos += amt;
        self.absolute_pos += amt as u64;
    }
}

impl<R: Read + Seek> Seek for BufReader<R> {
    /// Seek to an offset, in bytes, in the buffer or the underlying reader.
    ///
    /// The position used for seeking with `SeekFrom::Current(_)` is the
    /// current position of the underlying reader plus the current position
    /// in the internal buffer.
    ///
    /// Calling `.unwrap()` immediately after a seek doesn't guarantee
    /// the underlying reader at the same position!
    ///
    /// See `std::io::Seek` for more details.
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match pos {
            SeekFrom::Current(n) => {
                if n < 0 {
                    // Seek backwards
                    let n = n.abs() as u64;
                    match self.buf_pos.checked_sub(n as usize) {
                        Some(_) => {
                            // Seek our internal buffer
                            self.absolute_pos -= n;
                            self.buf_pos -= n as usize;
                        }
                        None => {
                            // Seek in our internal buffer first, and the remaining offset in the inner reader
                            self.absolute_pos =
                                try!(self.inner.seek(SeekFrom::Start(self.absolute_pos - n)));
                            self.reset_buffer();
                        }
                    }
                } else {
                    // Seek forwards
                    let n = n as usize;
                    let remaining = self.available();
                    if remaining > 0 {
                        if remaining.checked_sub(n).is_some() {
                            // Seek in our internal buffer
                            self.consume(n);
                        } else {
                            // Out of scope. Seek inner reader to new position and reset buffer
                            self.absolute_pos =
                                try!(self.inner.seek(SeekFrom::Start(self.absolute_pos + n as u64)));
                            self.reset_buffer();
                        }
                    } else {
                        // Buffer is full. Seek inner reader to new position
                        self.absolute_pos =
                            try!(self.inner.seek(SeekFrom::Start(self.absolute_pos + n as u64)));
                    }
                }
            }
            SeekFrom::Start(_) | SeekFrom::End(_) => {
                self.absolute_pos = try!(self.inner.seek(pos));
                self.reset_buffer();
            }
        }
        Ok(self.absolute_pos)
    }
}

impl<R> fmt::Debug for BufReader<R> where R: fmt::Debug + Read {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("BufReader")
            .field("reader", &self.inner)
            .field("available", &self.available())
            .field("capacity", &self.cap)
            .field("position", &self.absolute_pos)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Cursor, Read, Seek, SeekFrom};

    #[test]
    fn default_behaviour() {
        let mut reader = BufReader::new(Cursor::new([5, 6, 7, 0, 1, 2, 3, 4]));

        let mut buf = [0; 8];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [5, 6, 7, 0, 1, 2, 3, 4]);

        let mut buf = [0; 8];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn default_behaviour_std() {
        let mut reader = io::BufReader::new(Cursor::new([5, 6, 7, 0, 1, 2, 3, 4]));

        let mut buf = [0; 8];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [5, 6, 7, 0, 1, 2, 3, 4]);

        let mut buf = [0; 8];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn small_capacity() {
        let inner = Cursor::new([5, 6, 7, 0, 1, 2, 3, 4]);
        let mut reader = BufReader::with_capacity(2, inner);

        let mut buf = [0, 0, 0];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [5, 6, 7]);

        let mut buf = [0, 0];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [0, 1]);

        let mut buf = [0];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [2]);
    }

    #[test]
    fn small_capacity_std() {
        let inner = Cursor::new([5, 6, 7, 0, 1, 2, 3, 4]);
        let mut reader = io::BufReader::with_capacity(2, inner);

        let mut buf = [0, 0, 0];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [5, 6, 7]);

        let mut buf = [0, 0];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [0, 1]);

        let mut buf = [0];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [2]);
    }

    #[test]
    fn seek_start() {
        let inner = Cursor::new([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        let mut reader = BufReader::with_capacity(2, inner);

        reader.seek(SeekFrom::Start(10)).unwrap();
        let mut buf = [0; 8];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [10, 11, 12, 13, 14, 15, 16, 0]);

        reader.seek(SeekFrom::Start(13)).unwrap();
        let mut buf = [0; 8];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [13, 14, 15, 16, 0, 0, 0, 0]);
    }

    #[test]
    fn seek_start_std() {
        let inner = Cursor::new([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        let mut reader = io::BufReader::with_capacity(2, inner);

        reader.seek(SeekFrom::Start(10)).unwrap();
        let mut buf = [0; 8];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [10, 11, 12, 13, 14, 15, 16, 0]);

        reader.seek(SeekFrom::Start(13)).unwrap();
        let mut buf = [0; 8];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [13, 14, 15, 16, 0, 0, 0, 0]);
    }

    #[test]
    fn seek_current_positive() {
        let inner = Cursor::new([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        let mut reader = BufReader::with_capacity(2, inner);

        reader.seek(SeekFrom::Current(4)).unwrap();
        let mut buf = [0; 8];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [4, 5, 6, 7, 8, 9, 10, 11]);

        reader.seek(SeekFrom::Current(6)).unwrap();
        let mut buf = [0; 8];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn seek_current_positive_std() {
        let inner = Cursor::new([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        let mut reader = io::BufReader::with_capacity(2, inner);

        reader.seek(SeekFrom::Current(4)).unwrap();
        let mut buf = [0; 8];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [4, 5, 6, 7, 8, 9, 10, 11]);

        reader.seek(SeekFrom::Current(6)).unwrap();
        let mut buf = [0; 8];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn seek_current_negative() {
        let inner = Cursor::new([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        let mut reader = BufReader::with_capacity(2, inner);

        reader.seek(SeekFrom::Current(4)).unwrap();
        let mut buf = [0; 8];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [4, 5, 6, 7, 8, 9, 10, 11]);

        reader.seek(SeekFrom::Current(-2)).unwrap();
        let mut buf = [0; 8];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [10, 11, 12, 13, 14, 15, 16, 0]);
    }

    #[test]
    fn seek_current_negative_std() {
        let inner = Cursor::new([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        let mut reader = io::BufReader::with_capacity(2, inner);

        reader.seek(SeekFrom::Current(4)).unwrap();
        let mut buf = [0; 8];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [4, 5, 6, 7, 8, 9, 10, 11]);

        reader.seek(SeekFrom::Current(-2)).unwrap();
        let mut buf = [0; 8];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [10, 11, 12, 13, 14, 15, 16, 0]);
    }

    #[test]
    fn seek_end() {
        let inner = Cursor::new([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        let mut reader = BufReader::with_capacity(2, inner);

        reader.seek(SeekFrom::End(-6)).unwrap();
        let mut buf = [0; 8];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [11, 12, 13, 14, 15, 16, 0, 0]);

        reader.seek(SeekFrom::End(0)).unwrap();
        let mut buf = [0; 8];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn seek_end_std() {
        let inner = Cursor::new([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        let mut reader = io::BufReader::with_capacity(2, inner);

        reader.seek(SeekFrom::End(-6)).unwrap();
        let mut buf = [0; 8];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [11, 12, 13, 14, 15, 16, 0, 0]);

        reader.seek(SeekFrom::End(0)).unwrap();
        let mut buf = [0; 8];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [0, 0, 0, 0, 0, 0, 0, 0]);
    }
}

#[cfg(bench)]
mod bench {
    use super::*;
    use test::Bencher;
    use std::fs::{self, File};
    use std::io::{self, Cursor, Read, Write, Seek, SeekFrom};

    #[bench]
    fn read_10mb_default_from_cursor(b: &mut Bencher) {
        b.iter(|| {
            let inner = Cursor::new(vec![1; 10000000]);
            let mut reader = BufReader::new(inner);

            let mut buf: Vec<u8> = Vec::with_capacity(10000000);
            reader.read_to_end(&mut buf).unwrap();
        });
    }

    #[bench]
    fn read_10mb_default_from_cursor_std(b: &mut Bencher) {
        b.iter(|| {
            let inner = Cursor::new(vec![1; 10000000]);
            let mut reader = io::BufReader::new(inner);

            let mut buf: Vec<u8> = Vec::with_capacity(10000000);
            reader.read_to_end(&mut buf).unwrap();
        });
    }

    #[bench]
    fn read_10mb_default_from_file(b: &mut Bencher) {
        let mut f = File::create("foo.txt").unwrap();
        f.write_all(&vec![0; 10000000]).unwrap();
        b.iter(|| {
            let mut reader = BufReader::new(File::open("foo.txt").unwrap());

            let mut buf: Vec<u8> = Vec::with_capacity(10000000);
            reader.read_to_end(&mut buf).unwrap();
        });
        fs::remove_file("foo.txt").unwrap();
    }

    #[bench]
    fn read_10mb_default_from_file_std(b: &mut Bencher) {
        let mut f = File::create("foo.txt").unwrap();
        f.write_all(&vec![0; 10000000]).unwrap();
        b.iter(|| {
            let mut reader = io::BufReader::new(File::open("foo.txt").unwrap());

            let mut buf: Vec<u8> = Vec::with_capacity(10000000);
            reader.read_to_end(&mut buf).unwrap();
        });
        fs::remove_file("foo.txt").unwrap();
    }

    #[bench]
    fn read_10mb_halfbuf_from_file(b: &mut Bencher) {
        let mut f = File::create("foo.txt").unwrap();
        f.write_all(&vec![0; 10000000]).unwrap();
        b.iter(|| {
            let mut reader = BufReader::with_capacity(5000000, File::open("foo.txt").unwrap());

            let mut buf: Vec<u8> = Vec::with_capacity(10000000);
            reader.read_to_end(&mut buf).unwrap();
        });
        fs::remove_file("foo.txt").unwrap();
    }

    #[bench]
    fn read_10mb_halfbuf_from_file_std(b: &mut Bencher) {
        let mut f = File::create("foo.txt").unwrap();
        f.write_all(&vec![0; 10000000]).unwrap();
        b.iter(|| {
            let mut reader = io::BufReader::with_capacity(5000000, File::open("foo.txt").unwrap());

            let mut buf: Vec<u8> = Vec::with_capacity(10000000);
            reader.read_to_end(&mut buf).unwrap();
        });
        fs::remove_file("foo.txt").unwrap();
    }

    #[bench]
    fn read_10mb_fullbuf_from_file(b: &mut Bencher) {
        let mut f = File::create("foo.txt").unwrap();
        f.write_all(&vec![0; 10000000]).unwrap();
        b.iter(|| {
            let mut reader = BufReader::with_capacity(10000000, File::open("foo.txt").unwrap());

            let mut buf: Vec<u8> = Vec::with_capacity(10000000);
            reader.read_to_end(&mut buf).unwrap();
        });
        fs::remove_file("foo.txt").unwrap();
    }

    #[bench]
    fn read_10mb_fullbuf_from_file_std(b: &mut Bencher) {
        let mut f = File::create("foo.txt").unwrap();
        f.write_all(&vec![0; 10000000]).unwrap();
        b.iter(|| {
            let mut reader = io::BufReader::with_capacity(10000000, File::open("foo.txt").unwrap());

            let mut buf: Vec<u8> = Vec::with_capacity(10000000);
            reader.read_to_end(&mut buf).unwrap();
        });
        fs::remove_file("foo.txt").unwrap();
    }

    #[bench]
    fn read_seek_10mb_halfbuf_from_file(b: &mut Bencher) {
        let mut f = File::create("foo.txt").unwrap();
        f.write_all(&vec![0; 10000000]).unwrap();
        b.iter(|| {
            let mut reader = BufReader::with_capacity(5000000, File::open("foo.txt").unwrap());
            let mut buf: Vec<u8> = Vec::with_capacity(100000);
            for i in 0...100 {
                reader.seek(SeekFrom::Current(i * 10)).unwrap();
                reader.read(&mut buf).unwrap();
            }

        });
        fs::remove_file("foo.txt").unwrap();
    }

    #[bench]
    fn read_seek_10mb_halfbuf_from_file_std(b: &mut Bencher) {
        let mut f = File::create("foo.txt").unwrap();
        f.write_all(&vec![0; 10000000]).unwrap();
        b.iter(|| {
            let mut reader = io::BufReader::with_capacity(5000000, File::open("foo.txt").unwrap());
            let mut buf: Vec<u8> = Vec::with_capacity(100000);
            for i in 0...100 {
                reader.seek(SeekFrom::Current(i * 10)).unwrap();
                reader.read(&mut buf).unwrap();
            }

        });
        fs::remove_file("foo.txt").unwrap();
    }

}
