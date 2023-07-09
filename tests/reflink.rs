use std::fs::{self, File};
use std::io;
use std::path::Path;
use tempfile::tempdir;

use reflink::{reflink, reflink_or_copy};

#[test]
fn reflink_file_does_not_exist() {
    let from = Path::new("test/nonexistent-bogus-path");
    let to = Path::new("test/other-bogus-path");

    match reflink(from, to) {
        Ok(..) => panic!(),
        Err(..) => {
            assert!(!from.exists());
            assert!(!to.exists());
        }
    }
}

#[test]
fn reflink_src_does_not_exist() {
    let tmpdir = tempdir().unwrap();
    let from = Path::new("test/nonexistent-bogus-path");
    let to = tmpdir.path().join("out.txt");

    fs::write(&to, b"hello").unwrap();
    assert!(reflink(from, &to).is_err());

    assert!(!from.exists());
    assert_eq!(fs::read(&to).unwrap(), b"hello");
}

#[test]
fn reflink_dest_is_dir() {
    let dir = tempdir().unwrap();
    let src_file_path = dir.path().join("src.txt");
    let _src_file = File::create(&src_file_path).unwrap();
    match reflink(&src_file_path, dir.path()) {
        Ok(()) => panic!(),
        Err(e) => {
            println!("{:?}", e);
            if !cfg!(windows) {
                assert_eq!(e.kind(), io::ErrorKind::AlreadyExists);
            }
        }
    }
}

#[test]
fn reflink_src_is_dir() {
    let dir = tempdir().unwrap();
    let dest_file_path = dir.path().join("dest.txt");

    match reflink(dir.path(), dest_file_path) {
        Ok(()) => panic!(),
        Err(e) => {
            println!("{:?}", e);
            assert_eq!(e.kind(), io::ErrorKind::InvalidInput)
        }
    }
}

#[test]
fn reflink_existing_dest_results_in_error() {
    let dir = tempdir().unwrap();
    let src_file_path = dir.path().join("src.txt");
    let dest_file_path = dir.path().join("dest.txt");

    let _src_file = File::create(&src_file_path).unwrap();
    let _dest_file = File::create(&dest_file_path).unwrap();

    match reflink(&src_file_path, &dest_file_path) {
        Ok(()) => panic!(),
        Err(e) => {
            println!("{:?}", e);
            assert_eq!(e.kind(), io::ErrorKind::AlreadyExists)
        }
    }
}

#[test]
fn reflink_ok() {
    let dir = tempdir().unwrap();
    let src_file_path = dir.path().join("src.txt");
    let dest_file_path = dir.path().join("dest.txt");

    fs::write(&src_file_path, b"this is a test").unwrap();

    match reflink(&src_file_path, &dest_file_path) {
        Ok(()) => {}
        Err(e) => {
            println!("{:?}", e);
            // do not panic for now, CI envs are old and will probably error out
            return;
        }
    }
    assert_eq!(fs::read(&dest_file_path).unwrap(), b"this is a test");
}

#[test]
fn reflink_or_copy_ok() {
    let tmpdir = tempdir().unwrap();
    let input = tmpdir.path().join("in.txt");
    let out = tmpdir.path().join("out.txt");

    fs::write(&input, b"hello").unwrap();

    reflink_or_copy(&input, &out).unwrap();

    assert_eq!(fs::read(&out).unwrap(), b"hello");

    assert_eq!(
        input.metadata().unwrap().permissions(),
        out.metadata().unwrap().permissions()
    );
}
