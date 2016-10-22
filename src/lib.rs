// Original work Copyright 2013 The Rust Project Developers.
// Modified work Copyright 2016 gcarq.
// See the LICENSE file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! <a href="https://github.com/gcarq/seek_bufread">
//! <img src="https://s3.amazonaws.com/github/ribbons/forkme_right_red_aa0000.png" style="position: absolute; top: 0; right: 0; border: 0; margin-top: 55px" alt="Fork me on GitHub">
//! </a>
//!
//! The `BufReader` is a drop-in replacement for `std::io::BufReader` with seeking support.
//! If `.seek(SeekFrom::Current(n))` is called and `n` is in range of the internal buffer the
//! underlying reader is not invoked. This has the side effect that you can no longer access
//! the underlying buffer directly after being consumed by `BufReader`,
//! because its position could be out of sync.
//!
//! # Examples
//!
//! ```
//! use std::io::{self, Cursor, Read, Seek, SeekFrom};
//! use seek_bufread::BufReader;
//!
//! let inner = Cursor::new([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
//! let mut reader = BufReader::new(inner);
//!
//! reader.seek(SeekFrom::Current(4)).unwrap();
//! let mut buf = [0; 8];
//!
//! // read bytes from internal buffer
//! reader.read(&mut buf).unwrap();;
//! assert_eq!(buf, [4, 5, 6, 7, 8, 9, 10, 11]);
//! ```

//#![feature(test)]
//extern crate test;

use std::fmt;
use std::io::{self, BufRead, Read, Seek, SeekFrom};

const DEFAULT_BUF_SIZE: usize = 8 * 1024;

/// The `BufReader` struct adds buffering to any reader.
///
/// It can be excessively inefficient to work directly with a `Read` instance.
/// For example, every call to `read` on `TcpStream` results in a system call.
/// A `BufReader` performs large, infrequent reads on the underlying `Read`
/// and maintains an in-memory buffer of the results.
///
/// # Examples
///
/// ```
/// use std::io::prelude::*;
/// use std::fs::File;
/// use seek_bufread::BufReader;
///
/// # fn foo() -> std::io::Result<()> {
/// let mut f = try!(File::open("log.txt"));
/// let mut reader = BufReader::new(f);
///
/// let mut line = String::new();
/// let len = try!(reader.read_line(&mut line));
/// println!("First line is {} bytes long", len);
/// # Ok(())
/// # }
/// ```
pub struct BufReader<R> {
    inner: R,              // internal reader
    buf: Box<[u8]>,        // internal buffer
    buf_pos: usize,        // position within buf
    cap: usize,            // buf capacity
    absolute_pos: u64,     // absolute position
}

impl<R: Read + Seek> BufReader<R> {

    /// Creates a new `BufReader` with a default buffer capacity (8192 bytes).
    ///
    /// # Examples
    ///
    /// ```
    /// use std::fs::File;
    /// use seek_bufread::BufReader;
    ///
    /// # fn foo() -> std::io::Result<()> {
    /// let mut f = try!(File::open("log.txt"));
    /// let mut reader = BufReader::new(f);
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(inner: R) -> BufReader<R> {
        BufReader::with_capacity(DEFAULT_BUF_SIZE, inner)
    }

    /// Creates a new `BufReader` with the specified buffer capacity.
    ///
    /// # Examples
    ///
    /// Creating a buffer with ten bytes of capacity:
    ///
    /// ```
    /// use std::fs::File;
    /// use seek_bufread::BufReader;
    ///
    /// # fn foo() -> std::io::Result<()> {
    /// let mut f = try!(File::open("log.txt"));
    /// let mut reader = BufReader::with_capacity(10, f);
    /// # Ok(())
    /// # }
    /// ```
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

    /// Returns the current number of remaining bytes available in the buffer.
    pub fn available(&self) -> usize {
        self.cap.checked_sub(self.buf_pos).unwrap_or(0)
    }

    /// Consumes `self`, synchronizes the inner reader position and returns the inner reader.
    pub fn into_inner(mut self) -> io::Result<R> {
        // Sync position of internal reader
        try!(self.inner.seek(SeekFrom::Start(self.absolute_pos)));
        Ok(self.inner)
    }

    /// Syncs the position of our underlying reader and empties the buffer
    fn sync_and_flush(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.buf_pos = self.cap;
        self.absolute_pos = try!(self.inner.seek(pos));
        Ok(self.absolute_pos)
    }

    /// Seeks `n` bytes backwards from current position
    fn seek_backward(&mut self, n: usize) -> io::Result<u64> {
        if self.buf_pos - n > 0 {
            // Seek our internal buffer
            self.absolute_pos -= n as u64;
            self.buf_pos -= n;
            Ok(self.absolute_pos)
        } else {
            // Out of scope. Seek inner reader to new position and reset buffer
            self.sync_and_flush(SeekFrom::Current(n as i64 * -1))
        }
    }

    /// Seeks `n` bytes forwards from current position
    fn seek_forward(&mut self, n: usize) -> io::Result<u64> {
        if self.available().checked_sub(n).is_some() {
            self.consume(n);
            Ok(self.absolute_pos)
        } else {
            // Out of scope. Seek inner reader to new position and reset buffer
            self.sync_and_flush(SeekFrom::Current(n as i64))
        }
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
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        // If we've reached the end of our internal buffer then we need to fetch
        // some more data from the underlying reader.
        if self.cap == self.buf_pos {
            self.cap = try!(self.inner.read(&mut self.buf));
            self.buf_pos = 0;
        }
        Ok(&self.buf[self.buf_pos..self.cap])
    }

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
                match n > 0 {
                    true => self.seek_forward(n as usize),
                    false => self.seek_backward(n.abs() as usize)
                }
            }
            SeekFrom::Start(_) | SeekFrom::End(_) => {
                self.sync_and_flush(pos)
            }
        }
    }
}

impl<R> fmt::Debug for BufReader<R> where R: fmt::Debug + Read + Seek {
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
            for i in 0..100 {
                reader.seek(SeekFrom::Current(i * 100)).unwrap();
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
            for i in 0..100 {
                reader.seek(SeekFrom::Current(i * 100)).unwrap();
                reader.read(&mut buf).unwrap();
            }
        });
        fs::remove_file("foo.txt").unwrap();
    }

    #[bench]
    fn read_seek_10mb_default_from_file(b: &mut Bencher) {
        let mut f = File::create("foo.txt").unwrap();
        f.write_all(&vec![0; 10000000]).unwrap();
        b.iter(|| {
            let mut reader = BufReader::new(File::open("foo.txt").unwrap());
            let mut buf: Vec<u8> = Vec::with_capacity(100000);
            for i in 0..100 {
                reader.seek(SeekFrom::Current(i * 100)).unwrap();
                reader.read(&mut buf).unwrap();
            }
        });
        fs::remove_file("foo.txt").unwrap();
    }

    #[bench]
    fn read_seek_10mb_default_from_file_std(b: &mut Bencher) {
        let mut f = File::create("foo.txt").unwrap();
        f.write_all(&vec![0; 10000000]).unwrap();
        b.iter(|| {
            let mut reader = io::BufReader::new(File::open("foo.txt").unwrap());
            let mut buf: Vec<u8> = Vec::with_capacity(100000);
            for i in 0..100 {
                reader.seek(SeekFrom::Current(i * 100)).unwrap();
                reader.read(&mut buf).unwrap();
            }
        });
        fs::remove_file("foo.txt").unwrap();
    }
}
