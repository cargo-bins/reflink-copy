use std::io;
use std::path::Path;

#[cfg(any(target_os = "linux", target_os = "android"))]
mod linux;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub use linux::reflink;

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub fn reflink(from: &Path, to: &Path) -> io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    fn cstr(path: &Path) -> io::Result<CString> {
        Ok(CString::new(path.as_os_str().as_bytes())?)
    }

    // const CLONE_NOFOLLOW: libc::c_int = 0x0001;
    const CLONE_NOOWNERCOPY: libc::c_int = 0x0002;

    extern "C" {
        // http://www.manpagez.com/man/2/clonefileat/
        // https://github.com/apple/darwin-xnu/blob/0a798f6738bc1db01281fc08ae024145e84df927/bsd/sys/clonefile.h
        // TODO We need weak linkage here (OSX > 10.12, iOS > 10.0), otherwise compilation will fail on older versions
        fn clonefile(
            src: *const libc::c_char,
            dest: *const libc::c_char,
            flags: libc::c_int,
        ) -> libc::c_int;
    }

    let src = cstr(from)?;
    let dest = cstr(to)?;

    let ret = unsafe { clonefile(src.as_ptr(), dest.as_ptr(), CLONE_NOOWNERCOPY) };

    if ret == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios"
)))]
pub fn reflink(_from: &Path, _to: &Path) -> io::Result<()> {
    super::reflink_not_supported()
}
