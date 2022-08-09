#![allow(dead_code)]

use std::{
    fs::{remove_file, File},
    io,
    path::{Path, PathBuf},
};

use derive_destructure2::destructure;

#[derive(Debug, destructure)]
pub(super) struct NamedTempFile {
    pub(super) inner: File,
    path: PathBuf,
}

impl NamedTempFile {
    pub(super) fn create_new(path: &Path) -> io::Result<Self> {
        // pass O_EXCL to mimic macos behaviour
        let inner = File::options().write(true).create_new(true).open(path)?;

        Ok(Self {
            inner,
            path: path.into(),
        })
    }

    pub(super) fn persist(self) {
        self.destructure();
    }
}

impl Drop for NamedTempFile {
    fn drop(&mut self) {
        let _ = remove_file(&self.path);
    }
}
