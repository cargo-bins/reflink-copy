use std::{
    ffi::CString,
    fs::File,
    io,
    os::unix::{ffi::OsStrExt, io::AsFd},
    path::Path,
};

fn cstr(path: &Path) -> io::Result<CString> {
    Ok(CString::new(path.as_os_str().as_bytes())?)
}

mod ffi {
    use std::{
        ffi::CStr,
        io,
        os::{
            raw::c_int,
            unix::io::{AsRawFd, BorrowedFd},
        },
    };

    // Not in libc.
    // https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/clonefile.h
    // const CLONE_NOFOLLOW: u32 = 0x0001;
    const CLONE_NOOWNERCOPY: u32 = 0x0002;

    fn cvt(ret: c_int) -> io::Result<()> {
        if ret == -1 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// Clone `src` into a new file `dest`.
    pub fn clonefile(src: &CStr, dest: &CStr) -> io::Result<()> {
        cvt(unsafe { libc::clonefile(src.as_ptr(), dest.as_ptr(), CLONE_NOOWNERCOPY) })
    }

    /// Clone `src` into a new file `dst`, resolved relative to `dst_dir`, or to
    /// the current directory if it is `None`.
    pub fn fclonefileat(
        src: BorrowedFd<'_>,
        dst_dir: Option<BorrowedFd<'_>>,
        dst: &CStr,
    ) -> io::Result<()> {
        let dst_dirfd = dst_dir.map_or(libc::AT_FDCWD, |fd| fd.as_raw_fd());
        cvt(unsafe {
            libc::fclonefileat(src.as_raw_fd(), dst_dirfd, dst.as_ptr(), CLONE_NOOWNERCOPY)
        })
    }
}

pub fn reflink(from: &Path, to: &Path) -> io::Result<()> {
    ffi::clonefile(&cstr(from)?, &cstr(to)?)
}

pub fn reflink_file_to_path(from: &File, to: &Path) -> io::Result<()> {
    ffi::fclonefileat(from.as_fd(), None, &cstr(to)?)
}

#[cfg(feature = "cap-std")]
pub fn reflink_file_to_dir(from: &File, dir: &cap_std::fs::Dir, to: &Path) -> io::Result<()> {
    // `fclonefileat` resolves `dst` relative to `dst_dir` without any
    // restriction: `..`, symlinks and absolute paths can all escape `dir`.
    // So open the parent with cap-std, which does restrict it, and only pass
    // the last component to `fclonefileat`.
    let name = to
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    let parent = match to.parent() {
        // The opened `Dir` is kept alive by temporary lifetime extension.
        Some(parent) if !parent.as_os_str().is_empty() => &dir.open_dir(parent)?,
        _ => dir,
    };

    ffi::fclonefileat(from.as_fd(), Some(parent.as_fd()), &cstr(Path::new(name))?)
}
