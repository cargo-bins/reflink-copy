use std::{convert::TryInto, fs, io, mem::size_of, os::unix::io::AsRawFd, path::Path};

use ioctl_sys::iow;
use libc::c_int;
use tempfile::NamedTempFile;

const C_INT_SIZE: usize = size_of::<c_int>();
const FICLONE: u32 = iow!(0x94, 9, C_INT_SIZE);

pub fn reflink(from: &Path, to: &Path) -> io::Result<()> {
    let src = fs::File::open(&from)?;

    // pass O_EXCL to mimic macos behaviour
    let to_parent = to
        .parent()
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
    let dest = NamedTempFile::new_in(to_parent)?;

    let ret = unsafe {
        // http://man7.org/linux/man-pages/man2/ioctl_ficlonerange.2.html
        libc::ioctl(
            dest.as_raw_fd(),
            FICLONE.try_into().unwrap(),
            src.as_raw_fd(),
        )
    };

    if ret == -1 {
        Err(io::Error::last_os_error())
    } else {
        dest.persist_noclobber(to)
            .map_err(|persist_err| persist_err.error)?;
        Ok(())
    }
}
