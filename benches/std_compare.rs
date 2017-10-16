#![feature(test)]

extern crate seek_bufread;
extern crate test;

use seek_bufread::BufReader;

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
