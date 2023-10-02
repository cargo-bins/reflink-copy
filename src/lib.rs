//! Some file systems implement COW (copy on write) functionality in order to speed up file copies.
//! On a high level, the new file does not actually get copied, but shares the same on-disk data
//! with the source file. As soon as one of the files is modified, the actual copying is done by
//! the underlying OS.
//!
//! This library exposes a single function, `reflink`, which attempts to copy a file using the
//! underlying OSs' block cloning capabilities. The function signature is identical to `std::fs::copy`.
//!
//! At the moment Linux, Android, OSX, iOS, and Windows are supported.
//!
//! Note: On Windows, the integrity information features are only available on Windows Server editions
//! starting from Windows Server 2012. Client versions of Windows do not support these features.
//! [More Information](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ni-winioctl-fsctl_set_integrity_information)
//!
//! As soon as other OSes support the functionality, support will be added.

mod sys;

use std::fs;
use std::io;
use std::path::Path;

/// Copies a file using COW semantics.
///
/// For compatibility reasons with macOS, the target file will be created using `OpenOptions::create_new`.
/// If you want to overwrite existing files, make sure you manually delete the target file first
/// if it exists.
///
/// ```rust
/// match reflink_copy::reflink("src.txt", "dest.txt") {
///     Ok(()) => println!("file has been reflinked"),
///     Err(e) => println!("error while reflinking: {:?}", e)
/// }
/// ```
///
/// # Implementation details per platform
///
/// ## Linux / Android
///
/// Uses `ioctl_ficlone`. Supported file systems include btrfs and XFS (and maybe more in the future).
/// NOTE that it generates a temporary file and is not atomic.
///
/// ## OS X / iOS
///
/// Uses `clonefile` library function. This is supported on OS X Version >=10.12 and iOS version >= 10.0
/// This will work on APFS partitions (which means most desktop systems are capable).
///
/// ## Windows
///
/// Uses ioctl `FSCTL_DUPLICATE_EXTENTS_TO_FILE`.
///
/// Supports ReFS on Windows Server and Windows Dev Drives. *Important note*: The windows implementation is currently
/// untested and probably buggy. Contributions/testers with access to a Windows Server or Dev Drives are welcome.
/// [More Information on Dev Drives](https://learn.microsoft.com/en-US/windows/dev-drive/#how-does-dev-drive-work)
///
/// NOTE that it generates a temporary file and is not atomic.
#[inline(always)]
pub fn reflink(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    fn inner(from: &Path, to: &Path) -> io::Result<()> {
        if !from.is_file() {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "the source path is not an existing regular file",
            ))
        } else {
            sys::reflink(from, to)
        }
    }

    inner(from.as_ref(), to.as_ref())
}

/// Attempts to reflink a file. If the operation fails, a conventional copy operation is
/// attempted as a fallback.
///
/// If the function reflinked a file, the return value will be `Ok(None)`.
///
/// If the function copied a file, the return value will be `Ok(Some(written))`.
///
/// ```rust
/// match reflink_copy::reflink_or_copy("src.txt", "dest.txt") {
///     Ok(None) => println!("file has been reflinked"),
///     Ok(Some(written)) => println!("file has been copied ({} bytes)", written),
///     Err(e) => println!("an error occured: {:?}", e)
/// }
/// ```
#[inline(always)]
pub fn reflink_or_copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<Option<u64>> {
    fn inner(from: &Path, to: &Path) -> io::Result<Option<u64>> {
        if let Ok(()) = reflink(from, to) {
            Ok(None)
        } else {
            fs::copy(from, to).map(Some)
        }
    }

    inner(from.as_ref(), to.as_ref())
}
