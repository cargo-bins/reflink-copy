use std::fs::File;
use std::io;
use std::path::Path;

/// Reflink `self` into `other`.
///
/// This trait unifies the path based and the file based APIs. Every
/// implementation is a thin wrapper around one of the free functions.
///
/// # Implementations
///
/// | `Self` | `Other` | Same as |
/// | --- | --- | --- |
/// | `Path` | `Path` | [`reflink`](crate::reflink) |
/// | `File` | `File` | [`reflink_file`](crate::reflink_file) |
/// | `File` | `Path` | [`reflink_file_to_path`](crate::reflink_file_to_path) |
/// | `File` | `(&Dir, &P)` | [`reflink_file_to_dir`](crate::reflink_file_to_dir) |
/// | `(&Dir, &P)` | `(&Dir, &P)` | opens the source in its `Dir`, then [`reflink_file_to_dir`](crate::reflink_file_to_dir) |
///
/// `Dir` is [`cap_std::fs::Dir`] and `P: AsRef<Path>`; these require the
/// `cap-std` feature.
///
/// Thanks to auto-deref, `Path` to `Path` also works for `PathBuf`. Since
/// `File` has several implementations, pass a `&Path` to it, e.g.
/// `file.reflink(path_buf.as_path())`.
///
/// ```no_run
/// use reflink_copy::ReflinkOp;
/// use std::fs::File;
///
/// fn clone() -> std::io::Result<()> {
///     let src = File::open("src.txt")?;
///     let dest = File::create_new("dest.txt")?;
///     src.reflink(&dest)
/// }
/// ```
pub trait ReflinkOp<Other: ?Sized = Self> {
    /// Reflink `self` into `other`.
    fn reflink(&self, other: &Other) -> io::Result<()>;
}

impl ReflinkOp for Path {
    /// Same as [`reflink`](crate::reflink).
    #[inline]
    fn reflink(&self, other: &Path) -> io::Result<()> {
        crate::reflink(self, other)
    }
}

impl ReflinkOp for File {
    /// Same as [`reflink_file`](crate::reflink_file).
    #[inline]
    fn reflink(&self, other: &File) -> io::Result<()> {
        crate::reflink_file(self, other)
    }
}

impl ReflinkOp<Path> for File {
    /// Same as [`reflink_file_to_path`](crate::reflink_file_to_path).
    #[inline]
    fn reflink(&self, other: &Path) -> io::Result<()> {
        crate::reflink_file_to_path(self, other)
    }
}

#[cfg(all(feature = "cap-std", any(unix, windows)))]
impl<P: AsRef<Path> + ?Sized> ReflinkOp<(&cap_std::fs::Dir, &P)> for File {
    /// Same as [`reflink_file_to_dir`](crate::reflink_file_to_dir).
    #[inline]
    fn reflink(&self, (dir, path): &(&cap_std::fs::Dir, &P)) -> io::Result<()> {
        crate::reflink_file_to_dir(self, dir, path)
    }
}

#[cfg(all(feature = "cap-std", any(unix, windows)))]
impl<P, Q> ReflinkOp<(&cap_std::fs::Dir, &Q)> for (&cap_std::fs::Dir, &P)
where
    P: AsRef<Path> + ?Sized,
    Q: AsRef<Path> + ?Sized,
{
    /// Opens the source in its `Dir`, then same as
    /// [`reflink_file_to_dir`](crate::reflink_file_to_dir).
    #[inline]
    fn reflink(&self, (dir, path): &(&cap_std::fs::Dir, &Q)) -> io::Result<()> {
        let (src_dir, src_path) = self;
        let src = src_dir.open(src_path)?.into_std();
        crate::reflink_file_to_dir(&src, dir, path)
    }
}
