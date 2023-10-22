use std::{fs, io, path::Path};

use crate::sys::utility::AutoRemovedFile;

pub fn reflink(from: &Path, to: &Path) -> io::Result<()> {
    let src = fs::File::open(from)?;

    // pass O_EXCL to mimic macos behaviour
    let dest = AutoRemovedFile::create_new(to)?;
    rustix::fs::ioctl_ficlone(&dest, &src)?;

    dest.persist();
    Ok(())
}
