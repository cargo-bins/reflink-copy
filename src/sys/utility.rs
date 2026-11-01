#![allow(dead_code)]

use std::{
    fs::{remove_file, File},
    io,
    path::Path,
};

#[cfg(unix)]
use std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, RawFd};

/// A newly created file, removed on drop unless
/// [`AutoRemovedFile::persist_with_same_permission_as`] is called.
#[derive(Debug)]
pub(super) struct AutoRemovedFile<'a> {
    // Option<File> uses File's niche, so this is zero cost
    inner: Option<File>,
    path: &'a Path,
    /// The directory `path` is relative to.
    #[cfg(all(feature = "cap-std", any(unix, windows)))]
    dir: Option<&'a cap_std::fs::Dir>,
}

impl<'a> AutoRemovedFile<'a> {
    pub fn create_new(path: &'a Path) -> io::Result<Self> {
        // pass O_EXCL to mimic macos behaviour
        let inner = File::options().write(true).create_new(true).open(path)?;

        Ok(Self {
            inner: Some(inner),
            path,
            #[cfg(all(feature = "cap-std", any(unix, windows)))]
            dir: None,
        })
    }

    /// Like [`AutoRemovedFile::create_new`], with `path` bounded by `dir`.
    #[cfg(all(feature = "cap-std", any(unix, windows)))]
    pub fn create_new_in(dir: &'a cap_std::fs::Dir, path: &'a Path) -> io::Result<Self> {
        // pass O_EXCL to mimic macos behaviour
        let inner = dir
            .open_with(
                path,
                cap_std::fs::OpenOptions::new().write(true).create_new(true),
            )?
            .into_std();

        Ok(Self {
            inner: Some(inner),
            path,
            dir: Some(dir),
        })
    }

    #[cfg(unix)]
    pub fn as_raw_fd(&self) -> RawFd {
        self.as_inner_file().as_raw_fd()
    }

    /// Keep the file, giving it the same permissions as `file`.
    pub fn persist_with_same_permission_as(mut self, file: &File) -> io::Result<()> {
        let permissions = file.metadata()?.permissions();
        self.inner.take().unwrap().set_permissions(permissions)
    }

    pub fn as_inner_file(&self) -> &File {
        self.inner.as_ref().unwrap()
    }
}

#[cfg(unix)]
impl AsFd for AutoRemovedFile<'_> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.as_inner_file().as_fd()
    }
}

impl Drop for AutoRemovedFile<'_> {
    fn drop(&mut self) {
        // Close it first, an opened file may not be removable on windows.
        let Some(_) = self.inner.take() else {
            return;
        };

        #[cfg(all(feature = "cap-std", any(unix, windows)))]
        let res = match self.dir {
            Some(dir) => dir.remove_file(self.path),
            None => remove_file(self.path),
        };
        #[cfg(not(all(feature = "cap-std", any(unix, windows))))]
        let res = remove_file(self.path);

        if let Err(_err) = res {
            #[cfg(feature = "tracing")]
            tracing::warn!(
                ?_err,
                "Failed to remove dest file {} on cleanup (failed to reflink)",
                self.path.display(),
            );
        }
    }
}
