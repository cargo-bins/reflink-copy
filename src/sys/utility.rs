#![allow(dead_code)]

use std::{
    fs::{remove_file, File},
    io,
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::os::unix::io::{AsRawFd, RawFd};

#[derive(Debug)]
pub(super) struct AutoRemovedFile {
    // Option<File> uses File's niche, so this is zero cost
    inner: Option<File>,
    path: PathBuf,
}

impl AutoRemovedFile {
    pub fn create_new(path: &Path) -> io::Result<Self> {
        // pass O_EXCL to mimic macos behaviour
        let inner = File::options().write(true).create_new(true).open(path)?;

        Ok(Self {
            inner: Some(inner),
            path: path.into(),
        })
    }

    #[cfg(unix)]
    pub fn as_raw_fd(&self) -> Option<RawFd> {
        self.inner.as_ref().map(|file| file.as_raw_fd())
    }

    pub fn persist(mut self) {
        self.inner.take();
    }
}

impl Drop for AutoRemovedFile {
    fn drop(&mut self) {
        if self.inner.is_some() {
            let _ = remove_file(&self.path);
        }
    }
}
