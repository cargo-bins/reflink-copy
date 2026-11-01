use std::{
    ffi::{CStr, CString},
    fs::File,
    io,
    os::{
        raw::c_int,
        unix::{
            ffi::OsStrExt,
            io::{AsFd, AsRawFd, BorrowedFd},
        },
    },
    path::Path,
};

fn cstr(path: &Path) -> io::Result<CString> {
    Ok(CString::new(path.as_os_str().as_bytes())?)
}

// const CLONE_NOFOLLOW: c_int = 0x0001;
const CLONE_NOOWNERCOPY: c_int = 0x0002;

// https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/fcntl.h
const AT_FDCWD: c_int = -2;

mod ffi {
    use std::os::raw::{c_char, c_int};

    extern "C" {
        // http://www.manpagez.com/man/2/clonefileat/
        // https://github.com/apple/darwin-xnu/blob/0a798f6738bc1db01281fc08ae024145e84df927/bsd/sys/clonefile.h
        // TODO We need weak linkage here (OSX > 10.12, iOS > 10.0), otherwise compilation will fail on older versions
        pub fn clonefile(src: *const c_char, dest: *const c_char, flags: c_int) -> c_int;

        // Available since the same versions as `clonefile`.
        pub fn fclonefileat(
            srcfd: c_int,
            dst_dirfd: c_int,
            dst: *const c_char,
            flags: c_int,
        ) -> c_int;
    }
}

fn cvt(ret: c_int) -> io::Result<()> {
    if ret == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn clonefile(src: &CStr, dest: &CStr) -> io::Result<()> {
    cvt(unsafe { ffi::clonefile(src.as_ptr(), dest.as_ptr(), CLONE_NOOWNERCOPY) })
}

/// Clone `src` into a new file `dst`, resolved relative to `dst_dir`, or to
/// the current directory if it is `None`.
fn fclonefileat(
    src: BorrowedFd<'_>,
    dst_dir: Option<BorrowedFd<'_>>,
    dst: &CStr,
) -> io::Result<()> {
    let dst_dirfd = dst_dir.map_or(AT_FDCWD, |fd| fd.as_raw_fd());
    cvt(unsafe { ffi::fclonefileat(src.as_raw_fd(), dst_dirfd, dst.as_ptr(), CLONE_NOOWNERCOPY) })
}

pub fn reflink(from: &Path, to: &Path) -> io::Result<()> {
    clonefile(&cstr(from)?, &cstr(to)?)
}

pub fn reflink_file_to_path(from: &File, to: &Path) -> io::Result<()> {
    fclonefileat(from.as_fd(), None, &cstr(to)?)
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
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let parent = dir.open_dir(parent)?;

    fclonefileat(from.as_fd(), Some(parent.as_fd()), &cstr(Path::new(name))?)
}
