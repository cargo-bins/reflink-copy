use std::fs::File;
use std::io;
use std::path::Path;

use crate::sys;

/// Reflink `self` into `other`.
///
/// This trait unifies the path based API ([`reflink`](crate::reflink)) and the
/// file based API, which operates on already opened files.
///
/// # Implementations
///
/// - `Path` to `Path`: same as [`reflink`](crate::reflink). Thanks to
///   auto-deref, it also works for `PathBuf`.
/// - `File` to `Path`: like [`reflink`](crate::reflink), `other` is created
///   with `OpenOptions::create_new` and gets the permissions of `self`.
///   Since `File` has several implementations, pass `&Path`, e.g.
///   `file.reflink(path_buf.as_path())`.
/// - `File` to `File`: see below.
///
/// # `File` to `File`
///
/// The whole content of `self` is cloned into `other`, replacing its content;
/// `other` must be opened for writing.
/// Its permissions are left untouched, and if the operation fails its content
/// is unspecified.
///
/// Supported on Linux (`FICLONE`), Android and Windows
/// (`FSCTL_DUPLICATE_EXTENTS_TO_FILE`, see [`reflink`](crate::reflink) for its
/// restrictions).
/// On other platforms, including macOS whose `clonefile` can only create a
/// new file, it returns an error of kind [`io::ErrorKind::Unsupported`].
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
    #[cfg_attr(
        feature = "tracing",
        tracing_attributes::instrument(name = "reflink_file")
    )]
    fn reflink(&self, other: &File) -> io::Result<()> {
        sys::reflink_file(self, other)
    }
}

impl ReflinkOp<Path> for File {
    /// Reflink `self` into a new file at `other`.
    ///
    /// Same as [`reflink`](crate::reflink) except that the source is an
    /// opened file: `other` is created with `OpenOptions::create_new`, and
    /// the permissions of `self` are copied to it.
    ///
    /// Supported on Linux, Android, Windows and macOS (`fclonefileat`).
    #[cfg_attr(
        feature = "tracing",
        tracing_attributes::instrument(name = "reflink_file_to_path")
    )]
    fn reflink(&self, other: &Path) -> io::Result<()> {
        sys::reflink_file_to_path(self, other)
    }
}
