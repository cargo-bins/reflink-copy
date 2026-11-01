//! Tests for [`ReflinkOp`].
//!
//! Set `REFLINK_COPY_TEST_DIR` to a directory on a file system supporting
//! reflink (e.g. btrfs or XFS on Linux) to require the reflink to succeed,
//! otherwise failing to reflink because of the file system is tolerated.

use std::fs::{self, File};
use std::io::{self, Read, Seek};
use std::path::{Path, PathBuf};

use reflink_copy::ReflinkOp;
use tempfile::TempDir;

const DATA_LEN: usize = 3 * 64 * 1024 + 7;

fn test_dir() -> (TempDir, bool) {
    match std::env::var_os("REFLINK_COPY_TEST_DIR") {
        Some(dir) => (tempfile::tempdir_in(dir).unwrap(), true),
        None => (tempfile::tempdir().unwrap(), false),
    }
}

fn data() -> Vec<u8> {
    (0..DATA_LEN).map(|i| (i % 251) as u8).collect()
}

const FILE_OP_SUPPORTED: bool = cfg!(any(
    all(
        any(target_os = "linux", target_os = "android"),
        not(any(target_arch = "sparc", target_arch = "sparc64"))
    ),
    windows
));

/// Check the result of a file based reflink, return `true` if the content
/// should be checked.
fn check_file_result(res: io::Result<()>, must_succeed: bool) -> bool {
    match res {
        Ok(()) if FILE_OP_SUPPORTED => true,
        Ok(()) => panic!("file based reflink should be unsupported on this platform"),
        Err(err) if !FILE_OP_SUPPORTED => {
            assert_eq!(err.kind(), io::ErrorKind::Unsupported, "{:?}", err);
            false
        }
        Err(err) => {
            assert!(!must_succeed, "reflink failed: {:?}", err);
            println!("reflink failed (tolerated): {:?}", err);
            false
        }
    }
}

fn read_all(mut file: &File) -> Vec<u8> {
    let mut buf = Vec::new();
    file.rewind().unwrap();
    file.read_to_end(&mut buf).unwrap();
    buf
}

#[test]
fn file_to_new_file() {
    let (dir, must_succeed) = test_dir();
    let src_path = dir.path().join("src");
    fs::write(&src_path, data()).unwrap();

    let src = File::open(&src_path).unwrap();
    let dest = File::options()
        .read(true)
        .write(true)
        .create_new(true)
        .open(dir.path().join("dest"))
        .unwrap();

    if check_file_result(src.reflink(&dest), must_succeed) {
        assert_eq!(read_all(&dest), data());
    }
}

fn reflink_to_existing_larger_file(src_data: &[u8]) {
    let (dir, must_succeed) = test_dir();
    let src_path = dir.path().join("src");
    fs::write(&src_path, src_data).unwrap();

    let dest_path = dir.path().join("dest");
    fs::write(&dest_path, vec![0xffu8; src_data.len() * 2]).unwrap();

    let src = File::open(&src_path).unwrap();
    let dest = File::options()
        .read(true)
        .write(true)
        .open(&dest_path)
        .unwrap();

    if check_file_result(src.reflink(&dest), must_succeed) {
        assert_eq!(read_all(&dest), src_data);
    }
}

/// On Linux, FICLONE rejects an unaligned source into a larger destination
/// with EINVAL.
#[test]
fn file_to_existing_larger_file_unaligned() {
    reflink_to_existing_larger_file(&data());
}

/// On Linux, FICLONE accepts a block aligned source into a larger
/// destination but keeps the destination's tail.
#[test]
fn file_to_existing_larger_file_aligned() {
    reflink_to_existing_larger_file(&data()[..3 * 64 * 1024]);
}

#[cfg(unix)]
#[test]
fn file_keeps_dest_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let (dir, must_succeed) = test_dir();
    let src_path = dir.path().join("src");
    fs::write(&src_path, data()).unwrap();
    fs::set_permissions(&src_path, fs::Permissions::from_mode(0o755)).unwrap();

    let dest_path = dir.path().join("dest");
    let dest = File::create_new(&dest_path).unwrap();
    dest.set_permissions(fs::Permissions::from_mode(0o600))
        .unwrap();

    let src = File::open(&src_path).unwrap();
    if check_file_result(src.reflink(&dest), must_succeed) {
        let mode = fs::metadata(&dest_path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

#[test]
fn file_src_not_readable() {
    let (dir, _) = test_dir();
    let src_path = dir.path().join("src");
    fs::write(&src_path, data()).unwrap();

    // Opened write-only, so it cannot be used as reflink source.
    let src = File::options().write(true).open(&src_path).unwrap();
    let dest = File::create_new(dir.path().join("dest")).unwrap();

    src.reflink(&dest).unwrap_err();
}

#[test]
fn path_impls() {
    let (dir, must_succeed) = test_dir();
    let src: PathBuf = dir.path().join("src");
    fs::write(&src, data()).unwrap();

    let check = |res: io::Result<()>, dest: &Path| match res {
        Ok(()) => assert_eq!(fs::read(dest).unwrap(), data()),
        Err(err) => {
            assert!(!must_succeed, "reflink failed: {:?}", err);
            println!("reflink failed (tolerated): {:?}", err);
        }
    };

    let dest1 = dir.path().join("dest1");
    check(src.as_path().reflink(dest1.as_path()), &dest1);

    let dest2 = dir.path().join("dest2");
    check(src.as_path().reflink(&dest2), &dest2);

    let dest3 = dir.path().join("dest3");
    check(src.reflink(&dest3), &dest3);

    let dest4 = dir.path().join("dest4");
    check(src.reflink(dest4.as_path()), &dest4);

    // Same as `reflink`: `other` must not exist.
    let existing = dir.path().join("existing");
    fs::write(&existing, b"hello").unwrap();
    let err = src.reflink(&existing).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::AlreadyExists, "{:?}", err);
    assert_eq!(fs::read(&existing).unwrap(), b"hello");
}

/// A directory opened as a `File` is reported as an invalid source.
#[cfg(target_os = "linux")]
#[test]
fn file_src_is_dir() {
    let (dir, _) = test_dir();
    let src = File::open(dir.path()).unwrap();

    let dest_path = dir.path().join("dest");
    let err = src.reflink(dest_path.as_path()).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{:?}", err);
    assert!(!dest_path.exists());

    let dest = File::create_new(&dest_path).unwrap();
    let err = src.reflink(&dest).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{:?}", err);
}

const FILE_TO_PATH_SUPPORTED: bool = FILE_OP_SUPPORTED
    || cfg!(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos"
    ));

#[test]
fn file_to_path() {
    let (dir, must_succeed) = test_dir();
    let src_path = dir.path().join("src");
    fs::write(&src_path, data()).unwrap();
    let src = File::open(&src_path).unwrap();

    let dest = dir.path().join("dest");
    match src.reflink(dest.as_path()) {
        Ok(()) if FILE_TO_PATH_SUPPORTED => assert_eq!(fs::read(&dest).unwrap(), data()),
        Ok(()) => panic!("file to path reflink should be unsupported on this platform"),
        Err(err) if !FILE_TO_PATH_SUPPORTED => {
            assert_eq!(err.kind(), io::ErrorKind::Unsupported, "{:?}", err);
        }
        Err(err) => {
            assert!(!must_succeed, "reflink failed: {:?}", err);
            println!("reflink failed (tolerated): {:?}", err);
            // The dest file is removed on failure.
            assert!(!dest.exists());
        }
    }
}

#[test]
fn file_to_existing_path() {
    let (dir, _) = test_dir();
    let src_path = dir.path().join("src");
    fs::write(&src_path, data()).unwrap();
    let src = File::open(&src_path).unwrap();

    let existing = dir.path().join("existing");
    fs::write(&existing, b"hello").unwrap();
    let err = src.reflink(existing.as_path()).unwrap_err();
    if FILE_TO_PATH_SUPPORTED {
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists, "{:?}", err);
    }
    assert_eq!(fs::read(&existing).unwrap(), b"hello");
}

#[cfg(unix)]
#[test]
fn file_to_path_copies_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let (dir, must_succeed) = test_dir();
    let src_path = dir.path().join("src");
    fs::write(&src_path, data()).unwrap();
    fs::set_permissions(&src_path, fs::Permissions::from_mode(0o751)).unwrap();
    let src = File::open(&src_path).unwrap();

    let dest = dir.path().join("dest");
    match src.reflink(dest.as_path()) {
        Ok(()) => {
            let mode = fs::metadata(&dest).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o751);
        }
        Err(err) => {
            assert!(!(must_succeed && FILE_TO_PATH_SUPPORTED), "{:?}", err);
        }
    }
}

#[cfg(all(feature = "cap-std", any(unix, windows)))]
mod cap_std_tests {
    use super::*;
    use cap_std::{ambient_authority, fs::Dir};

    fn open_dir(dir: &TempDir) -> Dir {
        let dir = Dir::open_ambient_dir(dir.path(), ambient_authority()).unwrap();
        let mut src = dir.create("src").unwrap();
        std::io::Write::write_all(&mut src, &data()).unwrap();
        dir
    }

    fn create_dest(dir: &Dir, name: &str) -> cap_std::fs::File {
        dir.open_with(
            name,
            cap_std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true),
        )
        .unwrap()
    }

    fn read_all_cap(file: &cap_std::fs::File) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut file = file;
        file.rewind().unwrap();
        file.read_to_end(&mut buf).unwrap();
        buf
    }

    #[test]
    fn reflink_block_cap_std_whole_file() {
        use reflink_copy::ReflinkBlockBuilder;
        use std::num::NonZeroU64;

        let (tmp, must_succeed) = test_dir();
        let dir = open_dir(&tmp);

        // Block cloning needs cluster aligned lengths on windows.
        let len = 3 * 64 * 1024;
        dir.write("aligned_src", &data()[..len]).unwrap();
        let src = dir.open("aligned_src").unwrap();
        let dest = create_dest(&dir, "dest");
        dest.set_len(len as u64).unwrap();

        let res =
            ReflinkBlockBuilder::new_cap_std(&src, &dest, NonZeroU64::new(len as u64).unwrap())
                .reflink_block();
        if check_file_result(res, must_succeed) {
            assert_eq!(read_all_cap(&dest), &data()[..len]);
        }
    }

    /// Check the result of a reflink into a `Dir`, return `true` if the
    /// content should be checked.
    fn check_dir_result(res: io::Result<()>, must_succeed: bool) -> bool {
        match res {
            Ok(()) if FILE_TO_PATH_SUPPORTED => true,
            Ok(()) => panic!("reflink into a Dir should be unsupported on this platform"),
            Err(err) if !FILE_TO_PATH_SUPPORTED => {
                assert_eq!(err.kind(), io::ErrorKind::Unsupported, "{:?}", err);
                false
            }
            Err(err) => {
                assert!(!must_succeed, "reflink failed: {:?}", err);
                println!("reflink failed (tolerated): {:?}", err);
                false
            }
        }
    }

    #[test]
    fn file_to_dir() {
        let (tmp, must_succeed) = test_dir();
        let dir = open_dir(&tmp);
        dir.create_dir("sub").unwrap();
        let src = File::open(tmp.path().join("src")).unwrap();

        let check = |res, name: &str| {
            if check_dir_result(res, must_succeed) {
                assert_eq!(fs::read(tmp.path().join(name)).unwrap(), data());
            } else {
                // The dest file is removed on failure.
                assert!(!tmp.path().join(name).exists());
            }
        };

        for name in ["dest", "sub/dest"] {
            check(src.reflink(&(&dir, name)), name);
        }

        let name = Path::new("sub").join("dest_fn");
        check(
            reflink_copy::reflink_file_to_dir(&src, &dir, &name),
            "sub/dest_fn",
        );
    }

    #[test]
    fn dir_to_dir() {
        let (tmp, must_succeed) = test_dir();
        let dir = open_dir(&tmp);

        let src_tmp = tempfile::tempdir_in(tmp.path()).unwrap();
        let src_dir = Dir::open_ambient_dir(src_tmp.path(), ambient_authority()).unwrap();
        src_dir.write("src", data()).unwrap();

        let res = (&src_dir, "src").reflink(&(&dir, "dest"));
        if check_dir_result(res, must_succeed) {
            assert_eq!(fs::read(tmp.path().join("dest")).unwrap(), data());
        }

        // The source is bounded too.
        let name = src_tmp.path().file_name().unwrap();
        let escaping = Path::new("..").join(name).join("src");
        (&src_dir, &escaping).reflink(&(&dir, "dest3")).unwrap_err();
        assert!(!tmp.path().join("dest3").exists());
    }

    #[test]
    fn dir_cannot_escape() {
        let (tmp, _) = test_dir();
        let dir = open_dir(&tmp);
        dir.create_dir("sub").unwrap();
        let sub = dir.open_dir("sub").unwrap();
        let src = File::open(tmp.path().join("src")).unwrap();

        let outside = tmp.path().join("outside");
        for path in [Path::new("../outside"), outside.as_path()] {
            src.reflink(&(&sub, path)).unwrap_err();
            assert!(!outside.exists(), "{}", path.display());
        }

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(tmp.path(), tmp.path().join("sub/link")).unwrap();
            src.reflink(&(&sub, "link/outside")).unwrap_err();
            assert!(!outside.exists());
        }
    }

    #[test]
    fn file_to_existing_in_dir() {
        let (tmp, _) = test_dir();
        let dir = open_dir(&tmp);
        let src = File::open(tmp.path().join("src")).unwrap();
        dir.write("existing", b"hello").unwrap();

        let err = src.reflink(&(&dir, "existing")).unwrap_err();
        if FILE_TO_PATH_SUPPORTED {
            assert_eq!(err.kind(), io::ErrorKind::AlreadyExists, "{:?}", err);
        }
        assert_eq!(fs::read(tmp.path().join("existing")).unwrap(), b"hello");
    }

    #[cfg(unix)]
    #[test]
    fn file_to_dir_copies_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let (tmp, must_succeed) = test_dir();
        let dir = open_dir(&tmp);
        let src_path = tmp.path().join("src");
        fs::set_permissions(&src_path, fs::Permissions::from_mode(0o751)).unwrap();
        let src = File::open(&src_path).unwrap();

        let res = src.reflink(&(&dir, "dest"));
        if check_dir_result(res, must_succeed) {
            let mode = fs::metadata(tmp.path().join("dest"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o751);
        }
    }
}
