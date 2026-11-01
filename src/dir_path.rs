use std::fs::File;
use std::io;
use std::path::Path;

use cap_std::fs::Dir;

use crate::ReflinkOp;

/// A path bounded by a [`cap_std::fs::Dir`].
///
/// Like the paths accepted by [`Dir`]'s methods, `path` is resolved relative
/// to `dir` and is never allowed to escape it: absolute paths, `..` going
/// above `dir` and symlinks pointing outside of `dir` are rejected.
///
/// It implements [`ReflinkOp`], with the same semantics as the `Path`
/// implementations:
///
/// - `File` to `DirPath`: reflink an opened file into a new file inside the
///   directory.
/// - `DirPath` to `DirPath`.
///
/// ```no_run
/// use cap_std::{ambient_authority, fs::Dir};
/// use reflink_copy::{DirPath, ReflinkOp};
/// use std::fs::File;
///
/// fn clone() -> std::io::Result<()> {
///     let dir = Dir::open_ambient_dir("install-dir", ambient_authority())?;
///     let src = File::open("src.txt")?;
///     src.reflink(&DirPath::new(&dir, "bin/dest.txt"))
/// }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct DirPath<'a> {
    dir: &'a Dir,
    path: &'a Path,
}

impl<'a> DirPath<'a> {
    /// Creates a new [`DirPath`] for `path` relative to `dir`.
    pub fn new<P: AsRef<Path> + ?Sized>(dir: &'a Dir, path: &'a P) -> Self {
        Self {
            dir,
            path: path.as_ref(),
        }
    }

    /// The directory `path` is relative to.
    pub fn dir(&self) -> &'a Dir {
        self.dir
    }

    /// The path, relative to `dir`.
    pub fn path(&self) -> &'a Path {
        self.path
    }
}

impl ReflinkOp<DirPath<'_>> for File {
    /// Reflink `self` into a new file at `other`.
    ///
    /// Same as the `ReflinkOp<Path>` implementation, except that the new
    /// file is created inside `other.dir()`: it is created with
    /// `OpenOptions::create_new`, gets the permissions of `self`, and is
    /// removed if the reflink fails.
    ///
    /// Supported on Linux, Android, Windows and macOS (`fclonefileat`).
    #[cfg_attr(
        feature = "tracing",
        tracing_attributes::instrument(name = "reflink_file_to_dir_path")
    )]
    fn reflink(&self, other: &DirPath<'_>) -> io::Result<()> {
        reflink_file_to_dir_path(self, other.dir, other.path)
    }
}

impl ReflinkOp for DirPath<'_> {
    /// Same as [`reflink`](crate::reflink), with both paths bounded by their
    /// directory.
    #[cfg_attr(
        feature = "tracing",
        tracing_attributes::instrument(name = "reflink_dir_path")
    )]
    fn reflink(&self, other: &DirPath<'_>) -> io::Result<()> {
        let src = self.dir.open(self.path)?.into_std();
        reflink_file_to_dir_path(&src, other.dir, other.path)
    }
}

#[cfg(any(
    all(
        any(target_os = "linux", target_os = "android"),
        not(any(target_arch = "sparc", target_arch = "sparc64"))
    ),
    windows
))]
fn reflink_file_to_dir_path(from: &File, dir: &Dir, path: &Path) -> io::Result<()> {
    let permissions = from.metadata()?.permissions();

    // pass O_EXCL to mimic macos behaviour
    let dest = dir
        .open_with(
            path,
            cap_std::fs::OpenOptions::new().write(true).create_new(true),
        )?
        .into_std();

    let res =
        crate::sys::reflink_file(from, &dest).and_then(|()| dest.set_permissions(permissions));
    if res.is_err() {
        // Close it first, windows cannot remove an opened file.
        drop(dest);
        if let Err(_err) = dir.remove_file(path) {
            #[cfg(feature = "tracing")]
            tracing::warn!(
                ?_err,
                "Failed to remove dest file {} on cleanup (failed to reflink)",
                path.display(),
            );
        }
    }
    res
}

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos"
))]
fn reflink_file_to_dir_path(from: &File, dir: &Dir, path: &Path) -> io::Result<()> {
    use std::os::unix::io::AsFd;

    // `fclonefileat` resolves `dst` relative to the directory fd without any
    // restriction, so open the parent with cap-std and only pass it a single
    // normal component.
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    let parent_dir;
    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => {
            parent_dir = dir.open_dir(parent)?;
            &parent_dir
        }
        _ => dir,
    };
    crate::sys::reflink_file_to_dir_entry(from, parent.as_fd(), name)
}

#[cfg(not(any(
    all(
        any(target_os = "linux", target_os = "android"),
        not(any(target_arch = "sparc", target_arch = "sparc64"))
    ),
    windows,
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos"
)))]
fn reflink_file_to_dir_path(_from: &File, _dir: &Dir, _path: &Path) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}
