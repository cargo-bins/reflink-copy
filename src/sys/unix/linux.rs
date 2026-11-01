use std::convert::TryInto;
use std::os::unix::io::AsRawFd;
use std::{fs, io, path::Path};

use crate::sys::utility::AutoRemovedFile;

pub fn reflink(from: &Path, to: &Path) -> io::Result<()> {
    reflink_file_to_path(&fs::File::open(from)?, to)
}

pub fn reflink_file_to_path(from: &fs::File, to: &Path) -> io::Result<()> {
    // pass O_EXCL to mimic macos behaviour
    let dest = AutoRemovedFile::create_new(to)?;
    rustix::fs::ioctl_ficlone(&dest, from)?;

    dest.persist_with_same_permission_as(from)
}

#[cfg(feature = "cap-std")]
pub fn reflink_file_to_dir(from: &fs::File, dir: &cap_std::fs::Dir, to: &Path) -> io::Result<()> {
    // pass O_EXCL to mimic macos behaviour
    let dest = AutoRemovedFile::create_new_in(dir, to)?;
    rustix::fs::ioctl_ficlone(&dest, from)?;

    dest.persist_with_same_permission_as(from)
}

pub fn reflink_file(from: &fs::File, to: &fs::File) -> io::Result<()> {
    // FICLONE never shrinks `to`: if `to` is larger than `from`, it either
    // keeps the old tail of `to` (`from` ends at a block boundary) or fails
    // with EINVAL (it does not, since a partial block cannot be cloned into
    // the middle of a file). Truncate it first to replace the content,
    // consistent with the windows implementation.
    if to.metadata()?.len() != 0 {
        to.set_len(0)?;
    }
    rustix::fs::ioctl_ficlone(to, from)?;
    Ok(())
}

#[cfg(target_os = "linux")]
pub(crate) fn reflink_block(
    from: crate::sys::BorrowedFile<'_>,
    from_offset: u64,
    to: crate::sys::BorrowedFile<'_>,
    to_offset: u64,
    src_length: u64,
    _cluster_size: Option<std::num::NonZeroU64>,
) -> io::Result<()> {
    let ret = unsafe {
        libc::ioctl(
            to.as_raw_fd(),
            libc::FICLONERANGE.try_into().unwrap(),
            &libc::file_clone_range {
                src_fd: from.as_raw_fd().into(),
                src_offset: from_offset,
                src_length,
                dest_offset: to_offset,
            },
        )
    };

    if ret == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(target_os = "android")]
pub(crate) use crate::sys::reflink_block_not_supported as reflink_block;
