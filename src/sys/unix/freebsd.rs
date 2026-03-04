use std::os::unix::io::AsRawFd;
use std::{fs, io, path::Path};

use crate::sys::utility::AutoRemovedFile;

pub fn reflink(from: &Path, to: &Path) -> io::Result<()> {
    let src = fs::File::open(from)?;
    let dest = AutoRemovedFile::create_new(to)?;

    let len = src.metadata()?.len();
    let mut off_in: libc::off_t = 0;
    let mut off_out: libc::off_t = 0;
    let mut remaining = len;

    while remaining > 0 {
        let chunk = remaining.min(i64::MAX as u64) as usize;
        let ret = unsafe {
            libc::copy_file_range(
                src.as_raw_fd(),
                &mut off_in,
                dest.as_inner_file().as_raw_fd(),
                &mut off_out,
                chunk,
                0,
            )
        };

        if ret < 0 {
            return Err(io::Error::last_os_error());
        }
        if ret == 0 {
            break;
        }

        remaining -= ret as u64;
    }

    dest.persist();
    Ok(())
}
