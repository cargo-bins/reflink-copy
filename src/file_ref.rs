//! A borrowed view of a [`File`], which can be created either from a
//! `&std::fs::File` or (on unix and windows) from any type owning a file
//! descriptor/handle, e.g. `cap_std::fs::File`.

use std::fmt;
use std::fs::File;
#[cfg(any(unix, windows))]
use std::{marker::PhantomData, mem::ManuallyDrop, ops::Deref};

#[cfg(any(unix, windows))]
pub(crate) struct FileRef<'a> {
    // Never dropped: the file descriptor/handle is owned by someone else,
    // `'a` ensures it outlives this view.
    file: ManuallyDrop<File>,
    _marker: PhantomData<&'a File>,
}

#[cfg(any(unix, windows))]
impl<'a> FileRef<'a> {
    pub(crate) fn new(file: &'a File) -> Self {
        Self::from_owner(file)
    }

    #[cfg(unix)]
    pub(crate) fn from_owner<T: std::os::unix::io::AsFd + ?Sized>(owner: &'a T) -> Self {
        use std::os::unix::io::{AsRawFd, FromRawFd};

        let fd = owner.as_fd().as_raw_fd();
        // SAFETY: `fd` is valid for `'a` since `owner` is borrowed for `'a`,
        // and the `File` is wrapped in `ManuallyDrop` so it never closes it.
        let file = unsafe { File::from_raw_fd(fd) };
        Self {
            file: ManuallyDrop::new(file),
            _marker: PhantomData,
        }
    }

    #[cfg(windows)]
    pub(crate) fn from_owner<T: std::os::windows::io::AsHandle + ?Sized>(owner: &'a T) -> Self {
        use std::os::windows::io::{AsRawHandle, FromRawHandle};

        let handle = owner.as_handle().as_raw_handle();
        // SAFETY: `handle` is valid for `'a` since `owner` is borrowed for `'a`,
        // and the `File` is wrapped in `ManuallyDrop` so it never closes it.
        let file = unsafe { File::from_raw_handle(handle) };
        Self {
            file: ManuallyDrop::new(file),
            _marker: PhantomData,
        }
    }
}

#[cfg(any(unix, windows))]
impl Deref for FileRef<'_> {
    type Target = File;

    fn deref(&self) -> &File {
        &self.file
    }
}

#[cfg(not(any(unix, windows)))]
pub(crate) struct FileRef<'a>(&'a File);

#[cfg(not(any(unix, windows)))]
impl<'a> FileRef<'a> {
    pub(crate) fn new(file: &'a File) -> Self {
        Self(file)
    }
}

#[cfg(not(any(unix, windows)))]
impl std::ops::Deref for FileRef<'_> {
    type Target = File;

    fn deref(&self) -> &File {
        self.0
    }
}

impl fmt::Debug for FileRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}
