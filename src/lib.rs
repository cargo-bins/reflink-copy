//! Some file systems implement COW (copy on write) functionality in order to speed up file copies.
//! On a high level, the new file does not actually get copied, but shares the same on-disk data
//! with the source file. As soon as one of the files is modified, the actual copying is done by
//! the underlying OS.
//!
//! This library exposes a single function, `reflink`, which attempts to copy a file using the
//! underlying OSs' block cloning capabilities. The function signature is identical to `std::fs::copy`.
//!
//! At the moment Linux, Android, OSX, ios and Windows are supported.
//! As soon as other OS support the functionality, support will be added.

mod sys;

use std::fs;
use std::io;
use std::path::Path;

/// Copies a file using COW semantics.
///
/// For compatibility reasons with macos, the target file will be created using `OpenOptions::create_new`.
/// If you want to overwrite existing files, make sure you manually delete the target file first
/// if it exists.
///
/// ```rust
/// use reflink;
/// match reflink::reflink("src.txt", "dest.txt") {
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
/// ## OS X / ios
///
/// Uses `clonefile` library function. This is supported on OS X Version >=10.12 and iOS version >= 10.0
/// This will work on APFS partitions (which means most desktop systems are capable).
///
/// ## Windows
///
/// Uses ioctl `FSCTL_DUPLICATE_EXTENTS_TO_FILE`.
///
/// Only supports ReFS on Windows Server. *Important note*: The windows implementation is currently
/// untested and probably buggy. Contributions/testers with access to a Windows Server welcome.
///
/// NOTE that it generates a temporary file and is not atomic.
pub fn reflink<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> io::Result<()> {
    let (from, to) = (from.as_ref(), to.as_ref());
    if !from.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the source path is not an existing regular file",
        ));
    }
    sys::reflink(from, to)
}

/// Attempts to reflink a file. If the operation fails, a conventional copy operation is
/// attempted as a fallback.
///
/// If the function reflinked a file, the return value will be `Ok(None)`.
///
/// If the function copied a file, the return value will be `Ok(Some(written))`.
///
/// ```rust
/// use reflink;
/// match reflink::reflink_or_copy("src.txt", "dest.txt") {
///     Ok(None) => println!("file has been reflinked"),
///     Ok(Some(written)) => println!("file has been copied ({} bytes)", written),
///     Err(e) => println!("an error occured: {:?}", e)
/// }
/// ```
pub fn reflink_or_copy<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> io::Result<Option<u64>> {
    if let Ok(()) = reflink(&from, &to) {
        Ok(None)
    } else {
        fs::copy(from, to).map(Some)
    }
}
