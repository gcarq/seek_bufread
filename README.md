# seek_bufread

[![Build Status](https://travis-ci.org/gcarq/seek_bufread.svg?branch=master)](https://travis-ci.org/gcarq/seek_bufread) [![Coverage Status](https://coveralls.io/repos/github/gcarq/seek_bufread/badge.svg?branch=master)](https://coveralls.io/github/gcarq/seek_bufread?branch=master) [![Crates.io](https://img.shields.io/crates/v/seek_bufread.svg)](https://crates.io/crates/seek_bufread/)

**24.04.2019 NOTE:** This library is no longer needed since BufReader provides [seek_relative()](https://doc.rust-lang.org/std/io/struct.BufReader.html#method.seek_relative) with the same functionality. (See [rust#31100](https://github.com/rust-lang/rust/issues/31100)).


A drop-in replacement for `std::io::BufReader` with seeking support.

[Full Documentation](https://gcarq.github.io/seek_bufread)

## Quick Example

```rust
use std::io::{self, Cursor, Read, Seek, SeekFrom};
use seek_bufread::BufReader;

let inner = Cursor::new([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
let mut reader = BufReader::new(inner);

reader.seek(SeekFrom::Current(4)).unwrap();
let mut buf = [0; 8];

// read bytes from internal buffer
reader.read(&mut buf).unwrap();
assert_eq!(buf, [4, 5, 6, 7, 8, 9, 10, 11]);
```

## Usage

Put this in your `Cargo.toml`:

```toml
[dependencies]
seek_bufread = "~1.2"
```

And this in your crate root:

```rust
extern crate seek_bufread;
```

## Benchmarks

Tests with the suffix `_std` are using the standard `std::io::BufRead`
implementation. The Overall performance without seek operations is
quite similar between both. With seek operations ``seek_bufread::BufRead``
is significantly faster.

```
test tests::read_10mb_default_from_cursor        ... bench:   6,044,915 ns/iter (+/- 275,518)
test tests::read_10mb_default_from_cursor_std    ... bench:   6,038,466 ns/iter (+/- 227,145)
test tests::read_10mb_default_from_file          ... bench:   4,213,179 ns/iter (+/- 116,043)
test tests::read_10mb_default_from_file_std      ... bench:   4,224,658 ns/iter (+/- 132,629)
test tests::read_10mb_fullbuf_from_file          ... bench:   7,280,470 ns/iter (+/- 209,827)
test tests::read_10mb_fullbuf_from_file_std      ... bench:   7,448,666 ns/iter (+/- 2,720,199)
test tests::read_10mb_halfbuf_from_file          ... bench:   5,962,017 ns/iter (+/- 415,952)
test tests::read_10mb_halfbuf_from_file_std      ... bench:   5,904,902 ns/iter (+/- 240,471)
test tests::read_seek_10mb_default_from_file     ... bench:       6,621 ns/iter (+/- 145)
test tests::read_seek_10mb_default_from_file_std ... bench:      59,651 ns/iter (+/- 1,993)
test tests::read_seek_10mb_halfbuf_from_file     ... bench:   1,168,511 ns/iter (+/- 63,956)
test tests::read_seek_10mb_halfbuf_from_file_std ... bench:  62,872,335 ns/iter (+/- 5,344,766)
```

## License

Apache-2.0
