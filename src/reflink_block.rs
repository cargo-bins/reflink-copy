use crate::sys;
use std::fs::File;
use std::io;
use std::num::NonZeroU64;

/// Creates a reflink of a specified block from one file to another.
///
/// This functionality is designed to be highly performant and does not perform any extra API calls.
/// It is expected that the user takes care of necessary preliminary checks and preparations.
///
/// If you need to clone an entire file, consider using the [`reflink`] or [`reflink_or_copy`]
/// functions instead.
///
/// > Note: Currently the function works only for windows. It returns `Err` for any other platform.
///
/// # Windows Restrictions and Remarks
/// - The source and destination regions must begin and end at a cluster boundary.
/// - The destination region must not extend past the end of file. If the application wishes to
///   extend the destination with cloned data, it must first call
///   [`File::set_len`](fn@std::fs::File::set_len).
/// - If the source and destination regions are in the same file, they must not overlap. (The
///   application may able to proceed by splitting up the block clone operation into multiple block
///   clones that no longer overlap.)
/// - The source and destination files must be on the same ReFS volume.
/// - The source and destination files must have the same Integrity Streams setting (that is,
///   Integrity Streams must be enabled in both files, or disabled in both files).
/// - If the source file is sparse, the destination file must also be sparse.
/// - The block clone operation will break Shared Opportunistic Locks (also known as Level 2
///   Opportunistic Locks).
/// - The ReFS volume must have been formatted with Windows Server 2016, and if Windows Failover
///   Clustering is in use, the Clustering Functional Level must have been Windows Server 2016 or
///   later at format time.
/// - Note: If block is 4GB or larger, [`ReflinkBlockBuilder::reflink_block`] splits it to multiple
///   smaller blocks with the size of 4GB minus cluster size.
///
/// More information can be found by the
/// [link](https://learn.microsoft.com/en-us/windows/win32/fileio/block-cloning).
///
/// # Examples
///
/// ```no_run
/// use std::fs::File;
/// use std::num::NonZeroU64;
///
/// fn main() -> std::io::Result<()> {
///     const CLUSTER_SIZE: u64 = 4096;
///     let from_file = File::open("source.txt")?;
///     let len = from_file.metadata()?.len();
///     let to_file = File::create("destination.txt")?;
///     to_file.set_len(len)?;
///     let mut offset = 0u64;
///     while offset < len {
///         reflink_copy::ReflinkBlockBuilder::new()
///             .from(&from_file)
///             .from_offset(offset)
///             .to(&to_file)
///             .to_offset(offset)
///             .src_length(NonZeroU64::new(CLUSTER_SIZE).unwrap())
///             .cluster_size(NonZeroU64::new(CLUSTER_SIZE).unwrap())
///             .reflink_block()?;
///         offset += CLUSTER_SIZE;
///     }
///     Ok(())
/// }
/// ```
/// [`reflink`]: crate::reflink
/// [`reflink_or_copy`]: crate::reflink_or_copy
#[derive(Debug, Default)]
pub struct ReflinkBlockBuilder<'from, 'to> {
    from: Option<&'from File>,
    from_offset: u64,
    to: Option<&'to File>,
    to_offset: u64,
    src_length: u64,
    cluster_size: Option<NonZeroU64>,
}

impl<'from, 'to> ReflinkBlockBuilder<'from, 'to> {
    /// Creates a new instance of [`ReflinkBlockBuilder`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the source file.
    #[must_use]
    pub fn from(mut self, from: &'from File) -> ReflinkBlockBuilder<'from, 'to> {
        self.from = Some(from);
        self
    }

    /// Sets the offset within the source file.
    #[must_use]
    pub fn from_offset(mut self, from_offset: u64) -> Self {
        self.from_offset = from_offset;
        self
    }

    /// Sets the destination file.
    #[must_use]
    pub fn to(mut self, to: &'to File) -> ReflinkBlockBuilder<'from, 'to> {
        self.to = Some(to);
        self
    }

    /// Sets the offset within the destination file.
    #[must_use]
    pub fn to_offset(mut self, to_offset: u64) -> Self {
        self.to_offset = to_offset;
        self
    }

    /// Sets the length of the source data to be reflinked.
    #[must_use]
    pub fn src_length(mut self, src_length: NonZeroU64) -> Self {
        self.src_length = src_length.get();
        self
    }

    /// Sets the cluster size. It is used to calculate the max block size of a single reflink call
    /// on Windows.
    #[must_use]
    pub fn cluster_size(mut self, cluster_size: NonZeroU64) -> Self {
        self.cluster_size = Some(cluster_size);
        self
    }

    /// Performs reflink operation for the specified block of data.
    #[cfg_attr(not(windows), allow(unused_variables))]
    pub fn reflink_block(self) -> io::Result<()> {
        assert!(self.from.is_some(), "`from` is not set");
        assert!(self.to.is_some(), "`to` is not set");
        assert_ne!(self.src_length, 0, "`src_length` is not set");

        #[cfg(windows)]
        return sys::reflink_block(
            self.from.unwrap(),
            self.from_offset,
            self.to.unwrap(),
            self.to_offset,
            self.src_length,
            self.cluster_size,
        );
        #[cfg(not(windows))]
        Err(io::Error::other("Not implemented"))
    }
}
