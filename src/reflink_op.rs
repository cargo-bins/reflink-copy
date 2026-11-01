use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use crate::sys;

/// Reflink `self` into `other`.
///
/// This trait unifies the path based API ([`reflink`](crate::reflink)) and the
/// file based API, which operates on already opened files and thus can be used
/// with sandboxed file systems APIs like [`cap-std`](https://docs.rs/cap-std)
/// (enable the `cap-std` feature).
///
/// # Path implementations
///
/// Same as [`reflink`](crate::reflink): `other` is created with
/// `OpenOptions::create_new`, so it must not exist, and the permissions of
/// `self` are copied to it.
///
/// # File implementations
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

macro_rules! impl_path {
    ($($from:ty => $to:ty),* $(,)?) => {
        $(
            impl ReflinkOp<$to> for $from {
                #[inline]
                fn reflink(&self, other: &$to) -> io::Result<()> {
                    crate::reflink(self, other)
                }
            }
        )*
    };
}

impl_path!(
    Path => Path,
    Path => PathBuf,
    PathBuf => PathBuf,
    PathBuf => Path,
);

impl ReflinkOp for File {
    #[cfg_attr(
        feature = "tracing",
        tracing_attributes::instrument(name = "reflink_file")
    )]
    fn reflink(&self, other: &File) -> io::Result<()> {
        sys::reflink_file(self, other)
    }
}

#[cfg(all(feature = "cap-std", any(unix, windows)))]
mod cap_std_impl {
    use super::*;
    use crate::file_ref::FileRef;

    impl ReflinkOp for cap_std::fs::File {
        #[inline]
        fn reflink(&self, other: &cap_std::fs::File) -> io::Result<()> {
            FileRef::from_owner(self).reflink(&*FileRef::from_owner(other))
        }
    }

    impl ReflinkOp<File> for cap_std::fs::File {
        #[inline]
        fn reflink(&self, other: &File) -> io::Result<()> {
            FileRef::from_owner(self).reflink(other)
        }
    }

    impl ReflinkOp<cap_std::fs::File> for File {
        #[inline]
        fn reflink(&self, other: &cap_std::fs::File) -> io::Result<()> {
            self.reflink(&*FileRef::from_owner(other))
        }
    }
}
