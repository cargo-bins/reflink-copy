use std::path::Path;
use std::{fs, io};

use cfg_if::cfg_if;

mod utility;

cfg_if! {
    if #[cfg(unix)] {
        mod unix;
        pub use self::unix::{reflink, reflink_file, reflink_file_to_path};
        pub(crate) use self::unix::reflink_block;
        #[cfg(all(feature = "cap-std", any(target_os = "macos", target_os = "ios", target_os = "tvos", target_os = "watchos")))]
        pub(crate) use self::unix::reflink_file_to_dir_entry;
    } else if #[cfg(windows)] {
        mod windows_impl;
        pub use self::windows_impl::{reflink, reflink_file, reflink_file_to_path};
        pub use self::windows_impl::check_reflink_support;
        pub(crate) use self::windows_impl::reflink_block;
    } else {
        pub use self::reflink_not_supported as reflink;
        pub use self::reflink_file_not_supported as reflink_file;
        pub use self::reflink_file_to_path_not_supported as reflink_file_to_path;
        pub(crate) use self::reflink_block_not_supported as reflink_block;
    }
}

cfg_if! {
    if #[cfg(unix)] {
        /// A borrowed file descriptor/handle of a file.
        pub(crate) type BorrowedFile<'a> = std::os::unix::io::BorrowedFd<'a>;

        pub(crate) fn borrow_file<T: std::os::unix::io::AsFd + ?Sized>(file: &T) -> BorrowedFile<'_> {
            file.as_fd()
        }
    } else if #[cfg(windows)] {
        /// A borrowed file descriptor/handle of a file.
        pub(crate) type BorrowedFile<'a> = std::os::windows::io::BorrowedHandle<'a>;

        pub(crate) fn borrow_file<T: std::os::windows::io::AsHandle + ?Sized>(file: &T) -> BorrowedFile<'_> {
            file.as_handle()
        }
    } else {
        /// A borrowed file descriptor/handle of a file.
        pub(crate) type BorrowedFile<'a> = &'a fs::File;

        pub(crate) fn borrow_file(file: &fs::File) -> BorrowedFile<'_> {
            file
        }
    }
}

#[allow(dead_code)]
pub fn reflink_not_supported(_from: &Path, _to: &Path) -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}

#[allow(dead_code)]
pub fn reflink_file_not_supported(_from: &fs::File, _to: &fs::File) -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}

#[allow(dead_code)]
pub fn reflink_file_to_path_not_supported(_from: &fs::File, _to: &Path) -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}

#[allow(dead_code)]
pub(crate) fn reflink_block_not_supported(
    _from: BorrowedFile<'_>,
    _from_offset: u64,
    _to: BorrowedFile<'_>,
    _to_offset: u64,
    _src_length: u64,
    _cluster_size: Option<std::num::NonZeroU64>,
) -> io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}
